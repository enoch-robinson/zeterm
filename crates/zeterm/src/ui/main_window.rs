//! 主窗口模块
//!
//! 管理应用程序的主窗口，包括主机列表、终端视图和分屏功能。

use gpui::{
    App, Context, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
};
use std::sync::Arc;

use crate::ui::split_pane::SplitManager;
use crate::ui::tab_manager::TabManager;

/// 主窗口
#[derive(Debug)]
pub struct MainWindow {
    /// 主机列表视图
    host_list_view: Option<SharedString>,
    /// 标签管理器
    tab_manager: TabManager,
    /// 终端视图映射
    terminal_views: std::collections::HashMap<usize, SharedString>,
    /// 分屏管理器
    split_manager: Entity<SplitManager>,
    /// 分屏视图
    split_view: Entity<gpui::AnyView>,
    /// 是否显示侧边栏
    show_sidebar: bool,
}

impl MainWindow {
    /// 创建新的主窗口
    pub fn new(split_manager: Entity<SplitManager>, cx: &mut Context<Self>) -> Self {
        let split_view =
            cx.new_view(|_cx| crate::ui::split_pane::SplitView::new(split_manager.clone(), _cx));
        Self {
            host_list_view: None,
            tab_manager: TabManager::new(),
            terminal_views: std::collections::HashMap::new(),
            split_manager,
            split_view,
            show_sidebar: true,
        }
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        false
    }

    /// 切换侧边栏
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.show_sidebar = !self.show_sidebar;
        cx.notify();
    }

    /// 水平分屏
    pub fn split_horizontal(&mut self, cx: &mut Context<Self>) {
        if let Some(pane_id) = self.split_manager.read(cx).focused_pane() {
            self.split_manager.update(cx, |manager, cx| {
                manager.split_horizontal(pane_id, cx);
            });
        }
    }

    /// 垂直分屏
    pub fn split_vertical(&mut self, cx: &mut Context<Self>) {
        if let Some(pane_id) = self.split_manager.read(cx).focused_pane() {
            self.split_manager.update(cx, |manager, cx| {
                manager.split_vertical(pane_id, cx);
            });
        }
    }

    /// 关闭面板
    pub fn close_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(pane_id) = self.split_manager.read(cx).focused_pane() {
            self.split_manager.update(cx, |manager, cx| {
                manager.close_pane(pane_id, cx);
            });
        }
    }

    /// 渲染欢迎界面
    fn render_welcome(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .w_full()
            .h_full()
            .child(
                div()
                    .text_4xl()
                    .font_bold()
                    .text_center()
                    .mb_4()
                    .child("Zeterm"),
            )
            .child(
                div()
                    .text_xl()
                    .text_center()
                    .text_gray_500()
                    .child("选择左侧主机列表中的主机开始连接"),
            )
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> gpui::FocusHandle {
        gpui::FocusHandle::disabled(_cx)
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_content(cx)
    }
}

impl MainWindow {
    /// 渲染主内容区域
    fn render_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let secondary = theme.secondary;
        let border = theme.border;
        let is_connected = self.is_connected();

        // 右侧主区域容器
        let main_area = div().id("main-area").flex_1().flex().flex_col();

        // 终端或欢迎界面
        let content_area = if is_connected {
            div()
                .id("terminal-panel")
                .flex_1()
                .w_full()
                .flex()
                .flex_col()
                .child(
                    div().id("terminal-container").flex_1().w_full().child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child("Terminal View"),
                    ),
                )
                .into_any_element()
        } else {
            div()
                .id("welcome-panel")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(self.render_welcome())
                .into_any_element()
        };

        // 构建主内容区域（水平布局：左侧边栏 + 右侧主区域）
        div()
            .id("content")
            .flex_1()
            .w_full()
            .flex()
            .flex_row()
            .child({
                // 左侧面板：主机列表
                if self.show_sidebar {
                    if let Some(ref host_list_view) = self.host_list_view {
                        div()
                            .id("sidebar")
                            .w(px(280.0))
                            .h_full()
                            .flex_shrink_0()
                            .border_r_1()
                            .border_color(border)
                            .bg(secondary)
                            .child(host_list_view.clone())
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }
                } else {
                    div().into_any_element()
                }
            })
            .child(main_area.child(content_area))
    }
}

