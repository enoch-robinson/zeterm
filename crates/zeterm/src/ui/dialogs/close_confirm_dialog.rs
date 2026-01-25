//! 关闭确认对话框
//!
//! 当用户尝试关闭有活动连接的 Tab 或应用程序时显示确认对话框。
//!
//! # 功能
//!
//! - 警告用户有活动的 SSH 连接
//! - 列出受影响的连接
//! - 提供取消和强制关闭选项
//!
//! # 示例
//!
//! ```ignore
//! use zeterm::ui::dialogs::{CloseConfirmDialog, CloseConfirmEvent};
//!
//! let dialog = cx.new(|cx| CloseConfirmDialog::new(
//!     CloseConfirmType::CloseTab,
//!     vec!["user@server1".to_string(), "user@server2".to_string()],
//!     cx,
//! ));
//!
//! cx.subscribe(&dialog, |this, _, event, cx| {
//!     match event {
//!         CloseConfirmEvent::Confirmed => { /* 执行关闭 */ }
//!         CloseConfirmEvent::Cancelled => { /* 取消操作 */ }
//!     }
//! });
//! ```

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

// ============================================================================
// 事件定义
// ============================================================================

/// 关闭确认对话框事件
#[derive(Debug, Clone)]
pub enum CloseConfirmEvent {
    /// 用户确认关闭
    Confirmed,
    /// 用户取消操作
    Cancelled,
}

// ============================================================================
// 关闭类型
// ============================================================================

/// 关闭类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseConfirmType {
    /// 关闭单个 Tab
    CloseTab,
    /// 关闭多个 Tab
    CloseTabs,
    /// 关闭应用程序
    CloseApplication,
    /// 关闭除当前 Tab 外的所有 Tab
    CloseOtherTabs,
}

impl CloseConfirmType {
    /// 获取标题
    pub fn title(&self) -> &'static str {
        match self {
            CloseConfirmType::CloseTab => "Close Tab?",
            CloseConfirmType::CloseTabs => "Close Tabs?",
            CloseConfirmType::CloseApplication => "Quit Application?",
            CloseConfirmType::CloseOtherTabs => "Close Other Tabs?",
        }
    }

    /// 获取描述
    pub fn description(&self, count: usize) -> String {
        match self {
            CloseConfirmType::CloseTab => {
                "This tab has an active SSH connection. Closing it will disconnect the session."
                    .to_string()
            },
            CloseConfirmType::CloseTabs => {
                format!(
                    "{} tabs have active SSH connections. Closing them will disconnect all sessions.",
                    count
                )
            },
            CloseConfirmType::CloseApplication => {
                format!(
                    "There {} {} active SSH connection{}. Quitting will disconnect all sessions.",
                    if count == 1 { "is" } else { "are" },
                    count,
                    if count == 1 { "" } else { "s" }
                )
            },
            CloseConfirmType::CloseOtherTabs => {
                format!(
                    "{} other tab{} {} active SSH connections. Closing them will disconnect these sessions.",
                    count,
                    if count == 1 { "" } else { "s" },
                    if count == 1 { "has" } else { "have" }
                )
            },
        }
    }

    /// 获取确认按钮文本
    pub fn confirm_button_text(&self) -> &'static str {
        match self {
            CloseConfirmType::CloseTab => "Close Tab",
            CloseConfirmType::CloseTabs => "Close All",
            CloseConfirmType::CloseApplication => "Quit",
            CloseConfirmType::CloseOtherTabs => "Close Others",
        }
    }

    /// 获取图标
    pub fn icon(&self) -> &'static str {
        match self {
            CloseConfirmType::CloseTab => "⚠️",
            CloseConfirmType::CloseTabs => "⚠️",
            CloseConfirmType::CloseApplication => "🚪",
            CloseConfirmType::CloseOtherTabs => "⚠️",
        }
    }
}

// ============================================================================
// 关闭确认对话框
// ============================================================================

/// 关闭确认对话框
pub struct CloseConfirmDialog {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 关闭类型
    close_type: CloseConfirmType,
    /// 受影响的连接列表
    affected_connections: Vec<String>,
    /// 是否显示连接列表详情
    show_details: bool,
    /// hover 的按钮
    hovered_button: Option<&'static str>,
}

impl CloseConfirmDialog {
    /// 创建新的关闭确认对话框
    pub fn new(
        close_type: CloseConfirmType,
        affected_connections: Vec<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            close_type,
            affected_connections,
            show_details: false,
            hovered_button: None,
        }
    }

    /// 创建关闭单个 Tab 的确认对话框
    pub fn close_tab(connection_name: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self::new(CloseConfirmType::CloseTab, vec![connection_name.into()], cx)
    }

    /// 创建关闭应用程序的确认对话框
    pub fn close_application(affected_connections: Vec<String>, cx: &mut Context<Self>) -> Self {
        Self::new(CloseConfirmType::CloseApplication, affected_connections, cx)
    }

    /// 获取受影响的连接数量
    pub fn connection_count(&self) -> usize {
        self.affected_connections.len()
    }

    /// 切换详情显示
    pub fn toggle_details(&mut self, cx: &mut Context<Self>) {
        self.show_details = !self.show_details;
        cx.notify();
    }

    /// 确认关闭
    fn confirm(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseConfirmEvent::Confirmed);
    }

    /// 取消操作
    fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(CloseConfirmEvent::Cancelled);
    }

    /// 处理键盘事件
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => {
                self.cancel(cx);
            },
            "enter" => {
                // Enter 默认取消（更安全）
                self.cancel(cx);
            },
            _ => {},
        }
    }

    /// 渲染标题区域
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .pb_3()
            .border_b_1()
            .border_color(theme.border)
            // 图标
            .child(div().text_2xl().child(self.close_type.icon()))
            // 标题
            .child(
                div()
                    .flex_1()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child(self.close_type.title()),
            )
    }

    /// 渲染描述区域
    fn render_description(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let count = self.connection_count();

        div()
            .w_full()
            .py_3()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(self.close_type.description(count)))
    }

    /// 渲染连接列表
    fn render_connection_list(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let show_details = self.show_details;
        let connections = self.affected_connections.clone();
        let count = connections.len();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            // 展开/折叠按钮
            .when(count > 0, |el| {
                el.child(
                    div()
                        .id("toggle-details")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(theme.primary)
                        .child(if show_details { "▼" } else { "▶" })
                        .child(SharedString::from(format!(
                            "{} Active Connection{}",
                            count,
                            if count == 1 { "" } else { "s" }
                        ))),
                )
            })
            // 连接列表（可折叠）
            .when(show_details && count > 0, |el| {
                el.child(
                    div()
                        .w_full()
                        .max_h(px(120.0))
                        .overflow_y_hidden()
                        .p_2()
                        .rounded_md()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .children(connections.into_iter().map(|conn| {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .py_1()
                                .text_xs()
                                .text_color(theme.foreground)
                                .child(
                                    div()
                                        .text_color(gpui::hsla(0.33, 0.7, 0.45, 1.0))
                                        .child("●"),
                                )
                                .child(SharedString::from(conn))
                        })),
                )
            })
    }

    /// 渲染按钮区域
    fn render_buttons(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap_2()
            .pt_3()
            .border_t_1()
            .border_color(theme.border)
            // 取消按钮
            .child(
                div()
                    .id("btn-cancel")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .hover(|el| el.bg(theme.muted))
                    .child("Cancel"),
            )
            // 确认按钮（危险操作）
            .child(
                div()
                    .id("btn-confirm")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(gpui::white())
                    .bg(gpui::hsla(0.0, 0.7, 0.5, 1.0)) // 红色背景
                    .hover(|el| el.bg(gpui::hsla(0.0, 0.8, 0.4, 1.0)))
                    .child(self.close_type.confirm_button_text()),
            )
    }
}

impl EventEmitter<CloseConfirmEvent> for CloseConfirmDialog {}

impl Focusable for CloseConfirmDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CloseConfirmDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 背景遮罩
        div()
            .id("close-confirm-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.5))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key_down(event, cx);
            }))
            // 点击遮罩关闭
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.cancel(cx);
                }),
            )
            // 对话框
            .child(
                div()
                    .id("close-confirm-dialog")
                    .w(px(400.0))
                    .max_w(px(500.0))
                    .p_4()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_2()
                    // 阻止点击事件冒泡到遮罩
                    .on_mouse_down(MouseButton::Left, |_, _window, _cx| {
                        // 不做任何事，只是阻止冒泡
                    })
                    // 标题
                    .child(self.render_header(cx))
                    // 描述
                    .child(self.render_description(cx))
                    // 连接列表
                    .child(
                        div()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_details(cx);
                                }),
                            )
                            .child(self.render_connection_list(cx)),
                    )
                    // 按钮
                    .child(self.render_buttons(cx)),
            )
    }
}

/// 创建关闭确认对话框的便捷方法
pub fn create_close_confirm_dialog(
    close_type: CloseConfirmType,
    affected_connections: Vec<String>,
    cx: &mut Context<CloseConfirmDialog>,
) -> CloseConfirmDialog {
    CloseConfirmDialog::new(close_type, affected_connections, cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_close_confirm_type_title() {
        assert_eq!(CloseConfirmType::CloseTab.title(), "Close Tab?");
        assert_eq!(CloseConfirmType::CloseTabs.title(), "Close Tabs?");
        assert_eq!(
            CloseConfirmType::CloseApplication.title(),
            "Quit Application?"
        );
        assert_eq!(
            CloseConfirmType::CloseOtherTabs.title(),
            "Close Other Tabs?"
        );
    }

    #[test]
    fn test_close_confirm_type_description() {
        let desc = CloseConfirmType::CloseTab.description(1);
        assert!(desc.contains("active SSH connection"));

        let desc2 = CloseConfirmType::CloseTabs.description(3);
        assert!(desc2.contains("3 tabs"));

        let desc3 = CloseConfirmType::CloseApplication.description(1);
        assert!(desc3.contains("is 1 active"));

        let desc4 = CloseConfirmType::CloseApplication.description(2);
        assert!(desc4.contains("are 2 active"));
    }

    #[test]
    fn test_close_confirm_type_confirm_button_text() {
        assert_eq!(
            CloseConfirmType::CloseTab.confirm_button_text(),
            "Close Tab"
        );
        assert_eq!(
            CloseConfirmType::CloseTabs.confirm_button_text(),
            "Close All"
        );
        assert_eq!(
            CloseConfirmType::CloseApplication.confirm_button_text(),
            "Quit"
        );
        assert_eq!(
            CloseConfirmType::CloseOtherTabs.confirm_button_text(),
            "Close Others"
        );
    }

    #[test]
    fn test_close_confirm_type_icon() {
        assert_eq!(CloseConfirmType::CloseTab.icon(), "⚠️");
        assert_eq!(CloseConfirmType::CloseApplication.icon(), "🚪");
    }

    #[test]
    fn test_close_confirm_event_debug() {
        let confirmed = CloseConfirmEvent::Confirmed;
        let cancelled = CloseConfirmEvent::Cancelled;

        assert!(format!("{:?}", confirmed).contains("Confirmed"));
        assert!(format!("{:?}", cancelled).contains("Cancelled"));
    }
}
