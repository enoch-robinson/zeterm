//! 传输队列组件
//!
//! 提供 SFTP 文件传输进度显示功能，包括：
//! - 显示所有进行中的传输任务
//! - 进度条和传输速度显示
//! - 暂停/恢复/取消按钮
//! - 传输历史记录

use std::collections::HashMap;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

use zeterm_ssh::{TransferDirection, TransferProgress, TransferState, TransferTaskId};

// ============================================================================
// 事件定义
// ============================================================================

/// 传输队列事件
#[derive(Debug, Clone)]
pub enum TransferQueueEvent {
    /// 请求暂停传输
    PauseRequested(TransferTaskId),
    /// 请求恢复传输
    ResumeRequested(TransferTaskId),
    /// 请求取消传输
    CancelRequested(TransferTaskId),
    /// 请求重试传输
    RetryRequested(TransferTaskId),
    /// 清除已完成的传输
    ClearCompleted,
    /// 清除所有传输
    ClearAll,
    /// 打开传输目标位置
    OpenDestination(String),
}

// ============================================================================
// 传输项数据
// ============================================================================

/// 传输项（UI 显示用）
#[derive(Debug, Clone)]
pub struct TransferItem {
    /// 任务 ID
    pub id: TransferTaskId,
    /// 进度信息
    pub progress: TransferProgress,
    /// 是否展开详情
    pub expanded: bool,
}

impl TransferItem {
    /// 创建新的传输项
    pub fn new(id: TransferTaskId, progress: TransferProgress) -> Self {
        Self {
            id,
            progress,
            expanded: false,
        }
    }

    /// 获取文件名（从路径中提取）
    pub fn file_name(&self) -> &str {
        let path = match self.progress.direction {
            TransferDirection::Upload => &self.progress.source_path,
            TransferDirection::Download => &self.progress.dest_path,
        };
        path.rsplit('/').next().unwrap_or(path)
    }

    /// 获取方向图标
    pub fn direction_icon(&self) -> &'static str {
        match self.progress.direction {
            TransferDirection::Upload => "⬆️",
            TransferDirection::Download => "⬇️",
        }
    }

    /// 获取状态图标
    pub fn status_icon(&self) -> &'static str {
        match self.progress.state {
            TransferState::Pending => "⏳",
            TransferState::InProgress => "🔄",
            TransferState::Paused => "⏸",
            TransferState::Completed => "✅",
            TransferState::Failed => "❌",
            TransferState::Cancelled => "🚫",
        }
    }

    /// 获取状态文本
    pub fn status_text(&self) -> &'static str {
        match self.progress.state {
            TransferState::Pending => "Pending",
            TransferState::InProgress => "In Progress",
            TransferState::Paused => "Paused",
            TransferState::Completed => "Completed",
            TransferState::Failed => "Failed",
            TransferState::Cancelled => "Cancelled",
        }
    }

    /// 是否可以暂停
    pub fn can_pause(&self) -> bool {
        matches!(self.progress.state, TransferState::InProgress)
    }

    /// 是否可以恢复
    pub fn can_resume(&self) -> bool {
        matches!(self.progress.state, TransferState::Paused)
    }

    /// 是否可以取消
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.progress.state,
            TransferState::Pending | TransferState::InProgress | TransferState::Paused
        )
    }

    /// 是否可以重试
    pub fn can_retry(&self) -> bool {
        matches!(
            self.progress.state,
            TransferState::Failed | TransferState::Cancelled
        )
    }

    /// 是否已终止
    pub fn is_terminated(&self) -> bool {
        self.progress.is_terminated()
    }
}

// ============================================================================
// 传输队列视图
// ============================================================================

/// 传输队列过滤器
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferFilter {
    /// 显示所有
    #[default]
    All,
    /// 仅显示进行中
    Active,
    /// 仅显示已完成
    Completed,
    /// 仅显示失败
    Failed,
}

impl TransferFilter {
    /// 获取过滤器标题
    pub fn title(&self) -> &'static str {
        match self {
            TransferFilter::All => "All",
            TransferFilter::Active => "Active",
            TransferFilter::Completed => "Completed",
            TransferFilter::Failed => "Failed",
        }
    }

    /// 所有过滤器
    pub fn all() -> &'static [TransferFilter] {
        &[
            TransferFilter::All,
            TransferFilter::Active,
            TransferFilter::Completed,
            TransferFilter::Failed,
        ]
    }

    /// 检查传输项是否匹配过滤器
    pub fn matches(&self, item: &TransferItem) -> bool {
        match self {
            TransferFilter::All => true,
            TransferFilter::Active => matches!(
                item.progress.state,
                TransferState::Pending | TransferState::InProgress | TransferState::Paused
            ),
            TransferFilter::Completed => matches!(item.progress.state, TransferState::Completed),
            TransferFilter::Failed => matches!(
                item.progress.state,
                TransferState::Failed | TransferState::Cancelled
            ),
        }
    }
}

/// 传输队列视图
pub struct TransferQueueView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 传输项列表
    transfers: HashMap<TransferTaskId, TransferItem>,
    /// 传输顺序（用于排序显示）
    transfer_order: Vec<TransferTaskId>,
    /// 当前过滤器
    filter: TransferFilter,
    /// 是否折叠
    collapsed: bool,
    /// 是否显示详情
    show_details: bool,
    /// hover 的传输 ID
    hovered_id: Option<TransferTaskId>,
}

impl TransferQueueView {
    /// 创建新的传输队列视图
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            transfers: HashMap::new(),
            transfer_order: Vec::new(),
            filter: TransferFilter::All,
            collapsed: false,
            show_details: false,
            hovered_id: None,
        }
    }

    /// 添加传输任务
    pub fn add_transfer(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        let item = TransferItem::new(id, progress);
        self.transfers.insert(id, item);
        self.transfer_order.push(id);
        cx.notify();
    }

    /// 更新传输进度
    pub fn update_progress(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        if let Some(item) = self.transfers.get_mut(&id) {
            item.progress = progress;
            cx.notify();
        }
    }

    /// 移除传输任务
    pub fn remove_transfer(&mut self, id: TransferTaskId, cx: &mut Context<Self>) {
        self.transfers.remove(&id);
        self.transfer_order.retain(|&i| i != id);
        cx.notify();
    }

    /// 清除已完成的传输
    pub fn clear_completed(&mut self, cx: &mut Context<Self>) {
        let completed_ids: Vec<TransferTaskId> = self
            .transfers
            .iter()
            .filter(|(_, item)| item.is_terminated())
            .map(|(&id, _)| id)
            .collect();

        for id in completed_ids {
            self.transfers.remove(&id);
            self.transfer_order.retain(|&i| i != id);
        }
        cx.notify();
    }

    /// 清除所有传输
    pub fn clear_all(&mut self, cx: &mut Context<Self>) {
        self.transfers.clear();
        self.transfer_order.clear();
        cx.notify();
    }

    /// 设置过滤器
    pub fn set_filter(&mut self, filter: TransferFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    /// 设置折叠状态
    pub fn set_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.collapsed = collapsed;
        cx.notify();
    }

    /// 切换折叠状态
    pub fn toggle_collapsed(&mut self, cx: &mut Context<Self>) {
        self.collapsed = !self.collapsed;
        cx.notify();
    }

    /// 获取传输项数量
    pub fn transfer_count(&self) -> usize {
        self.transfers.len()
    }

    /// 获取活跃传输数量
    pub fn active_count(&self) -> usize {
        self.transfers
            .values()
            .filter(|item| !item.is_terminated())
            .count()
    }

    /// 获取过滤后的传输列表
    fn filtered_transfers(&self) -> Vec<&TransferItem> {
        self.transfer_order
            .iter()
            .filter_map(|id| self.transfers.get(id))
            .filter(|item| self.filter.matches(item))
            .collect()
    }

    /// 获取总进度百分比
    pub fn total_progress(&self) -> f64 {
        let active: Vec<_> = self
            .transfers
            .values()
            .filter(|item| !item.is_terminated())
            .collect();

        if active.is_empty() {
            return 100.0;
        }

        let total_bytes: u64 = active.iter().map(|item| item.progress.total_bytes).sum();
        let transferred_bytes: u64 = active
            .iter()
            .map(|item| item.progress.transferred_bytes)
            .sum();

        if total_bytes == 0 {
            0.0
        } else {
            (transferred_bytes as f64 / total_bytes as f64) * 100.0
        }
    }

    /// 渲染标题栏
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let active = self.active_count();
        let total = self.transfer_count();
        let collapsed = self.collapsed;

        div()
            .w_full()
            .h(px(32.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            // 左侧：标题和计数
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("toggle-collapse")
                            .cursor_pointer()
                            .text_color(theme.foreground)
                            .child(if collapsed { "▶" } else { "▼" }),
                    )
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("Transfers"),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_px()
                            .rounded_full()
                            .bg(if active > 0 {
                                theme.primary.opacity(0.2)
                            } else {
                                theme.muted.opacity(0.2)
                            })
                            .text_xs()
                            .text_color(if active > 0 {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .child(SharedString::from(format!(
                                "{}{}",
                                active,
                                if active != total {
                                    format!("/{}", total)
                                } else {
                                    String::new()
                                }
                            ))),
                    ),
            )
            // 右侧：操作按钮
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    // 清除已完成按钮
                    .child(
                        div()
                            .id("btn-clear-completed")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("Clear Done"),
                    ),
            )
    }

    /// 渲染过滤器标签
    fn render_filters(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let current_filter = self.filter;

        div()
            .w_full()
            .h(px(28.0))
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border.opacity(0.5))
            .text_xs()
            .children(TransferFilter::all().iter().map(|&filter| {
                let is_active = filter == current_filter;
                div()
                    .id(SharedString::from(format!("filter-{:?}", filter)))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if is_active {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    })
                    .when(is_active, |el| el.bg(theme.primary.opacity(0.1)))
                    .hover(|el| el.bg(theme.secondary))
                    .child(filter.title())
            }))
    }

    /// 渲染传输项
    fn render_transfer_item(&self, item: &TransferItem, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let progress = &item.progress;
        let percentage = progress.percentage();
        let is_hovered = self.hovered_id == Some(item.id);

        // 状态颜色
        let status_color = match progress.state {
            TransferState::Pending => theme.muted_foreground,
            TransferState::InProgress => theme.primary,
            TransferState::Paused => gpui::hsla(0.14, 0.9, 0.5, 1.0), // 黄色
            TransferState::Completed => gpui::hsla(0.33, 0.7, 0.45, 1.0), // 绿色
            TransferState::Failed => gpui::hsla(0.0, 0.8, 0.5, 1.0),  // 红色
            TransferState::Cancelled => theme.muted_foreground,
        };

        div()
            .id(SharedString::from(format!("transfer-{}", item.id)))
            .w_full()
            .px_3()
            .py_2()
            .bg(if is_hovered {
                theme.secondary
            } else {
                theme.background
            })
            .border_b_1()
            .border_color(theme.border.opacity(0.3))
            .hover(|el| el.bg(theme.secondary))
            .flex()
            .flex_col()
            .gap_1()
            // 第一行：图标 + 文件名 + 操作按钮
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .overflow_hidden()
                            .child(div().text_sm().child(item.direction_icon()))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(SharedString::from(item.file_name().to_string())),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(status_color)
                                    .child(item.status_icon()),
                            ),
                    )
                    // 操作按钮
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1()
                            // 暂停/恢复按钮
                            .when(item.can_pause() || item.can_resume(), |el| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("btn-pause-{}", item.id)))
                                        .px_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|el| el.bg(theme.muted.opacity(0.2)))
                                        .child(if item.can_pause() { "⏸" } else { "▶" }),
                                )
                            })
                            // 取消按钮
                            .when(item.can_cancel(), |el| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("btn-cancel-{}", item.id)))
                                        .px_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|el| {
                                            el.bg(gpui::hsla(0.0, 0.8, 0.5, 0.2))
                                                .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                                        })
                                        .child("✕"),
                                )
                            })
                            // 重试按钮
                            .when(item.can_retry(), |el| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("btn-retry-{}", item.id)))
                                        .px_1()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .hover(|el| el.bg(theme.muted.opacity(0.2)))
                                        .child("🔄"),
                                )
                            }),
                    ),
            )
            // 第二行：进度条
            .when(
                !matches!(
                    progress.state,
                    TransferState::Completed | TransferState::Cancelled
                ),
                |el| {
                    el.child(
                        div()
                            .w_full()
                            .h(px(4.0))
                            .rounded_full()
                            .bg(theme.muted.opacity(0.3))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(status_color)
                                    .w(gpui::relative(percentage as f32 / 100.0)),
                            ),
                    )
                },
            )
            // 第三行：详细信息
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(
                        // 进度信息
                        div().child(SharedString::from(progress.formatted_progress())),
                    )
                    .child(
                        // 速度和剩余时间
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .when(progress.speed_bps > 0, |el| {
                                el.child(SharedString::from(progress.formatted_speed()))
                            })
                            .when(progress.estimated_remaining_secs().is_some(), |el| {
                                el.child(SharedString::from(format!(
                                    "ETA: {}",
                                    progress.formatted_eta()
                                )))
                            }),
                    ),
            )
            // 错误信息（如果有）
            .when(progress.error.is_some(), |el| {
                el.child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                        .child(SharedString::from(
                            progress.error.clone().unwrap_or_default(),
                        )),
                )
            })
    }

    /// 渲染空状态
    fn render_empty(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h(px(80.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_1()
            .text_color(theme.muted_foreground)
            .child(div().text_2xl().child("📦"))
            .child(div().text_xs().child("No transfers"))
    }

    /// 渲染传输列表
    fn render_transfers(&self, cx: &Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_transfers();

        div().w_full().flex_1().overflow_y_hidden().children(
            filtered
                .iter()
                .map(|item| self.render_transfer_item(item, cx)),
        )
    }

    /// 渲染总计进度栏
    fn render_total_progress(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let active = self.active_count();
        let progress = self.total_progress();

        div()
            .w_full()
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .px_3()
            .gap_2()
            .bg(theme.secondary)
            .border_t_1()
            .border_color(theme.border)
            .text_xs()
            .text_color(theme.muted_foreground)
            .when(active > 0, |el| {
                el.child(
                    div()
                        .flex_1()
                        .h(px(3.0))
                        .rounded_full()
                        .bg(theme.muted.opacity(0.3))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(theme.primary)
                                .w(gpui::relative(progress as f32 / 100.0)),
                        ),
                )
                .child(SharedString::from(format!("{:.1}%", progress)))
            })
            .when(active == 0, |el| {
                el.child(div().flex_1().child(if self.transfer_count() > 0 {
                    "All transfers completed"
                } else {
                    "No active transfers"
                }))
            })
    }
}

impl EventEmitter<TransferQueueEvent> for TransferQueueView {}

impl Focusable for TransferQueueView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TransferQueueView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let collapsed = self.collapsed;
        let is_empty = self.filtered_transfers().is_empty();

        div()
            .id("sftp-transfer-queue")
            .w_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .overflow_hidden()
            // 标题栏
            .child(self.render_header(cx))
            // 内容区域（可折叠）
            .when(!collapsed, |el| {
                el
                    // 过滤器
                    .child(self.render_filters(cx))
                    // 传输列表
                    .child(
                        div()
                            .w_full()
                            .min_h(px(100.0))
                            .max_h(px(300.0))
                            .flex()
                            .flex_col()
                            .child(if is_empty {
                                self.render_empty(cx).into_any_element()
                            } else {
                                self.render_transfers(cx).into_any_element()
                            }),
                    )
                    // 总计进度栏
                    .child(self.render_total_progress(cx))
            })
    }
}

/// 创建传输队列视图的便捷方法
pub fn create_transfer_queue_view(cx: &mut Context<TransferQueueView>) -> TransferQueueView {
    TransferQueueView::new(cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_progress(
        direction: TransferDirection,
        state: TransferState,
    ) -> TransferProgress {
        let mut progress =
            TransferProgress::new(direction, "/local/file.txt", "/remote/file.txt", 1000);
        progress.state = state;
        progress.transferred_bytes = 500;
        progress.speed_bps = 1024;
        progress
    }

    #[test]
    fn test_transfer_item_file_name() {
        let progress = create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let item = TransferItem::new(TransferTaskId(1), progress);
        assert_eq!(item.file_name(), "file.txt");
    }

    #[test]
    fn test_transfer_item_direction_icon() {
        let up_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let up_item = TransferItem::new(TransferTaskId(1), up_progress);
        assert_eq!(up_item.direction_icon(), "⬆️");

        let down_progress =
            create_test_progress(TransferDirection::Download, TransferState::InProgress);
        let down_item = TransferItem::new(TransferTaskId(2), down_progress);
        assert_eq!(down_item.direction_icon(), "⬇️");
    }

    #[test]
    fn test_transfer_item_status_icon() {
        let pending = create_test_progress(TransferDirection::Upload, TransferState::Pending);
        assert_eq!(
            TransferItem::new(TransferTaskId(1), pending).status_icon(),
            "⏳"
        );

        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        assert_eq!(
            TransferItem::new(TransferTaskId(2), in_progress).status_icon(),
            "🔄"
        );

        let completed = create_test_progress(TransferDirection::Upload, TransferState::Completed);
        assert_eq!(
            TransferItem::new(TransferTaskId(3), completed).status_icon(),
            "✅"
        );

        let failed = create_test_progress(TransferDirection::Upload, TransferState::Failed);
        assert_eq!(
            TransferItem::new(TransferTaskId(4), failed).status_icon(),
            "❌"
        );
    }

    #[test]
    fn test_transfer_item_can_pause() {
        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let item = TransferItem::new(TransferTaskId(1), in_progress);
        assert!(item.can_pause());

        let paused = create_test_progress(TransferDirection::Upload, TransferState::Paused);
        let item2 = TransferItem::new(TransferTaskId(2), paused);
        assert!(!item2.can_pause());
    }

    #[test]
    fn test_transfer_item_can_resume() {
        let paused = create_test_progress(TransferDirection::Upload, TransferState::Paused);
        let item = TransferItem::new(TransferTaskId(1), paused);
        assert!(item.can_resume());

        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let item2 = TransferItem::new(TransferTaskId(2), in_progress);
        assert!(!item2.can_resume());
    }

    #[test]
    fn test_transfer_item_can_cancel() {
        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let item = TransferItem::new(TransferTaskId(1), in_progress);
        assert!(item.can_cancel());

        let completed = create_test_progress(TransferDirection::Upload, TransferState::Completed);
        let item2 = TransferItem::new(TransferTaskId(2), completed);
        assert!(!item2.can_cancel());
    }

    #[test]
    fn test_transfer_item_can_retry() {
        let failed = create_test_progress(TransferDirection::Upload, TransferState::Failed);
        let item = TransferItem::new(TransferTaskId(1), failed);
        assert!(item.can_retry());

        let cancelled = create_test_progress(TransferDirection::Upload, TransferState::Cancelled);
        let item2 = TransferItem::new(TransferTaskId(2), cancelled);
        assert!(item2.can_retry());

        let completed = create_test_progress(TransferDirection::Upload, TransferState::Completed);
        let item3 = TransferItem::new(TransferTaskId(3), completed);
        assert!(!item3.can_retry());
    }

    #[test]
    fn test_transfer_item_is_terminated() {
        let completed = create_test_progress(TransferDirection::Upload, TransferState::Completed);
        assert!(TransferItem::new(TransferTaskId(1), completed).is_terminated());

        let failed = create_test_progress(TransferDirection::Upload, TransferState::Failed);
        assert!(TransferItem::new(TransferTaskId(2), failed).is_terminated());

        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        assert!(!TransferItem::new(TransferTaskId(3), in_progress).is_terminated());
    }

    #[test]
    fn test_transfer_filter_matches() {
        let in_progress =
            create_test_progress(TransferDirection::Upload, TransferState::InProgress);
        let item_active = TransferItem::new(TransferTaskId(1), in_progress);

        let completed = create_test_progress(TransferDirection::Upload, TransferState::Completed);
        let item_completed = TransferItem::new(TransferTaskId(2), completed);

        let failed = create_test_progress(TransferDirection::Upload, TransferState::Failed);
        let item_failed = TransferItem::new(TransferTaskId(3), failed);

        // All filter
        assert!(TransferFilter::All.matches(&item_active));
        assert!(TransferFilter::All.matches(&item_completed));
        assert!(TransferFilter::All.matches(&item_failed));

        // Active filter
        assert!(TransferFilter::Active.matches(&item_active));
        assert!(!TransferFilter::Active.matches(&item_completed));
        assert!(!TransferFilter::Active.matches(&item_failed));

        // Completed filter
        assert!(!TransferFilter::Completed.matches(&item_active));
        assert!(TransferFilter::Completed.matches(&item_completed));
        assert!(!TransferFilter::Completed.matches(&item_failed));

        // Failed filter
        assert!(!TransferFilter::Failed.matches(&item_active));
        assert!(!TransferFilter::Failed.matches(&item_completed));
        assert!(TransferFilter::Failed.matches(&item_failed));
    }

    #[test]
    fn test_transfer_filter_title() {
        assert_eq!(TransferFilter::All.title(), "All");
        assert_eq!(TransferFilter::Active.title(), "Active");
        assert_eq!(TransferFilter::Completed.title(), "Completed");
        assert_eq!(TransferFilter::Failed.title(), "Failed");
    }

    #[test]
    fn test_transfer_filter_all() {
        let all = TransferFilter::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&TransferFilter::All));
        assert!(all.contains(&TransferFilter::Active));
        assert!(all.contains(&TransferFilter::Completed));
        assert!(all.contains(&TransferFilter::Failed));
    }

    #[test]
    fn test_transfer_queue_event_debug() {
        let event = TransferQueueEvent::CancelRequested(TransferTaskId(42));
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("CancelRequested"));

        let event2 = TransferQueueEvent::ClearCompleted;
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("ClearCompleted"));
    }
}
