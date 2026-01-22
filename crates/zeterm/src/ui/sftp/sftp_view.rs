//! SFTP 主视图组件
//!
//! 整合所有 SFTP 子组件，提供完整的文件管理界面，包括：
//! - 路径导航栏
//! - 文件列表
//! - 传输队列
//! - 工具栏操作

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

use zeterm_ssh::{SftpClient, TransferProgress, TransferTaskId};

use super::file_list::{FileListEvent, FileListView};
use super::path_bar::{PathBar, PathBarEvent};
use super::transfer_queue::{TransferQueueEvent, TransferQueueView};

// ============================================================================
// 事件定义
// ============================================================================

/// SFTP 视图事件
#[derive(Debug, Clone)]
pub enum SftpViewEvent {
    /// 请求关闭视图
    CloseRequested,
    /// 文件被选中
    FileSelected(String),
    /// 文件被打开（双击）
    FileOpened(String),
    /// 请求下载文件
    DownloadRequested {
        remote_path: String,
        local_path: Option<String>,
    },
    /// 请求上传文件
    UploadRequested {
        local_paths: Vec<String>,
        remote_dir: String,
    },
    /// 传输开始
    TransferStarted(TransferTaskId),
    /// 传输完成
    TransferCompleted(TransferTaskId),
    /// 传输失败
    TransferFailed {
        task_id: TransferTaskId,
        error: String,
    },
    /// 错误发生
    Error(String),
}

// ============================================================================
// 视图模式
// ============================================================================

/// SFTP 视图模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SftpViewMode {
    /// 文件浏览模式
    #[default]
    Browse,
    /// 传输队列模式（全屏显示传输）
    Transfers,
    /// 分屏模式（左右两个目录）
    Split,
}

impl SftpViewMode {
    /// 获取模式标题
    pub fn title(&self) -> &'static str {
        match self {
            SftpViewMode::Browse => "Browse",
            SftpViewMode::Transfers => "Transfers",
            SftpViewMode::Split => "Split View",
        }
    }

    /// 获取模式图标
    pub fn icon(&self) -> &'static str {
        match self {
            SftpViewMode::Browse => "📁",
            SftpViewMode::Transfers => "📦",
            SftpViewMode::Split => "⧉",
        }
    }
}

// ============================================================================
// SFTP 视图
// ============================================================================

/// SFTP 视图组件
///
/// 整合路径栏、文件列表和传输队列，提供完整的 SFTP 文件管理界面。
pub struct SftpView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// SFTP 客户端（可选，因为可能还未连接）
    sftp_client: Option<Arc<SftpClient>>,
    /// 路径栏
    path_bar: Entity<PathBar>,
    /// 文件列表
    file_list: Entity<FileListView>,
    /// 传输队列
    transfer_queue: Entity<TransferQueueView>,
    /// 当前视图模式
    mode: SftpViewMode,
    /// 当前路径
    current_path: String,
    /// 主机名（用于显示）
    hostname: Option<String>,
    /// 用户名
    username: Option<String>,
    /// 是否显示传输队列
    show_transfer_queue: bool,
    /// 是否正在加载
    loading: bool,
    /// 错误消息
    error_message: Option<String>,
    /// 连接状态
    connected: bool,
}

impl SftpView {
    /// 创建新的 SFTP 视图（未连接状态）
    pub fn new(cx: &mut Context<Self>) -> Self {
        let path_bar = cx.new(|cx| PathBar::new(cx));
        let file_list = cx.new(|cx| FileListView::new(cx));
        let transfer_queue = cx.new(|cx| TransferQueueView::new(cx));

        // 订阅子组件事件
        let this = Self {
            focus_handle: cx.focus_handle(),
            sftp_client: None,
            path_bar: path_bar.clone(),
            file_list: file_list.clone(),
            transfer_queue: transfer_queue.clone(),
            mode: SftpViewMode::Browse,
            current_path: "/".to_string(),
            hostname: None,
            username: None,
            show_transfer_queue: true,
            loading: false,
            error_message: None,
            connected: false,
        };

        // 订阅路径栏事件
        cx.subscribe(&path_bar, Self::handle_path_bar_event)
            .detach();

        // 订阅文件列表事件
        cx.subscribe(&file_list, Self::handle_file_list_event)
            .detach();

        // 订阅传输队列事件
        cx.subscribe(&transfer_queue, Self::handle_transfer_queue_event)
            .detach();

        this
    }

    /// 创建并连接到 SFTP 客户端
    pub fn with_client(mut self, client: Arc<SftpClient>) -> Self {
        self.sftp_client = Some(client);
        self.connected = true;
        self
    }

    /// 设置 SFTP 客户端
    pub fn set_client(&mut self, client: Option<Arc<SftpClient>>, cx: &mut Context<Self>) {
        self.sftp_client = client;
        self.connected = self.sftp_client.is_some();
        if self.connected {
            self.error_message = None;
        }
        cx.notify();
    }

    /// 设置主机信息
    pub fn set_host_info(
        &mut self,
        hostname: Option<String>,
        username: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.hostname = hostname;
        self.username = username;

        // 更新路径栏的主目录
        if let Some(user) = &self.username {
            self.path_bar.update(cx, |path_bar, cx| {
                path_bar.set_home_path(Some(format!("/home/{}", user)), cx);
            });
        }

        cx.notify();
    }

    /// 设置视图模式
    pub fn set_mode(&mut self, mode: SftpViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    /// 设置是否显示传输队列
    pub fn set_show_transfer_queue(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_transfer_queue = show;
        cx.notify();
    }

    /// 切换传输队列显示
    pub fn toggle_transfer_queue(&mut self, cx: &mut Context<Self>) {
        self.show_transfer_queue = !self.show_transfer_queue;
        cx.notify();
    }

    /// 获取当前路径
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// 是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// 导航到路径
    pub fn navigate_to(&mut self, path: impl Into<String>, cx: &mut Context<Self>) {
        let path = path.into();
        self.current_path = path.clone();

        // 更新路径栏
        self.path_bar.update(cx, |path_bar, cx| {
            path_bar.set_path(&self.current_path, cx);
        });

        // 加载目录内容
        self.load_directory(cx);
    }

    /// 刷新当前目录
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_directory(cx);
    }

    /// 加载目录内容
    fn load_directory(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.sftp_client.clone() else {
            self.error_message = Some("Not connected to SFTP server".to_string());
            cx.notify();
            return;
        };

        let path = self.current_path.clone();
        self.loading = true;
        self.error_message = None;

        // 更新文件列表状态
        self.file_list.update(cx, |file_list, cx| {
            file_list.set_loading(true, cx);
        });

        cx.notify();

        // TODO: 异步加载目录内容
        // 目前 SFTP 客户端需要异步调用，但 gpui 的 Context::spawn 有生命周期限制
        // 未来需要使用以下方式之一来解决：
        // 1. 使用 gpui 的事件系统（emit/subscribe）来异步通知结果
        // 2. 使用共享状态 + 定时器轮询
        // 3. 将 SFTP 客户端的加载结果缓存到内存中
        //
        // 临时解决方案：在后台线程中加载，通过共享状态传递结果
        let _file_list = self.file_list.clone();
        let path_clone = path.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            let result = rt.block_on(async move { client.list_dir(&path_clone).await });

            // 注意：这里无法直接更新 UI，因为跨线程
            // 结果需要通过其他方式传递给 UI 线程
            match result {
                Ok(entries) => {
                    tracing::info!("SFTP: Loaded {} entries from {}", entries.len(), path);
                    // TODO: 通过事件系统更新 file_list
                },
                Err(e) => {
                    tracing::error!("SFTP: Failed to load directory {}: {}", path, e);
                    // TODO: 通过事件系统显示错误
                },
            }
        });
    }

    /// 处理路径栏事件
    fn handle_path_bar_event(
        &mut self,
        _path_bar: Entity<PathBar>,
        event: &PathBarEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            PathBarEvent::NavigateTo(path) => {
                self.navigate_to(path.clone(), cx);
            },
            PathBarEvent::GoUp => {
                if self.current_path != "/" {
                    let parent = std::path::Path::new(&self.current_path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string());
                    self.navigate_to(parent, cx);
                }
            },
            PathBarEvent::GoBack => {
                // 路径栏内部处理历史
            },
            PathBarEvent::GoForward => {
                // 路径栏内部处理历史
            },
            PathBarEvent::Refresh => {
                self.refresh(cx);
            },
            PathBarEvent::GoHome => {
                if let Some(user) = &self.username {
                    self.navigate_to(format!("/home/{}", user), cx);
                }
            },
        }
    }

    /// 处理文件列表事件
    fn handle_file_list_event(
        &mut self,
        _file_list: Entity<FileListView>,
        event: &FileListEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FileListEvent::Selected(path) => {
                cx.emit(SftpViewEvent::FileSelected(path.clone()));
            },
            FileListEvent::Opened(path) => {
                // 检查是否为目录
                // 简单判断：如果路径以目录分隔符结尾或者没有扩展名可能是目录
                // 实际应该检查 entry_type
                self.navigate_to(path.clone(), cx);
                cx.emit(SftpViewEvent::FileOpened(path.clone()));
            },
            FileListEvent::MultiSelected(_paths) => {
                // 多选不触发事件，等待操作
            },
            FileListEvent::ContextMenu { path, position } => {
                // TODO: 显示右键菜单
                let _ = (path, position);
            },
            FileListEvent::SortChanged { column, order } => {
                // 排序在文件列表内部处理
                let _ = (column, order);
            },
            FileListEvent::UploadRequested(paths) => {
                cx.emit(SftpViewEvent::UploadRequested {
                    local_paths: paths.clone(),
                    remote_dir: self.current_path.clone(),
                });
            },
        }
    }

    /// 处理传输队列事件
    fn handle_transfer_queue_event(
        &mut self,
        _transfer_queue: Entity<TransferQueueView>,
        event: &TransferQueueEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TransferQueueEvent::PauseRequested(id) => {
                // TODO: 实现暂停逻辑
                let _ = id;
            },
            TransferQueueEvent::ResumeRequested(id) => {
                // TODO: 实现恢复逻辑
                let _ = id;
            },
            TransferQueueEvent::CancelRequested(id) => {
                // TODO: 实现取消逻辑
                let _ = id;
            },
            TransferQueueEvent::RetryRequested(id) => {
                // TODO: 实现重试逻辑
                let _ = id;
            },
            TransferQueueEvent::ClearCompleted => {
                self.transfer_queue.update(cx, |queue, cx| {
                    queue.clear_completed(cx);
                });
            },
            TransferQueueEvent::ClearAll => {
                self.transfer_queue.update(cx, |queue, cx| {
                    queue.clear_all(cx);
                });
            },
            TransferQueueEvent::OpenDestination(path) => {
                // 在文件管理器中打开
                let _ = path;
            },
        }
    }

    /// 添加传输任务到队列
    pub fn add_transfer(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        self.transfer_queue.update(cx, |queue, cx| {
            queue.add_transfer(id, progress, cx);
        });
        cx.emit(SftpViewEvent::TransferStarted(id));
    }

    /// 更新传输进度
    pub fn update_transfer_progress(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        self.transfer_queue.update(cx, |queue, cx| {
            queue.update_progress(id, progress, cx);
        });
    }

    /// 渲染工具栏
    fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let connected = self.connected;

        div()
            .w_full()
            .h(px(40.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            // 左侧：连接信息
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(if connected {
                                gpui::hsla(0.33, 0.7, 0.45, 1.0)
                            } else {
                                theme.muted_foreground
                            })
                            .child(if connected { "●" } else { "○" }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(SharedString::from(
                                self.hostname
                                    .clone()
                                    .unwrap_or_else(|| "Not connected".to_string()),
                            )),
                    )
                    .when(self.username.is_some(), |el| {
                        el.child(div().text_sm().text_color(theme.muted_foreground).child(
                            SharedString::from(format!("({})", self.username.as_ref().unwrap())),
                        ))
                    }),
            )
            // 右侧：操作按钮
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    // 上传按钮
                    .child(
                        div()
                            .id("btn-upload")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if connected {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("⬆️ Upload"),
                    )
                    // 新建文件夹按钮
                    .child(
                        div()
                            .id("btn-new-folder")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if connected {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("📁 New Folder"),
                    )
                    // 传输队列切换
                    .child(
                        div()
                            .id("btn-toggle-transfers")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if self.show_transfer_queue {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .when(self.show_transfer_queue, |el| {
                                el.bg(theme.primary.opacity(0.1))
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("📦 Transfers"),
                    )
                    // 关闭按钮
                    .child(
                        div()
                            .id("btn-close")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .hover(|el| {
                                el.bg(gpui::hsla(0.0, 0.8, 0.5, 0.2))
                                    .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                            })
                            .child("✕"),
                    ),
            )
    }

    /// 渲染未连接状态
    fn render_disconnected(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .text_color(theme.muted_foreground)
                    .child("🔌"),
            )
            .child(
                div()
                    .text_lg()
                    .text_color(theme.foreground)
                    .child("Not connected"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Connect to an SSH server to browse files"),
            )
    }

    /// 渲染主内容区域
    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let _theme = cx.theme();

        div()
            .w_full()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // 路径栏
            .child(self.path_bar.clone())
            // 文件列表
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .overflow_hidden()
                    .child(self.file_list.clone()),
            )
    }

    /// 渲染传输队列面板
    fn render_transfer_panel(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .max_h(px(250.0))
            .p_2()
            .child(self.transfer_queue.clone())
    }
}

impl EventEmitter<SftpViewEvent> for SftpView {}

impl Focusable for SftpView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SftpView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let connected = self.connected;
        let show_transfers = self.show_transfer_queue;

        div()
            .id("sftp-view")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .overflow_hidden()
            // 工具栏
            .child(self.render_toolbar(cx))
            // 主内容
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(if connected {
                        self.render_content(cx).into_any_element()
                    } else {
                        self.render_disconnected(cx).into_any_element()
                    }),
            )
            // 传输队列（可选）
            .when(show_transfers && connected, |el| {
                el.child(self.render_transfer_panel(cx))
            })
    }
}

/// 创建 SFTP 视图的便捷方法
pub fn create_sftp_view(cx: &mut Context<SftpView>) -> SftpView {
    SftpView::new(cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_view_mode_title() {
        assert_eq!(SftpViewMode::Browse.title(), "Browse");
        assert_eq!(SftpViewMode::Transfers.title(), "Transfers");
        assert_eq!(SftpViewMode::Split.title(), "Split View");
    }

    #[test]
    fn test_sftp_view_mode_icon() {
        assert_eq!(SftpViewMode::Browse.icon(), "📁");
        assert_eq!(SftpViewMode::Transfers.icon(), "📦");
        assert_eq!(SftpViewMode::Split.icon(), "⧉");
    }

    #[test]
    fn test_sftp_view_event_debug() {
        let event = SftpViewEvent::FileSelected("/test/path".to_string());
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("FileSelected"));

        let event2 = SftpViewEvent::CloseRequested;
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("CloseRequested"));
    }

    #[test]
    fn test_sftp_view_event_transfer() {
        let event = SftpViewEvent::TransferStarted(TransferTaskId(42));
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("TransferStarted"));

        let event2 = SftpViewEvent::TransferFailed {
            task_id: TransferTaskId(1),
            error: "Connection lost".to_string(),
        };
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("TransferFailed"));
        assert!(debug_str2.contains("Connection lost"));
    }

    #[test]
    fn test_sftp_view_event_download() {
        let event = SftpViewEvent::DownloadRequested {
            remote_path: "/remote/file.txt".to_string(),
            local_path: Some("/local/file.txt".to_string()),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("DownloadRequested"));
        assert!(debug_str.contains("remote/file.txt"));
    }

    #[test]
    fn test_sftp_view_event_upload() {
        let event = SftpViewEvent::UploadRequested {
            local_paths: vec!["/local/a.txt".to_string(), "/local/b.txt".to_string()],
            remote_dir: "/remote/dir".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("UploadRequested"));
        assert!(debug_str.contains("remote/dir"));
    }
}
