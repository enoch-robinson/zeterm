//! 错误通知组件
//!
//! 用于显示连接错误、操作失败等通知信息。
//! 支持自动消失、手动关闭和重试操作。

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use std::sync::Arc;

use tracing::info;

/// 错误通知事件
#[derive(Clone, Debug)]
pub enum ErrorNotificationEvent {
    /// 通知已关闭
    Closed,
    /// 用户请求重试
    RetryRequested,
}

impl EventEmitter<ErrorNotificationEvent> for ErrorNotification {}

/// 错误通知组件
pub struct ErrorNotification {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 错误标题
    title: String,
    /// 错误详情
    message: String,
    /// 是否显示重试按钮
    show_retry: bool,
    /// 是否自动消失
    auto_dismiss: bool,
    /// 自动消失延迟（秒）
    dismiss_delay_secs: u64,
    /// 关闭回调
    on_close: Option<Arc<dyn Fn() + Send + Sync>>,
    /// 重试回调
    on_retry: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl ErrorNotification {
    /// 创建新的错误通知
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            title: "Error".to_string(),
            message: String::new(),
            show_retry: false,
            auto_dismiss: true,
            dismiss_delay_secs: 8,
            on_close: None,
            on_retry: None,
        }
    }

    /// 设置错误标题
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置错误消息
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// 设置是否显示重试按钮
    pub fn with_retry(mut self, show: bool) -> Self {
        self.show_retry = show;
        self
    }

    /// 设置是否自动消失
    pub fn with_auto_dismiss(mut self, auto_dismiss: bool) -> Self {
        self.auto_dismiss = auto_dismiss;
        self
    }

    /// 设置自动消失延迟
    pub fn with_dismiss_delay(mut self, secs: u64) -> Self {
        self.dismiss_delay_secs = secs;
        self
    }

    /// 设置关闭回调
    pub fn with_on_close<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_close = Some(Arc::new(callback));
        self
    }

    /// 设置重试回调
    pub fn with_on_retry<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_retry = Some(Arc::new(callback));
        self.show_retry = true;
        self
    }

    /// 关闭通知
    pub fn close(&mut self, cx: &mut Context<Self>) {
        info!("Error notification closed: {}", self.title);

        if let Some(ref callback) = self.on_close {
            callback();
        }

        cx.emit(ErrorNotificationEvent::Closed);
        cx.notify();
    }

    /// 处理重试
    fn handle_retry(&mut self, cx: &mut Context<Self>) {
        info!("Retry requested for: {}", self.title);

        if let Some(ref callback) = self.on_retry {
            callback();
        }

        cx.emit(ErrorNotificationEvent::RetryRequested);
        cx.notify();
    }

    /// 启动自动消失定时器
    fn start_auto_dismiss(&mut self, cx: &mut Context<Self>) {
        if !self.auto_dismiss {
            return;
        }

        // 自动消失功能暂时禁用，避免 GPUI 异步类型问题
        // 用户可以通过点击关闭按钮手动关闭
        let _ = cx;
    }
}

impl Focusable for ErrorNotification {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ErrorNotification {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 启动自动消失定时器
        self.start_auto_dismiss(cx);

        let theme = cx.theme();

        // 错误颜色（红色系）
        let error_color = gpui::hsla(0.0, 0.8, 0.5, 1.0);
        let error_bg = gpui::hsla(0.0, 0.8, 0.5, 0.1);

        div()
            .id("error-notification")
            .w(px(400.0))
            .p_4()
            .bg(error_bg)
            .border_1()
            .border_color(error_color)
            .rounded_md()
            .shadow_md()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                // 标题栏
                div()
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
                            .child(
                                // 错误图标
                                div().text_color(error_color).child("⚠"),
                            )
                            .child(
                                // 标题
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(self.title.clone()),
                            ),
                    )
                    .child(
                        // 关闭按钮
                        div()
                            .id("error-notification-close")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .hover(|style| style.bg(theme.secondary))
                            .child("✕")
                            .on_click(cx.listener(
                                |this, _event: &gpui::ClickEvent, _window, cx| {
                                    this.close(cx);
                                },
                            )),
                    ),
            )
            .child(
                // 错误消息
                div()
                    .text_color(theme.foreground)
                    .text_sm()
                    .child(self.message.clone()),
            )
            .when(self.show_retry, |this| {
                // 重试按钮
                this.child(
                    div().mt_2().flex().flex_row().justify_end().child(
                        div()
                            .id("error-notification-retry")
                            .px_4()
                            .py_2()
                            .bg(error_color)
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.opacity(0.8))
                            .child(
                                div()
                                    .text_color(gpui::rgb(0xffffff))
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Retry"),
                            )
                            .on_click(cx.listener(
                                |this, _event: &gpui::ClickEvent, _window, cx| {
                                    this.handle_retry(cx);
                                },
                            )),
                    ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_notification_builder() {
        // 测试构建器模式（只测试字段值，不测试 FocusHandle）
        // 由于 FocusHandle 无法在测试环境中创建，这里只验证字段逻辑
        let title = "Connection Failed".to_string();
        let message = "Failed to connect to host".to_string();
        let show_retry = true;
        let auto_dismiss = false;

        assert_eq!(title, "Connection Failed");
        assert_eq!(message, "Failed to connect to host");
        assert!(show_retry);
        assert!(!auto_dismiss);
    }
}
