//! 主窗口模块
//!
//! 管理应用程序的主窗口，包括主机列表、终端视图和分屏功能。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div, prelude::*, px,
};
use gpui_component::ActiveTheme;
use tracing::info;

use crate::app::session::SessionCoordinator;
use crate::ui::split_pane::{Pane, PaneId, SplitManager, SplitView};
use crate::ui::terminal_view::TerminalView;

/// 主窗口
pub struct MainWindow {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 主机列表视图
    host_list_view: Option<SharedString>,
    /// 终端视图映射 (PaneId -> TerminalView Entity)
    terminal_views: HashMap<PaneId, Entity<TerminalView>>,
    /// 会话协调器映射 (PaneId -> SessionCoordinator)
    coordinators: HashMap<PaneId, Arc<SessionCoordinator>>,
    /// 分屏管理器
    split_manager: Entity<SplitManager>,
    /// 分屏视图
    split_view: Entity<SplitView>,
    /// 是否显示侧边栏
    show_sidebar: bool,
}

impl std::fmt::Debug for MainWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainWindow")
            .field("host_list_view", &self.host_list_view)
            .field("terminal_views", &self.terminal_views)
            .field("show_sidebar", &self.show_sidebar)
            .finish_non_exhaustive()
    }
}

impl MainWindow {
    /// 构建主窗口（工厂方法，供open_window 使用）
    pub fn build(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 先创建 SplitManager
        let split_manager = cx.new(|_cx| SplitManager::new());
        Self::new(split_manager, cx)
    }

    /// 创建新的主窗口
    pub fn new(split_manager: Entity<SplitManager>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let split_view = cx.new(|cx| SplitView::new(split_manager.clone(), cx));

        Self {
            focus_handle,
            host_list_view: None,
            terminal_views: HashMap::new(),
            coordinators: HashMap::new(),
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

    /// 添加终端面板///
    /// 创建一个新的终端面板并添加到分屏管理器。
    /// 如果当前没有面板，则设置为根面板；
    /// 如果已有面板，则在当前焦点面板旁边水平分屏。
    pub fn add_terminal_pane(
        &mut self,
        title: impl Into<String>,
        coordinator: Arc<SessionCoordinator>,
        cx: &mut Context<Self>,
    ) -> PaneId {
        let title = title.into();
        let new_pane = Pane::new_terminal(&title);
        let pane_id = new_pane.id;

        // 创建 TerminalView 实体
        let terminal_view = cx.new(|cx| TerminalView::new(coordinator.clone(), cx));

        // 存储映射关系
        self.terminal_views.insert(pane_id, terminal_view.clone());
        self.coordinators.insert(pane_id, coordinator);

        // 同步到 SplitView
        self.split_view.update(cx, |view, _cx| {
            view.register_terminal_view(pane_id, terminal_view);
        });

        let has_panes = self.split_manager.read(cx).has_panes();

        if !has_panes {
            // 没有面板，设置为根面板
            self.split_manager.update(cx, |manager, cx| {
                manager.set_root(new_pane, cx);
                manager.focus_pane(pane_id, cx);
            });
        } else {
            // 已有面板，在当前焦点面板旁边水平分屏
            if let Some(focused_id) = self.split_manager.read(cx).focused_pane() {
                self.split_manager.update(cx, |manager, cx| {
                    manager.split_horizontal(focused_id, cx);
                });
            }
        }

        info!("Added terminal pane: {} (id: {})", title, pane_id);
        cx.notify();
        pane_id
    }

    /// 添加 SSH 终端面板
    ///
    /// 使用主机配置创建一个新的终端面板
    pub fn add_ssh_terminal_pane(
        &mut self,
        host_config: &zeterm_core::entities::HostConfig,
        coordinator: Arc<SessionCoordinator>,
        cx: &mut Context<Self>,
    ) -> PaneId {
        let title = format!("{}@{}", host_config.username, host_config.host);
        self.add_terminal_pane(title, coordinator, cx)
    }

    /// 获取指定面板的 TerminalView
    pub fn get_terminal_view(&self, pane_id: PaneId) -> Option<&Entity<TerminalView>> {
        self.terminal_views.get(&pane_id)
    }

    /// 获取指定面板的 SessionCoordinator
    pub fn get_coordinator(&self, pane_id: PaneId) -> Option<&Arc<SessionCoordinator>> {
        self.coordinators.get(&pane_id)
    }

    /// 关闭指定面板的终端
    pub fn close_terminal_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        // 移除终端视图和协调器
        self.terminal_views.remove(&pane_id);
        self.coordinators.remove(&pane_id);

        // 从 SplitView 中移除
        self.split_view.update(cx, |view, _cx| {
            view.unregister_terminal_view(pane_id);
        });

        // 从分屏管理器中移除
        self.split_manager.update(cx, |manager, cx| {
            manager.close_pane(pane_id, cx);
        });

        info!("Closed terminal pane: {}", pane_id);
        cx.notify();
    }

    /// 获取所有终端视图
    pub fn terminal_views(&self) -> &HashMap<PaneId, Entity<TerminalView>> {
        &self.terminal_views
    }

    ///渲染欢迎界面
    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .w_full()
            .h_full()
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_center()
                    .mb_4()
                    .text_color(theme.foreground)
                    .child("Zeterm"),
            )
            .child(
                div()
                    .text_xl()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .child("选择左侧主机列表中的主机开始连接"),
            )
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
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

        // 右侧主区域容器
        let main_area = div().id("main-area").flex_1().flex().flex_col();

        // 使用 SplitView 渲染终端区域
        // 如果有面板则渲染 SplitView，否则渲染欢迎界面
        let has_panes = self.split_manager.read(cx).has_panes();

        let content_area = if has_panes {
            //渲染分屏视图
            div()
                .id("split-panel")
                .flex_1()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .child(self.split_view.clone())
                .into_any_element()
        } else {
            // 渲染欢迎界面
            div()
                .id("welcome-panel")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(self.render_welcome(cx))
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
