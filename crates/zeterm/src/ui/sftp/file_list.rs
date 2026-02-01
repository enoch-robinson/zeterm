//! 文件列表组件
//!
//! 提供 SFTP 文件列表显示功能，包括：
//! - 表格形式显示文件和目录
//! - 排序功能（名称、大小、修改时间、类型）
//! - 单选和多选支持
//! - 双击打开/进入目录
//! - 右键菜单支持

use std::collections::HashSet;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

use zeterm_ssh::{DirEntry, EntryType, format_file_size};

// ============================================================================
// 排序相关
// ============================================================================

/// 排序列
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    /// 按名称排序
    #[default]
    Name,
    /// 按大小排序
    Size,
    /// 按修改时间排序
    Modified,
    /// 按类型排序
    Type,
}

impl SortColumn {
    /// 获取列标题
    pub fn title(&self) -> &'static str {
        match self {
            SortColumn::Name => "Name",
            SortColumn::Size => "Size",
            SortColumn::Modified => "Modified",
            SortColumn::Type => "Type",
        }
    }

    /// 所有列
    pub fn all() -> &'static [SortColumn] {
        &[
            SortColumn::Name,
            SortColumn::Size,
            SortColumn::Modified,
            SortColumn::Type,
        ]
    }
}

/// 排序顺序
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// 升序
    #[default]
    Ascending,
    /// 降序
    Descending,
}

impl SortOrder {
    /// 切换排序顺序
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }

    /// 获取排序图标
    pub fn icon(&self) -> &'static str {
        match self {
            SortOrder::Ascending => "▲",
            SortOrder::Descending => "▼",
        }
    }
}

// ============================================================================
// 事件定义
// ============================================================================

/// 文件列表事件
#[derive(Debug, Clone)]
pub enum FileListEvent {
    /// 单击选中文件
    Selected(String),
    /// 双击打开文件/目录
    Opened(String),
    /// 选中多个文件
    MultiSelected(Vec<String>),
    /// 请求右键菜单
    ContextMenu { path: String, position: (f32, f32) },
    /// 排序变更
    SortChanged {
        column: SortColumn,
        order: SortOrder,
    },
    /// 请求上传（拖放本地文件）
    UploadRequested(Vec<String>),
}

// ============================================================================
// 文件列表组件
// ============================================================================

/// 文件列表视图
pub struct FileListView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 文件条目列表
    entries: Vec<DirEntry>,
    /// 当前选中的索引
    selected_index: Option<usize>,
    /// 多选的索引集合
    multi_selected: HashSet<usize>,
    /// 是否多选模式
    multi_select_mode: bool,
    /// 当前排序列
    sort_column: SortColumn,
    /// 当前排序顺序
    sort_order: SortOrder,
    /// 是否显示隐藏文件
    show_hidden: bool,
    /// 上次点击时间（用于检测双击）
    last_click_time: std::time::Instant,
    /// 上次点击的索引
    last_click_index: Option<usize>,
    /// hover 的行索引
    hovered_index: Option<usize>,
    /// 是否正在加载
    loading: bool,
    /// 错误消息
    error_message: Option<String>,
}

impl FileListView {
    /// 双击检测间隔（毫秒）
    const DOUBLE_CLICK_MS: u128 = 500;

    /// 创建新的文件列表视图
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            entries: Vec::new(),
            selected_index: None,
            multi_selected: HashSet::new(),
            multi_select_mode: false,
            sort_column: SortColumn::Name,
            sort_order: SortOrder::Ascending,
            show_hidden: false,
            last_click_time: std::time::Instant::now(),
            last_click_index: None,
            hovered_index: None,
            loading: false,
            error_message: None,
        }
    }

    /// 设置文件条目列表
    pub fn set_entries(&mut self, entries: Vec<DirEntry>, cx: &mut Context<Self>) {
        self.entries = entries;
        self.sort_entries();
        self.filter_entries();
        self.selected_index = None;
        self.multi_selected.clear();
        self.loading = false;
        self.error_message = None;
        cx.notify();
    }

    /// 设置加载状态
    pub fn set_loading(&mut self, loading: bool, cx: &mut Context<Self>) {
        self.loading = loading;
        if loading {
            self.error_message = None;
        }
        cx.notify();
    }

    /// 设置错误消息
    pub fn set_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.error_message = error;
        self.loading = false;
        cx.notify();
    }

    /// 设置是否显示隐藏文件
    pub fn set_show_hidden(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_hidden = show;
        self.filter_entries();
        cx.notify();
    }

    /// 设置排序
    pub fn set_sort(&mut self, column: SortColumn, order: SortOrder, cx: &mut Context<Self>) {
        self.sort_column = column;
        self.sort_order = order;
        self.sort_entries();
        cx.emit(FileListEvent::SortChanged { column, order });
        cx.notify();
    }

    /// 切换排序列
    pub fn toggle_sort_column(&mut self, column: SortColumn, cx: &mut Context<Self>) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.sort_entries();
        cx.emit(FileListEvent::SortChanged {
            column: self.sort_column,
            order: self.sort_order,
        });
        cx.notify();
    }

    /// 启用/禁用多选模式
    pub fn set_multi_select_mode(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.multi_select_mode = enabled;
        if !enabled {
            self.multi_selected.clear();
        }
        cx.notify();
    }

    /// 获取当前选中的条目
    pub fn selected_entry(&self) -> Option<&DirEntry> {
        self.selected_index
            .and_then(|i| self.visible_entries().get(i).copied())
    }

    /// 获取所有选中的条目
    pub fn selected_entries(&self) -> Vec<&DirEntry> {
        let visible = self.visible_entries();
        if self.multi_select_mode && !self.multi_selected.is_empty() {
            self.multi_selected
                .iter()
                .filter_map(|&i| visible.get(i).copied())
                .collect()
        } else {
            self.selected_entry().into_iter().collect()
        }
    }

    /// 获取可见的条目列表（考虑隐藏文件过滤）
    fn visible_entries(&self) -> Vec<&DirEntry> {
        if self.show_hidden {
            self.entries.iter().collect()
        } else {
            self.entries.iter().filter(|e| !e.is_hidden()).collect()
        }
    }

    /// 排序条目
    fn sort_entries(&mut self) {
        let column = self.sort_column;
        let order = self.sort_order;

        self.entries.sort_by(|a, b| {
            // 目录始终在文件之前
            let dir_cmp = b.is_dir().cmp(&a.is_dir());
            if dir_cmp != std::cmp::Ordering::Equal {
                return dir_cmp;
            }

            let cmp = match column {
                SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Modified => a.modified.cmp(&b.modified),
                SortColumn::Type => {
                    let ext_a = a.extension().unwrap_or("");
                    let ext_b = b.extension().unwrap_or("");
                    ext_a.to_lowercase().cmp(&ext_b.to_lowercase())
                },
            };

            match order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });
    }

    /// 过滤条目（隐藏文件）
    fn filter_entries(&mut self) {
        // 实际过滤在 visible_entries() 中进行
        // 这里只需要重置选中状态
        self.selected_index = None;
        self.multi_selected.clear();
    }

    /// 处理行点击
    fn handle_row_click(&mut self, index: usize, cx: &mut Context<Self>) {
        let now = std::time::Instant::now();
        let is_double_click = self.last_click_index == Some(index)
            && now.duration_since(self.last_click_time).as_millis() < Self::DOUBLE_CLICK_MS;

        self.last_click_time = now;
        self.last_click_index = Some(index);

        if is_double_click {
            // 双击：打开文件/目录
            if let Some(entry) = self.visible_entries().get(index) {
                cx.emit(FileListEvent::Opened(entry.path.clone()));
            }
        } else {
            // 单击：选中
            if self.multi_select_mode {
                if self.multi_selected.contains(&index) {
                    self.multi_selected.remove(&index);
                } else {
                    self.multi_selected.insert(index);
                }
                let paths: Vec<String> = self
                    .multi_selected
                    .iter()
                    .filter_map(|&i| self.visible_entries().get(i).map(|e| e.path.clone()))
                    .collect();
                cx.emit(FileListEvent::MultiSelected(paths));
            } else {
                self.selected_index = Some(index);
                if let Some(entry) = self.visible_entries().get(index) {
                    cx.emit(FileListEvent::Selected(entry.path.clone()));
                }
            }
        }
        cx.notify();
    }

    /// 处理右键点击
    fn handle_context_menu(&mut self, index: usize, position: (f32, f32), cx: &mut Context<Self>) {
        self.selected_index = Some(index);
        if let Some(entry) = self.visible_entries().get(index) {
            cx.emit(FileListEvent::ContextMenu {
                path: entry.path.clone(),
                position,
            });
        }
        cx.notify();
    }

    /// 获取文件类型图标
    fn get_file_icon(entry: &DirEntry) -> &'static str {
        match entry.entry_type {
            EntryType::Directory => "📁",
            EntryType::Symlink => "🔗",
            EntryType::File => {
                // 根据扩展名选择图标
                match entry.extension().map(|s| s.to_lowercase()).as_deref() {
                    Some("txt" | "md" | "log") => "📄",
                    Some("rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go") => "📜",
                    Some("jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp") => "🖼",
                    Some("mp3" | "wav" | "flac" | "ogg" | "m4a") => "🎵",
                    Some("mp4" | "avi" | "mkv" | "mov" | "webm") => "🎬",
                    Some("zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar") => "📦",
                    Some("pdf") => "📕",
                    Some("doc" | "docx" | "odt") => "📘",
                    Some("xls" | "xlsx" | "ods") => "📗",
                    Some("ppt" | "pptx" | "odp") => "📙",
                    Some("exe" | "app" | "dmg" | "deb" | "rpm") => "⚙️",
                    Some("sh" | "bash" | "zsh" | "fish") => "🐚",
                    Some("json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "conf") => "⚙️",
                    Some("html" | "htm" | "css" | "scss" | "sass") => "🌐",
                    Some("sql" | "db" | "sqlite") => "🗃",
                    Some("key" | "pem" | "crt" | "cer") => "🔑",
                    _ => "📄",
                }
            },
            EntryType::Other => "❓",
        }
    }

    /// 渲染表头
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let sort_column = self.sort_column;
        let sort_order = self.sort_order;

        div()
            .w_full()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            .text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            // 图标列
            .child(div().w(px(28.0)))
            // 名称列
            .child(
                div()
                    .id("header-name")
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if sort_column == SortColumn::Name {
                        theme.primary
                    } else {
                        theme.foreground
                    })
                    .child("Name")
                    .when(sort_column == SortColumn::Name, |el| {
                        el.child(SharedString::from(sort_order.icon()))
                    }),
            )
            // 大小列
            .child(
                div()
                    .id("header-size")
                    .w(px(80.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if sort_column == SortColumn::Size {
                        theme.primary
                    } else {
                        theme.foreground
                    })
                    .child("Size")
                    .when(sort_column == SortColumn::Size, |el| {
                        el.child(SharedString::from(sort_order.icon()))
                    }),
            )
            // 修改时间列
            .child(
                div()
                    .id("header-modified")
                    .w(px(140.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .text_color(if sort_column == SortColumn::Modified {
                        theme.primary
                    } else {
                        theme.foreground
                    })
                    .child("Modified")
                    .when(sort_column == SortColumn::Modified, |el| {
                        el.child(SharedString::from(sort_order.icon()))
                    }),
            )
            // 权限列
            .child(
                div()
                    .w(px(90.0))
                    .text_color(theme.muted_foreground)
                    .child("Permissions"),
            )
    }

    /// 渲染文件行
    fn render_row(&self, entry: &DirEntry, index: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_selected =
            self.selected_index == Some(index) || self.multi_selected.contains(&index);
        let is_hovered = self.hovered_index == Some(index);

        let icon = Self::get_file_icon(entry);
        let name = entry.name.clone();
        let size_str = if entry.is_dir() {
            "-".to_string()
        } else {
            format_file_size(entry.size)
        };
        let modified_str = entry.formatted_modified();
        let permissions_str = entry
            .permissions
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "---------".to_string());

        let bg_color = if is_selected {
            theme.selection
        } else if is_hovered {
            theme.secondary
        } else {
            gpui::transparent_black()
        };

        let text_color = if entry.is_hidden() {
            theme.muted_foreground
        } else {
            theme.foreground
        };

        div()
            .id(SharedString::from(format!("row-{}", index)))
            .w_full()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .bg(bg_color)
            .text_sm()
            .cursor_pointer()
            .border_b_1()
            .border_color(theme.border.opacity(0.3))
            .hover(|el| el.bg(theme.secondary))
            // 图标
            .child(div().w(px(28.0)).text_center().child(icon))
            // 名称
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_color(text_color)
                    .when(entry.is_dir(), |el| {
                        el.font_weight(gpui::FontWeight::MEDIUM)
                    })
                    .child(SharedString::from(name)),
            )
            // 大小
            .child(
                div()
                    .w(px(80.0))
                    .text_right()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(size_str)),
            )
            // 修改时间
            .child(
                div()
                    .w(px(140.0))
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(modified_str)),
            )
            // 权限
            .child(
                div()
                    .w(px(90.0))
                    .font_family("monospace")
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(permissions_str)),
            )
    }

    /// 渲染空状态
    fn render_empty(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(theme.muted_foreground)
            .child(div().text_2xl().child("📂"))
            .child(div().text_sm().child("Empty directory"))
    }

    /// 渲染加载状态
    fn render_loading(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(theme.muted_foreground)
            .child(div().text_2xl().child("⏳"))
            .child(div().text_sm().child("Loading..."))
    }

    /// 渲染错误状态
    fn render_error(&self, message: &str, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                div()
                    .text_2xl()
                    .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                    .child("⚠️"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Failed to load directory"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(gpui::hsla(0.0, 0.6, 0.5, 1.0))
                    .max_w(px(300.0))
                    .text_center()
                    .child(SharedString::from(message.to_string())),
            )
    }

    /// 渲染文件列表内容
    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let visible = self.visible_entries();

        div().w_full().flex_1().overflow_y_hidden().children(
            visible
                .iter()
                .enumerate()
                .map(|(i, entry)| self.render_row(entry, i, cx)),
        )
    }
}

impl EventEmitter<FileListEvent> for FileListView {}

impl Focusable for FileListView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FileListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let loading = self.loading;
        let error = self.error_message.clone();
        let entries_empty = self.visible_entries().is_empty();

        div()
            .id("sftp-file-list")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            // 表头
            .child(self.render_header(cx))
            // 内容区域
            .child(div().flex_1().overflow_hidden().child(if loading {
                self.render_loading(cx).into_any_element()
            } else if let Some(err) = error.as_ref() {
                self.render_error(err, cx).into_any_element()
            } else if entries_empty {
                self.render_empty(cx).into_any_element()
            } else {
                self.render_content(cx).into_any_element()
            }))
            // 状态栏
            .child(
                div()
                    .w_full()
                    .h(px(24.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .bg(theme.secondary)
                    .border_t_1()
                    .border_color(theme.border)
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(format!(
                        "{} items{}",
                        self.visible_entries().len(),
                        if self.multi_selected.is_empty() {
                            String::new()
                        } else {
                            format!(" ({} selected)", self.multi_selected.len())
                        }
                    ))),
            )
    }
}

/// 创建文件列表视图的便捷方法
pub fn create_file_list_view(cx: &mut Context<FileListView>) -> FileListView {
    FileListView::new(cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_column_title() {
        assert_eq!(SortColumn::Name.title(), "Name");
        assert_eq!(SortColumn::Size.title(), "Size");
        assert_eq!(SortColumn::Modified.title(), "Modified");
        assert_eq!(SortColumn::Type.title(), "Type");
    }

    #[test]
    fn test_sort_column_all() {
        let all = SortColumn::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&SortColumn::Name));
        assert!(all.contains(&SortColumn::Size));
    }

    #[test]
    fn test_sort_order_toggle() {
        assert_eq!(SortOrder::Ascending.toggle(), SortOrder::Descending);
        assert_eq!(SortOrder::Descending.toggle(), SortOrder::Ascending);
    }

    #[test]
    fn test_sort_order_icon() {
        assert_eq!(SortOrder::Ascending.icon(), "▲");
        assert_eq!(SortOrder::Descending.icon(), "▼");
    }

    #[test]
    fn test_file_icon_directory() {
        let dir = DirEntry::new("test", "/test", EntryType::Directory);
        assert_eq!(FileListView::get_file_icon(&dir), "📁");
    }

    #[test]
    fn test_file_icon_symlink() {
        let link = DirEntry::new("link", "/link", EntryType::Symlink);
        assert_eq!(FileListView::get_file_icon(&link), "🔗");
    }

    #[test]
    fn test_file_icon_by_extension() {
        let rs = DirEntry::new("main.rs", "/main.rs", EntryType::File);
        assert_eq!(FileListView::get_file_icon(&rs), "📜");

        let png = DirEntry::new("image.png", "/image.png", EntryType::File);
        assert_eq!(FileListView::get_file_icon(&png), "🖼");

        let zip = DirEntry::new("archive.zip", "/archive.zip", EntryType::File);
        assert_eq!(FileListView::get_file_icon(&zip), "📦");

        let pdf = DirEntry::new("document.pdf", "/document.pdf", EntryType::File);
        assert_eq!(FileListView::get_file_icon(&pdf), "📕");
    }

    #[test]
    fn test_file_list_event_debug() {
        let event = FileListEvent::Selected("/test/file.txt".to_string());
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Selected"));

        let event2 = FileListEvent::Opened("/home".to_string());
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("Opened"));
    }

    #[test]
    fn test_sort_change_event() {
        let event = FileListEvent::SortChanged {
            column: SortColumn::Size,
            order: SortOrder::Descending,
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("SortChanged"));
        assert!(debug_str.contains("Size"));
        assert!(debug_str.contains("Descending"));
    }

    #[test]
    fn test_context_menu_event() {
        let event = FileListEvent::ContextMenu {
            path: "/home/user/file.txt".to_string(),
            position: (100.0, 200.0),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("ContextMenu"));
        assert!(debug_str.contains("file.txt"));
    }

    #[test]
    fn test_double_click_constant() {
        assert_eq!(FileListView::DOUBLE_CLICK_MS, 500);
    }
}
