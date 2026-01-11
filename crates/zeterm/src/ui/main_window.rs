//! 主窗口视图组件
//!
//! 应用程序的主窗口，包含终端视图和其他 UI 元素。
//! 集成 gpui-component 的 Root 和 Theme 系统。

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable, Size,
    button::{Button, ButtonVariants},
    theme,
};

/// 主窗口视图
///
/// 应用程序的顶层视图组件，负责：
/// - 管理整体布局
/// - 协调子视图
/// - 处理全局快捷键
pub struct MainWindow {
    ///焦点句柄，用于键盘事件处理
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
        // 初始化 gpui-component 主题系统
        theme::init(cx);

        cx.new(|cx| Self::new(window, cx))
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 主窗口容器，使用 gpui-component 主题颜色
        div()
            .id("main-window")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_header(window, cx))
            .child(self.render_content(window, cx))
            .child(self.render_status_bar(window, cx))
    }
}

impl MainWindow {
    /// 渲染顶部标题栏
    fn render_header(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("header")
            .w_full()
            .h(px(48.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.title_bar)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Zeterm - SSH Terminal Client"),
            )
    }

    /// 渲染主内容区域
    fn render_content(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("content")
            .flex_1()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    //欢迎标题
                    .child(
                        div()
                            .text_size(px(28.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("🚀 Welcome to Zeterm"),
                    )
                    // 状态信息
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(theme.muted_foreground)
                            .child("Phase1: gpui-component 集成完成"),
                    )
                    // 测试按钮 - 验证 gpui-component Button 组件
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("btn-primary")
                                    .label("Primary Button")
                                    .primary()
                                    .with_size(Size::Medium),
                            )
                            .child(
                                Button::new("btn-secondary")
                                    .label("Secondary")
                                    .with_size(Size::Medium),
                            ),
                    )
                    // 快捷键提示
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.muted_foreground)
                            .child("按 Ctrl+Q 退出"),
                    ),
            )
    }

    /// 渲染底部状态栏
    fn render_status_bar(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("status-bar")
            .w_full()
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme.title_bar)
            .border_t_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted_foreground)
                    .child("Ready"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted_foreground)
                    .child("Theme: Dark"),
            )
    }
}
