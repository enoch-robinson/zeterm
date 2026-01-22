//! 删除确认对话框
//!
//! 在删除主机时显示确认对话框，警告用户此操作将删除所有关联的连接历史记录。

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Window, div, prelude::*,
};
use gpui_component::ActiveTheme;

use zeterm_core::entities::HostId;

// ============================================================================
// 事件定义
// ============================================================================

/// 删除确认事件
#[derive(Debug, Clone)]
pub enum DeleteConfirmEvent {
    /// 用户确认删除
    Confirmed(DeleteTarget),
    /// 用户取消删除
    Cancelled,
}

/// 删除目标
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    /// 删除单个主机
    Host { id: HostId, name: String },
    /// 删除多个主机
    Hosts { ids: Vec<HostId>, count: usize },
}

impl DeleteTarget {
    /// 创建单个主机删除目标
    pub fn host(id: HostId, name: impl Into<String>) -> Self {
        Self::Host {
            id,
            name: name.into(),
        }
    }

    /// 创建多个主机删除目标
    pub fn hosts(ids: Vec<HostId>) -> Self {
        let count = ids.len();
        Self::Hosts { ids, count }
    }

    /// 获取主机 ID（仅适用于单个主机）
    pub fn host_id(&self) -> Option<HostId> {
        match self {
            Self::Host { id, .. } => Some(*id),
            Self::Hosts { .. } => None,
        }
    }

    /// 获取主机名称（仅适用于单个主机）
    pub fn host_name(&self) -> Option<&str> {
        match self {
            Self::Host { name, .. } => Some(name),
            Self::Hosts { .. } => None,
        }
    }

    /// 获取描述文本
    pub fn description(&self) -> String {
        match self {
            Self::Host { name, .. } => {
                format!("你确定要删除主机 \"{}\" 吗？", name)
            },
            Self::Hosts { count, .. } => {
                format!("你确定要删除选中的 {} 个主机吗？", count)
            },
        }
    }

    /// 获取标题
    pub fn title(&self) -> &'static str {
        match self {
            Self::Host { .. } => "删除主机",
            Self::Hosts { .. } => "删除多个主机",
        }
    }
}

// ============================================================================
// 删除确认对话框
// ============================================================================

/// 删除确认对话框
///
/// 在用户删除主机前显示警告信息，要求确认。
pub struct DeleteConfirmDialog {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 删除目标
    target: DeleteTarget,
}

impl DeleteConfirmDialog {
    /// 创建新的删除确认对话框
    pub fn new(target: DeleteTarget, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            target,
        }
    }

    /// 创建删除单个主机的确认对话框
    pub fn for_host(id: HostId, name: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self::new(DeleteTarget::host(id, name), cx)
    }

    /// 获取删除目标
    pub fn target(&self) -> &DeleteTarget {
        &self.target
    }

    /// 确认删除
    fn confirm(&mut self, cx: &mut Context<Self>) {
        cx.emit(DeleteConfirmEvent::Confirmed(self.target.clone()));
    }

    /// 取消删除
    fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(DeleteConfirmEvent::Cancelled);
    }

    /// 处理按键事件
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" => self.cancel(cx),
            "enter" => self.confirm(cx),
            _ => {},
        }
    }

    /// 渲染标题区域
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .items_center()
            .gap_3()
            .mb_4()
            // 警告图标
            .child(
                div()
                    .w_10()
                    .h_10()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(gpui::hsla(0.0, 0.7, 0.5, 0.1))
                    .text_color(gpui::hsla(0.0, 0.7, 0.5, 1.0))
                    .text_xl()
                    .child("⚠️"),
            )
            // 标题
            .child(
                div().flex_1().child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .child(self.target.title()),
                ),
            )
    }

    /// 渲染描述区域
    fn render_description(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .mb_4()
            // 主要描述
            .child(
                div()
                    .text_color(theme.foreground)
                    .mb_2()
                    .child(self.target.description()),
            )
            // 警告信息
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(gpui::hsla(0.0, 0.7, 0.5, 0.05))
                    .border_1()
                    .border_color(gpui::hsla(0.0, 0.7, 0.5, 0.2))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(div().text_color(gpui::hsla(0.0, 0.7, 0.5, 1.0)).child("⚠"))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        "此操作不可撤销。删除主机将同时删除所有关联的连接历史记录。",
                                    ),
                            ),
                    ),
            )
    }

    /// 渲染按钮区域
    fn render_buttons(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .justify_end()
            .gap_2()
            .mt_4()
            // 取消按钮
            .child(
                div()
                    .id("cancel-button")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.muted))
                    .child("取消"),
            )
            // 删除按钮
            .child(
                div()
                    .id("delete-button")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(gpui::hsla(0.0, 0.7, 0.5, 1.0))
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .hover(|el| el.bg(gpui::hsla(0.0, 0.8, 0.4, 1.0)))
                    .child("删除"),
            )
    }
}

impl EventEmitter<DeleteConfirmEvent> for DeleteConfirmDialog {}

impl Focusable for DeleteConfirmDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DeleteConfirmDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 遮罩层 + 对话框
        div()
            .id("delete-confirm-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(
                // 对话框容器
                div()
                    .id("delete-confirm-dialog")
                    .w_96()
                    .p_6()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .track_focus(&self.focus_handle)
                    // 阻止点击事件冒泡到遮罩层
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    // 头部
                    .child(self.render_header(cx))
                    // 描述
                    .child(self.render_description(cx))
                    // 按钮
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .mt_4()
                            // 取消按钮
                            .child(
                                div()
                                    .id("cancel-button")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .text_color(theme.foreground)
                                    .cursor_pointer()
                                    .hover(|el| el.bg(theme.muted))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.cancel(cx);
                                        }),
                                    )
                                    .child("取消"),
                            )
                            // 删除按钮
                            .child(
                                div()
                                    .id("delete-button")
                                    .px_4()
                                    .py_2()
                                    .rounded_md()
                                    .bg(gpui::hsla(0.0, 0.7, 0.5, 1.0))
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|el| el.bg(gpui::hsla(0.0, 0.8, 0.4, 1.0)))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.confirm(cx);
                                        }),
                                    )
                                    .child("删除"),
                            ),
                    ),
            )
    }
}

/// 创建删除确认对话框的便捷函数
pub fn create_delete_confirm_dialog(
    target: DeleteTarget,
    cx: &mut Context<DeleteConfirmDialog>,
) -> DeleteConfirmDialog {
    DeleteConfirmDialog::new(target, cx)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_target_host() {
        let target = DeleteTarget::host(1, "test-host");
        assert_eq!(target.host_id(), Some(1));
        assert_eq!(target.host_name(), Some("test-host"));
        assert_eq!(target.title(), "删除主机");
        assert!(target.description().contains("test-host"));
    }

    #[test]
    fn test_delete_target_hosts() {
        let target = DeleteTarget::hosts(vec![1, 2, 3]);
        assert_eq!(target.host_id(), None);
        assert_eq!(target.host_name(), None);
        assert_eq!(target.title(), "删除多个主机");
        assert!(target.description().contains("3"));
    }

    #[test]
    fn test_delete_confirm_event_debug() {
        let event = DeleteConfirmEvent::Cancelled;
        assert!(format!("{:?}", event).contains("Cancelled"));

        let event = DeleteConfirmEvent::Confirmed(DeleteTarget::host(1, "test"));
        assert!(format!("{:?}", event).contains("Confirmed"));
    }
}
