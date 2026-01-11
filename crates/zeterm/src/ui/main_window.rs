//! 主窗口视图组件
//!
//! 应用程序的主窗口，包含终端视图和其他 UI 元素。

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, px, rgb,
};

/// 主窗口视图
///
/// 应用程序的顶层视图组件，负责：
/// - 管理整体布局
/// - 协调子视图
/// - 处理全局快捷键
pub struct MainWindow {
    /// 焦点句柄，用于键盘事件处理
    focus_handle: FocusHandle,
}

impl MainWindow {
    /// 创建新的主窗口视图
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }

    /// 在窗口上下文中构建主窗口视图
    pub fn build(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("main-window")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e2e)) // 深色背景
            .text_color(rgb(0xcdd6f4)) // 浅色文字
            .child(self.render_header())
            .child(self.render_content())
            .child(self.render_status_bar())
    }
}

impl MainWindow {
    /// 渲染顶部标题栏
    fn render_header(&self) -> impl IntoElement {
        div()
            .id("header")
            .w_full()
            .h(px(40.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0x181825))
            .border_b_1()
            .border_color(rgb(0x313244))
            .child(
                div()
                    .text_size(px(14.0))
                    .child("Zeterm - SSH Terminal Client"),
            )
    }

    /// 渲染主内容区域
    fn render_content(&self) -> impl IntoElement {
        div()
            .id("content")
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .child(div().text_size(px(24.0)).child("🚀 Welcome to Zeterm"))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(0x6c7086))
                            .child("Phase 1: GPUI 基础窗口已创建"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0x585b70))
                            .child("按 Ctrl+Q 退出"),
                    ),
            )
    }

    /// 渲染底部状态栏
    fn render_status_bar(&self) -> impl IntoElement {
        div()
            .id("status-bar")
            .w_full()
            .h(px(24.0))
            .flex()
            .items_center()
            .px_3()
            .bg(rgb(0x181825))
            .border_t_1()
            .border_color(rgb(0x313244))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x6c7086))
                    .child("Ready"),
            )
    }
}
