//! 路径导航栏组件
//!
//! 提供 SFTP 路径导航功能，包括：
//! - 面包屑导航（点击路径段跳转）
//! - 直接输入路径
//! - 前进/后退按钮
//! - 刷新按钮

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

// ============================================================================
// 事件定义
// ============================================================================

/// 路径栏事件
#[derive(Debug, Clone)]
pub enum PathBarEvent {
    /// 路径变更请求
    NavigateTo(String),
    /// 返回上级目录
    GoUp,
    /// 后退
    GoBack,
    /// 前进
    GoForward,
    /// 刷新当前目录
    Refresh,
    /// 主目录
    GoHome,
}

// ============================================================================
// 路径栏组件
// ============================================================================

/// 路径栏状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathBarMode {
    /// 显示模式（面包屑）
    Display,
    /// 编辑模式（文本输入）
    Edit,
}

/// 路径栏组件
pub struct PathBar {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 当前路径
    current_path: String,
    /// 路径段（解析后）
    path_segments: Vec<String>,
    /// 当前模式
    mode: PathBarMode,
    /// 编辑中的路径文本
    edit_text: String,
    /// 历史记录（用于前进/后退）
    history: Vec<String>,
    /// 历史记录当前位置
    history_index: usize,
    /// 主目录路径
    home_path: Option<String>,
    /// 是否显示隐藏文件
    show_hidden: bool,
    /// hover 的路径段索引
    hovered_segment: Option<usize>,
}

impl PathBar {
    /// 创建新的路径栏
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            current_path: "/".to_string(),
            path_segments: vec!["/".to_string()],
            mode: PathBarMode::Display,
            edit_text: String::new(),
            history: vec!["/".to_string()],
            history_index: 0,
            home_path: None,
            show_hidden: false,
            hovered_segment: None,
        }
    }

    /// 设置当前路径
    pub fn set_path(&mut self, path: impl Into<String>, cx: &mut Context<Self>) {
        let path = path.into();
        self.current_path = path.clone();
        self.path_segments = Self::parse_path(&self.current_path);
        self.edit_text = self.current_path.clone();

        // 添加到历史记录
        if self.history_index < self.history.len() - 1 {
            // 如果在历史中间，截断后面的记录
            self.history.truncate(self.history_index + 1);
        }
        if self.history.last() != Some(&path) {
            self.history.push(path);
            self.history_index = self.history.len() - 1;
        }

        cx.notify();
    }

    /// 设置主目录
    pub fn set_home_path(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.home_path = path;
        cx.notify();
    }

    /// 设置是否显示隐藏文件
    pub fn set_show_hidden(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_hidden = show;
        cx.notify();
    }

    /// 获取当前路径
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// 是否可以后退
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    /// 是否可以前进
    pub fn can_go_forward(&self) -> bool {
        self.history_index < self.history.len().saturating_sub(1)
    }

    /// 是否可以上级
    pub fn can_go_up(&self) -> bool {
        self.current_path != "/"
    }

    /// 解析路径为段
    fn parse_path(path: &str) -> Vec<String> {
        let mut segments = Vec::new();

        // 始终添加根目录
        segments.push("/".to_string());

        // 添加其他段
        for part in path.split('/').filter(|s| !s.is_empty()) {
            segments.push(part.to_string());
        }

        segments
    }

    /// 从段列表构建路径
    fn build_path_to_index(&self, index: usize) -> String {
        if index == 0 {
            return "/".to_string();
        }

        let parts: Vec<&str> = self.path_segments[1..=index]
            .iter()
            .map(|s| s.as_str())
            .collect();

        format!("/{}", parts.join("/"))
    }

    /// 进入编辑模式
    fn enter_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.mode = PathBarMode::Edit;
        self.edit_text = self.current_path.clone();
        cx.notify();
    }

    /// 退出编辑模式
    fn exit_edit_mode(&mut self, cx: &mut Context<Self>) {
        self.mode = PathBarMode::Display;
        self.edit_text = self.current_path.clone();
        cx.notify();
    }

    /// 提交编辑的路径
    fn submit_edit(&mut self, cx: &mut Context<Self>) {
        let path = self.edit_text.trim().to_string();
        if !path.is_empty() {
            self.mode = PathBarMode::Display;
            cx.emit(PathBarEvent::NavigateTo(path));
        }
    }

    /// 处理键盘输入
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.mode != PathBarMode::Edit {
            return;
        }

        match event.keystroke.key.as_str() {
            "escape" => {
                self.exit_edit_mode(cx);
            },
            "enter" => {
                self.submit_edit(cx);
            },
            "backspace" => {
                self.edit_text.pop();
                cx.notify();
            },
            key if key.len() == 1 => {
                let ch = key.chars().next().unwrap();
                if !event.keystroke.modifiers.control && !event.keystroke.modifiers.alt {
                    self.edit_text.push(ch);
                    cx.notify();
                }
            },
            "space" => {
                self.edit_text.push(' ');
                cx.notify();
            },
            _ => {},
        }
    }

    /// 导航到路径段
    fn navigate_to_segment(&mut self, index: usize, cx: &mut Context<Self>) {
        let path = self.build_path_to_index(index);
        cx.emit(PathBarEvent::NavigateTo(path));
    }

    /// 后退
    fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.can_go_back() {
            self.history_index -= 1;
            let path = self.history[self.history_index].clone();
            self.current_path = path.clone();
            self.path_segments = Self::parse_path(&self.current_path);
            cx.emit(PathBarEvent::NavigateTo(path));
        }
    }

    /// 前进
    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.can_go_forward() {
            self.history_index += 1;
            let path = self.history[self.history_index].clone();
            self.current_path = path.clone();
            self.path_segments = Self::parse_path(&self.current_path);
            cx.emit(PathBarEvent::NavigateTo(path));
        }
    }

    /// 渲染导航按钮
    fn render_nav_buttons(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let can_back = self.can_go_back();
        let can_forward = self.can_go_forward();
        let can_up = self.can_go_up();

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            // 后退按钮
            .child(
                div()
                    .id("btn-back")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if can_back {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .when(can_back, |el| el.hover(|el| el.bg(theme.secondary)))
                    .child("←"),
            )
            // 前进按钮
            .child(
                div()
                    .id("btn-forward")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if can_forward {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .when(can_forward, |el| el.hover(|el| el.bg(theme.secondary)))
                    .child("→"),
            )
            // 上级目录按钮
            .child(
                div()
                    .id("btn-up")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(if can_up {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .when(can_up, |el| el.hover(|el| el.bg(theme.secondary)))
                    .child("↑"),
            )
            // 刷新按钮
            .child(
                div()
                    .id("btn-refresh")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(theme.foreground)
                    .hover(|el| el.bg(theme.secondary))
                    .child("⟳"),
            )
            // 主目录按钮
            .when(self.home_path.is_some(), |el| {
                el.child(
                    div()
                        .id("btn-home")
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_color(theme.foreground)
                        .hover(|el| el.bg(theme.secondary))
                        .child("🏠"),
                )
            })
    }

    /// 渲染面包屑导航
    fn render_breadcrumbs(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let segments = self.path_segments.clone();

        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_1()
            .overflow_hidden()
            .children(segments.into_iter().enumerate().map(|(i, segment)| {
                let is_last = i == self.path_segments.len() - 1;
                let display_name = if i == 0 { "🖥" } else { &segment };

                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .id(SharedString::from(format!("segment-{}", i)))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(if is_last {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .font_weight(if is_last {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .hover(|el| el.bg(theme.secondary))
                            .child(SharedString::from(display_name.to_string())),
                    )
                    .when(!is_last, |el| {
                        el.child(div().text_color(theme.muted_foreground).mx_1().child("/"))
                    })
            }))
    }

    /// 渲染编辑输入框
    fn render_edit_input(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex_1()
            .px_2()
            .py_1()
            .mx_1()
            .rounded_md()
            .bg(theme.background)
            .border_1()
            .border_color(theme.primary)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        div()
                            .text_color(theme.foreground)
                            .child(SharedString::from(self.edit_text.clone())),
                    )
                    .child(
                        // 光标
                        div().w(px(1.0)).h(px(14.0)).bg(theme.foreground).ml_px(),
                    ),
            )
    }

    /// 渲染隐藏文件切换
    fn render_hidden_toggle(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let show_hidden = self.show_hidden;

        div()
            .id("toggle-hidden")
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_color(if show_hidden {
                theme.primary
            } else {
                theme.muted_foreground
            })
            .hover(|el| el.bg(theme.secondary))
            .child(if show_hidden { "👁" } else { "👁‍🗨" })
    }
}

impl EventEmitter<PathBarEvent> for PathBar {}

impl Focusable for PathBar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for PathBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mode = self.mode;

        div()
            .id("sftp-path-bar")
            .w_full()
            .h(px(36.0))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .gap_2()
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            .text_sm()
            // 捕获键盘事件
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                this.handle_key_down(event, cx);
            }))
            // 导航按钮
            .child(self.render_nav_buttons(cx))
            // 路径显示/编辑区域
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            if this.mode == PathBarMode::Display {
                                this.enter_edit_mode(cx);
                            }
                        }),
                    )
                    .child(match mode {
                        PathBarMode::Display => self.render_breadcrumbs(cx).into_any_element(),
                        PathBarMode::Edit => self.render_edit_input(cx).into_any_element(),
                    }),
            )
            // 隐藏文件切换
            .child(self.render_hidden_toggle(cx))
    }
}

/// 创建路径栏的便捷方法
pub fn create_path_bar(cx: &mut Context<PathBar>) -> PathBar {
    PathBar::new(cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_path_root() {
        let segments = PathBar::parse_path("/");
        assert_eq!(segments, vec!["/"]);
    }

    #[test]
    fn test_parse_path_simple() {
        let segments = PathBar::parse_path("/home");
        assert_eq!(segments, vec!["/", "home"]);
    }

    #[test]
    fn test_parse_path_nested() {
        let segments = PathBar::parse_path("/home/user/documents");
        assert_eq!(segments, vec!["/", "home", "user", "documents"]);
    }

    #[test]
    fn test_parse_path_trailing_slash() {
        let segments = PathBar::parse_path("/home/user/");
        assert_eq!(segments, vec!["/", "home", "user"]);
    }

    #[test]
    fn test_path_bar_event_debug() {
        let event = PathBarEvent::NavigateTo("/test".to_string());
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("NavigateTo"));
    }
}
