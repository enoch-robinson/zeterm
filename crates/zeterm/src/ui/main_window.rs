//! 主窗口模块
//!
//! 管理应用程序的主窗口，包括主机列表、终端视图、分屏功能、状态栏和主题。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, Styled, Window, div, prelude::*, px,
};
use gpui_component::ActiveTheme;
use tracing::info;

use crate::app::session::SessionCoordinator;
use crate::ui::app_theme::{AppThemeManager, BuiltinTheme, ThemeMode};
use crate::ui::split_pane::{Pane, PaneId, SplitManager, SplitView};
use crate::ui::status_bar::{ConnectionStatus, StatusBar, StatusInfo};
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
    /// 状态栏
    status_bar: Entity<StatusBar>,
    /// 主题管理器
    theme_manager: AppThemeManager,
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
    /// 构建主窗口（工厂方法，供 open_window 使用）
    pub fn build(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 先创建 SplitManager
        let split_manager = cx.new(|_cx| SplitManager::new());
        Self::new(split_manager, cx)
    }

    /// 创建新的主窗口
    pub fn new(split_manager: Entity<SplitManager>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let split_view = cx.new(|cx| SplitView::new(split_manager.clone(), cx));
        let status_bar = cx.new(|cx| StatusBar::new(cx));
        let theme_manager = AppThemeManager::new();

        Self {
            focus_handle,
            host_list_view: None,
            terminal_views: HashMap::new(),
            coordinators: HashMap::new(),
            split_manager,
            split_view,
            status_bar,
            theme_manager,
            show_sidebar: true,
        }
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        !self.coordinators.is_empty()
    }

    /// 获取状态栏实体
    pub fn status_bar(&self) -> &Entity<StatusBar> {
        &self.status_bar
    }

    /// 更新状态栏连接状态
    pub fn update_connection_status(&self, status: ConnectionStatus, cx: &mut Context<Self>) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_connection_status(status, cx);
        });
    }

    /// 更新状态栏用户和主机信息
    pub fn update_user_host(
        &self,
        username: Option<String>,
        hostname: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_user_host(username, hostname, cx);
        });
    }

    /// 更新状态栏终端尺寸
    pub fn update_terminal_size(&self, cols: u16, rows: u16, cx: &mut Context<Self>) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_size(cols, rows, cx);
        });
    }

    /// 更新状态栏 RTT
    pub fn update_rtt(&self, rtt_ms: Option<u32>, cx: &mut Context<Self>) {
        self.status_bar.update(cx, |bar, cx| {
            bar.set_rtt(rtt_ms, cx);
        });
    }

    /// 更新完整状态信息
    pub fn update_status_info(&self, info: StatusInfo, cx: &mut Context<Self>) {
        self.status_bar.update(cx, |bar, cx| {
            bar.update_status(info, cx);
        });
    }

    // ==================== 主题管理 ====================

    /// 获取主题管理器
    pub fn theme_manager(&self) -> &AppThemeManager {
        &self.theme_manager
    }

    /// 获取主题管理器（可变）
    pub fn theme_manager_mut(&mut self) -> &mut AppThemeManager {
        &mut self.theme_manager
    }

    /// 获取当前主题
    pub fn current_theme(&self) -> BuiltinTheme {
        self.theme_manager.current_theme()
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: BuiltinTheme, cx: &mut Context<Self>) {
        self.theme_manager.set_theme(theme);
        // 同步终端视图的主题
        self.sync_terminal_themes(cx);
        info!("Theme changed to: {:?}", theme);
        cx.notify();
    }

    /// 设置主题模式
    pub fn set_theme_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        self.theme_manager.set_mode(mode);
        self.sync_terminal_themes(cx);
        info!("Theme mode changed to: {:?}", mode);
        cx.notify();
    }

    /// 切换深色/浅色模式
    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_manager.toggle_dark_light();
        self.sync_terminal_themes(cx);
        info!(
            "Theme toggled to: {:?} (dark: {})",
            self.theme_manager.current_theme(),
            self.theme_manager.is_dark_mode()
        );
        cx.notify();
    }

    /// 切换到下一个主题
    pub fn next_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_manager.next_theme();
        self.sync_terminal_themes(cx);
        info!(
            "Switched to next theme: {:?}",
            self.theme_manager.current_theme()
        );
        cx.notify();
    }

    /// 切换到上一个主题
    pub fn prev_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_manager.prev_theme();
        self.sync_terminal_themes(cx);
        info!(
            "Switched to prev theme: {:?}",
            self.theme_manager.current_theme()
        );
        cx.notify();
    }

    /// 是否为深色模式
    pub fn is_dark_mode(&self) -> bool {
        self.theme_manager.is_dark_mode()
    }

    /// 获取所有可用主题
    pub fn available_themes(&self) -> &'static [BuiltinTheme] {
        self.theme_manager.available_themes()
    }

    /// 同步终端视图的主题
    fn sync_terminal_themes(&mut self, cx: &mut Context<Self>) {
        let terminal_theme = self.theme_manager.terminal_theme().clone();
        for terminal_view in self.terminal_views.values() {
            terminal_view.update(cx, |view, _cx| {
                view.set_theme(terminal_theme.clone());
            });
        }
    }

    // ==================== 侧边栏管理 ====================

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

    /// 添加终端面板
    ///
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

        // 更新状态栏为已连接状态
        self.update_connection_status(ConnectionStatus::Connected, cx);

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

        // 更新状态栏用户主机信息
        self.update_user_host(
            Some(host_config.username.clone()),
            Some(host_config.host.clone()),
            cx,
        );

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

        // 如果没有更多连接，更新状态栏
        if self.coordinators.is_empty() {
            self.update_connection_status(ConnectionStatus::Disconnected, cx);
            self.update_user_host(None, None, cx);
        }

        info!("Closed terminal pane: {}", pane_id);
        cx.notify();
    }

    /// 获取所有终端视图
    pub fn terminal_views(&self) -> &HashMap<PaneId, Entity<TerminalView>> {
        &self.terminal_views
    }

    /// 渲染欢迎界面
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
            // 渲染分屏视图
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

        // 构建主布局（垂直布局：内容区 + 状态栏）
        div()
            .id("main-window")
            .size_full()
            .flex()
            .flex_col()
            .child(
                // 内容区域（水平布局：左侧边栏 + 右侧主区域）
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
                    .child(main_area.child(content_area)),
            )
            .child(
                // 底部状态栏
                self.status_bar.clone(),
            )
    }
}
