//! 主窗口模块
//!
//! 管理应用程序的主窗口，包括 Tab 管理、分屏布局、状态栏和主题。
//! 采用 "每个 Tab 独立分屏" 模式，参考 iTerm2 和 Windows Terminal 设计。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement,
    Render, Styled, Window, div, prelude::*, px,
};
use gpui_component::ActiveTheme;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::app::runtime;
use crate::app::session::SessionCoordinator;
use crate::app::terminal::TerminalConfig;
use crate::ui::app_theme::{AppThemeManager, BuiltinTheme, ThemeMode};
use crate::ui::connection_manager::{ConnectionEvent, ConnectionManager};
use crate::ui::dialogs::{ErrorNotification, HostConnectionDialog};
use crate::ui::host_list::{HostListEvent, HostListView};
use crate::ui::split_pane::{Pane, PaneId, SplitDirection, SplitManager, SplitView};
use crate::ui::status_bar::{ConnectionStatus, StatusBar, StatusInfo};
use crate::ui::tab_manager::{TabId, TabInfo, TabManager, TabManagerEvent};
use crate::ui::tab_view::{TabView, TabViewEvent};
use crate::ui::terminal_pane_manager::{TerminalPaneData, TerminalPaneManager};
use crate::ui::terminal_view::TerminalView;
use zeterm_core::entities::HostConfig;
use zeterm_storage::{HostRepository, SqliteHostRepository};

/// 主窗口
pub struct MainWindow {
    /// 焦点句柄
    focus_handle: FocusHandle,

    /// 主机列表视图
    host_list_view: Entity<HostListView>,

    /// Tab 管理器
    tab_manager: Entity<TabManager>,

    /// Tab 视图
    tab_view: Entity<TabView>,

    /// 每个 Tab 的 SplitView（TabId -> SplitView）
    split_views: HashMap<TabId, Entity<SplitView>>,

    /// 终端面板数据（PaneId -> TerminalPaneData）
    terminal_panes: HashMap<PaneId, TerminalPaneData>,

    /// 终端面板管理器
    terminal_pane_manager: TerminalPaneManager,

    /// 连接管理器（静态方法，但保留为字段以保持一致性）
    _connection_manager: ConnectionManager,

    /// 状态栏
    status_bar: Entity<StatusBar>,

    /// 主题管理器
    theme_manager: AppThemeManager,

    /// 连接对话框（新建或编辑主机）- 使用共享引用以便在回调中关闭
    connection_dialog: Arc<Mutex<Option<Entity<HostConnectionDialog>>>>,

    /// 错误通知
    error_notification: Option<Entity<ErrorNotification>>,

    /// 待处理的连接错误队列
    pending_errors: Arc<Mutex<Vec<String>>>,

    /// 是否显示侧边栏
    show_sidebar: bool,

    /// 是否显示 Tab 栏
    show_tab_bar: bool,
}

impl std::fmt::Debug for MainWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MainWindow")
            .field("host_list_view", &self.host_list_view)
            .field("terminal_panes_count", &self.terminal_panes.len())
            .field("split_views_count", &self.split_views.len())
            .field("show_sidebar", &self.show_sidebar)
            .field("show_tab_bar", &self.show_tab_bar)
            .finish_non_exhaustive()
    }
}

/// 状态栏同步触发标志
/// 用于在事件处理器中触发状态栏同步，避免在 render 中修改状态
#[derive(Clone, Debug)]
enum StatusBarSyncEvent {
    SyncRequested,
}

impl MainWindow {
    /// 构建主窗口（工厂方法，供 open_window 使用）
    pub fn build(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(cx)
    }

    /// 创建新的主窗口
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // 获取全局数据库实例
        let database = crate::app::global_database()
            .expect("Database should be initialized before creating MainWindow");

        // 创建主机列表视图
        let host_list_view = cx.new(|cx| HostListView::new(database, cx));

        // 创建 Tab 管理器
        let tab_manager = cx.new(|_cx| TabManager::new());

        // 创建 Tab 视图
        let tab_view = cx.new(|cx| TabView::new(tab_manager.clone(), cx));

        // 创建状态栏
        let status_bar = cx.new(|cx| StatusBar::new(cx));

        // 主题管理器
        let theme_manager = AppThemeManager::new();

        // 订阅 Tab 管理器事件
        cx.subscribe(&tab_manager, Self::on_tab_manager_event)
            .detach();

        // 订阅主机列表视图事件
        cx.subscribe(&host_list_view, Self::on_host_list_event)
            .detach();

        // 订阅 TabView 事件
        cx.subscribe(&tab_view, Self::on_tab_view_event).detach();

        Self {
            focus_handle,
            host_list_view,
            tab_manager,
            tab_view,
            split_views: HashMap::new(),
            terminal_panes: HashMap::new(),
            terminal_pane_manager: TerminalPaneManager::new(),
            _connection_manager: ConnectionManager,
            status_bar,
            theme_manager,
            connection_dialog: Arc::new(Mutex::new(None)),
            error_notification: None,
            pending_errors: Arc::new(Mutex::new(Vec::new())),
            show_sidebar: true,
            show_tab_bar: true,
        }
    }

    /// 处理主机列表视图事件
    fn on_host_list_event(
        &mut self,
        _view: Entity<HostListView>,
        event: &HostListEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            HostListEvent::ConnectRequested(host) => {
                info!("用户请求连接主机: {}", host.name);
                self.connect_to_host(host.clone(), cx);
            },
            HostListEvent::NewHostRequested => {
                info!("用户请求新建主机");
                self.show_new_host_dialog(cx);
            },
            HostListEvent::EditHostRequested(host) => {
                info!("用户请求编辑主机: {}", host.name);
                self.show_edit_host_dialog(host.clone(), cx);
            },
            HostListEvent::HostDeleted(host_id) => {
                info!("主机已删除: {}", host_id);
            },
        }
    }

    // ==================== SSH 连接管理 ====================

    /// 连接到主机
    ///
    /// 执行完整的 SSH 连接流程：
    /// 1. 创建 SSH Tab
    /// 2. 添加终端面板
    /// 3. 委托给 ConnectionManager 处理连接
    fn connect_to_host(&mut self, host: zeterm_core::entities::HostConfig, cx: &mut Context<Self>) {
        // 1. 创建 SSH Tab
        let tab_id = match self.create_ssh_tab(&host, cx) {
            Some(id) => id,
            None => {
                error!("创建 Tab 失败");
                return;
            },
        };

        // 2. 创建 SessionCoordinator
        let config = TerminalConfig::default();
        let coordinator = Arc::new(SessionCoordinator::new(config));

        // 3. 添加 SSH 终端面板
        let _pane_id = match self.add_ssh_terminal_pane(tab_id, &host, coordinator.clone(), cx) {
            Some(id) => id,
            None => {
                error!("添加终端面板失败");
                return;
            },
        };

        // 4. 创建事件通道用于接收连接状态变化
        let (event_tx, mut event_rx) = mpsc::channel::<ConnectionEvent>(100);
        let pending_errors = self.pending_errors.clone();

        // 启动事件处理任务 - 使用 runtime::spawn 避免 GPUI 生命周期问题
        runtime::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    ConnectionEvent::Connected { .. } => {
                        // 状态栏更新将在 render 中通过 sync_status_bar_from_coordinator 处理
                    },
                    ConnectionEvent::Failed { host, error } => {
                        // 将错误加入队列，在 render 中显示
                        pending_errors
                            .lock()
                            .unwrap()
                            .push(format!("{}: {}", host, error));
                    },
                    ConnectionEvent::Disconnected { .. } => {
                        // 状态栏更新将在 render 中处理
                    },
                    ConnectionEvent::StateChanged { .. } => {
                        // 状态栏更新将在 render 中处理
                    },
                }
            }
        });

        // 5. 委托给连接管理器处理连接逻辑
        ConnectionManager::connect_to_host(
            host,
            tab_id,
            coordinator,
            cx,
            &self.tab_manager,
            &self.status_bar,
            event_tx,
            None, // 错误通过事件通道处理
        );

        cx.notify();
    }

    /// 显示连接错误通知
    pub fn show_connection_error(
        &mut self,
        title: impl Into<String>,
        message: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        let notification = cx.new(|cx| {
            ErrorNotification::new(cx)
                .with_title(title)
                .with_message(message)
                .with_retry(true)
                .with_auto_dismiss(false)
        });

        self.error_notification = Some(notification);
        cx.notify();
    }

    /// 关闭错误通知
    fn close_error_notification(&mut self, cx: &mut Context<Self>) {
        if self.error_notification.is_some() {
            self.error_notification = None;
            cx.notify();
        }
    }

    /// 处理 Tab 管理器事件
    fn on_tab_manager_event(
        &mut self,
        _tab_manager: Entity<TabManager>,
        event: &TabManagerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TabManagerEvent::TabAdded(tab_id, _info) => {
                info!("Tab added: {}", tab_id);
                // Tab 添加时 SplitManager 和 SplitView 已在 add_tab 中创建
            },
            TabManagerEvent::TabClosed(tab_id) => {
                info!("Tab closed: {}", tab_id);
                self.cleanup_tab(*tab_id, cx);
            },
            TabManagerEvent::TabSwitched(tab_id) => {
                info!("Tab switched to: {}", tab_id);
                self.on_tab_switched(*tab_id, cx);
                // Tab 切换时同步状态栏
                self.sync_status_bar_from_coordinator(cx);
            },
            TabManagerEvent::TabUpdated(tab_id) => {
                info!("Tab updated: {}", tab_id);
            },
            TabManagerEvent::TabMoved(tab_id, new_index) => {
                info!("Tab moved: {} to index {}", tab_id, new_index);
            },
        }
    }

    /// 处理 TabView 事件
    fn on_tab_view_event(
        &mut self,
        _tab_view: Entity<TabView>,
        event: &TabViewEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TabViewEvent::NewTabRequested => {
                info!("收到新建 Tab 请求");
                if let Some(tab_id) = self.create_local_tab("新终端", cx) {
                    // 激活新创建的本地 tab
                    self.tab_manager.update(cx, |manager, cx| {
                        manager.switch_to_tab(tab_id, cx);
                    });
                }
            },
        }
    }

    /// Tab 切换时的处理
    fn on_tab_switched(&mut self, tab_id: TabId, cx: &mut Context<Self>) {
        // 更新状态栏显示当前 Tab 的连接信息
        if let Some(tab_info) = self.tab_manager.read(cx).get_tab(tab_id) {
            if let Some(host_config) = &tab_info.host_config {
                self.update_user_host(
                    Some(host_config.username.clone()),
                    Some(host_config.host.clone()),
                    cx,
                );
                self.update_connection_status(ConnectionStatus::Connected, cx);
            } else {
                // 本地终端或无连接
                self.update_user_host(None, None, cx);
                self.update_connection_status(ConnectionStatus::Disconnected, cx);
            }
        }
    }

    /// 清理 Tab 相关资源
    ///
    /// 关闭 SSH 连接并清理相关资源
    fn cleanup_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) {
        // 收集需要关闭的 coordinators
        let coordinators_to_close: Vec<_> =
            self.terminal_pane_manager.get_coordinators_for_tab(tab_id);

        // 异步关闭 SSH 连接
        if !coordinators_to_close.is_empty() {
            let tab_id_clone = tab_id;
            crate::app::runtime::spawn(async move {
                for coordinator in coordinators_to_close {
                    coordinator.close().await;
                }
                info!("Tab {} 的 SSH 连接已关闭", tab_id_clone);
            });
        }

        // 移除 SplitView
        if let Some(_split_view) = self.split_views.remove(&tab_id) {
            // 从 TabView 中移除
            self.tab_view.update(cx, |view, _cx| {
                view.unregister_split_view(tab_id);
            });
        }

        // 清理该 Tab 下的所有终端面板
        self.terminal_pane_manager.cleanup_tab(tab_id);

        // 从主 terminal_panes 中移除
        let pane_ids_to_remove: Vec<PaneId> = self
            .terminal_panes
            .iter()
            .filter(|(_, data)| data.tab_id == tab_id)
            .map(|(id, _)| *id)
            .collect();

        for pane_id in pane_ids_to_remove {
            self.terminal_panes.remove(&pane_id);
        }

        // 从 TabView 中移除所有终端视图
        self.tab_view.update(cx, |view, _cx| {
            view.unregister_all_terminal_views(tab_id);
        });

        info!("Tab {} 已清理", tab_id);

        // 如果没有更多 Tab，更新状态栏
        if !self.tab_manager.read(cx).has_tabs() {
            self.update_connection_status(ConnectionStatus::Disconnected, cx);
            self.update_user_host(None, None, cx);
        }
    }

    // ==================== Tab 管理 ====================

    /// 创建新的 Tab（带独立 SplitManager）
    pub fn create_tab(&mut self, tab_info: TabInfo, cx: &mut Context<Self>) -> Option<TabId> {
        // 创建独立的 SplitManager
        let split_manager = cx.new(|_cx| SplitManager::new());

        // 创建 SplitView
        let split_view = cx.new(|cx| SplitView::new(split_manager.clone(), cx));

        // 添加到 Tab 管理器
        let tab_id = self.tab_manager.update(cx, |manager, cx| {
            manager.add_tab(tab_info, split_manager, cx)
        })?;

        // 存储 SplitView
        self.split_views.insert(tab_id, split_view.clone());

        // 注册到 TabView
        self.tab_view.update(cx, |view, _cx| {
            view.register_split_view(tab_id, split_view);
        });

        info!("Created new tab: {}", tab_id);
        cx.notify();

        Some(tab_id)
    }

    /// 创建新的本地终端 Tab
    pub fn create_local_tab(
        &mut self,
        title: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let tab_info = TabInfo::new_local(title);
        self.create_tab(tab_info, cx)
    }

    /// 创建新的 SSH 连接 Tab
    pub fn create_ssh_tab(
        &mut self,
        host_config: &zeterm_core::entities::HostConfig,
        cx: &mut Context<Self>,
    ) -> Option<TabId> {
        let tab_info = TabInfo::new_ssh(host_config.clone());
        self.create_tab(tab_info, cx)
    }

    /// 关闭指定 Tab
    pub fn close_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) -> bool {
        self.tab_manager
            .update(cx, |manager, cx| manager.close_tab(tab_id, cx))
    }

    /// 关闭当前活动 Tab
    pub fn close_active_tab(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(tab_id) = self.tab_manager.read(cx).active_tab_id() {
            self.close_tab(tab_id, cx)
        } else {
            false
        }
    }

    /// 切换到下一个 Tab
    pub fn switch_to_next_tab(&mut self, cx: &mut Context<Self>) -> bool {
        self.tab_manager
            .update(cx, |manager, cx| manager.switch_to_next_tab(cx))
    }

    /// 切换到上一个 Tab
    pub fn switch_to_prev_tab(&mut self, cx: &mut Context<Self>) -> bool {
        self.tab_manager
            .update(cx, |manager, cx| manager.switch_to_prev_tab(cx))
    }

    /// 切换到指定索引的 Tab
    pub fn switch_to_tab_at_index(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        self.tab_manager
            .update(cx, |manager, cx| manager.switch_to_tab_at_index(index, cx))
    }

    /// 获取当前活动 Tab ID
    pub fn active_tab_id(&self, cx: &Context<Self>) -> Option<TabId> {
        self.tab_manager.read(cx).active_tab_id()
    }

    /// 获取当前活动 Tab 的 SplitManager
    pub fn active_split_manager(&self, cx: &Context<Self>) -> Option<Entity<SplitManager>> {
        let tab_id = self.tab_manager.read(cx).active_tab_id()?;
        self.tab_manager.read(cx).get_split_manager(tab_id).cloned()
    }

    /// 获取当前活动 Pane 的 ID
    pub fn active_pane_id(&self, cx: &Context<Self>) -> Option<PaneId> {
        let tab_id = self.tab_manager.read(cx).active_tab_id()?;
        let split_manager = self.tab_manager.read(cx).get_split_manager(tab_id)?;
        split_manager.read(cx).focused_pane()
    }

    /// 获取 Tab 数量
    pub fn tab_count(&self, cx: &Context<Self>) -> usize {
        self.tab_manager.read(cx).tab_count()
    }

    /// 是否有 Tab
    pub fn has_tabs(&self, cx: &Context<Self>) -> bool {
        self.tab_manager.read(cx).has_tabs()
    }

    // ==================== 终端面板管理 ====================

    /// 在指定 Tab 中添加终端面板
    pub fn add_terminal_pane(
        &mut self,
        tab_id: TabId,
        title: impl Into<String>,
        coordinator: Arc<SessionCoordinator>,
        cx: &mut Context<Self>,
    ) -> Option<PaneId> {
        let title = title.into();

        // 获取该 Tab 的 SplitManager
        let split_manager = self.tab_manager.read(cx).get_split_manager(tab_id)?.clone();

        // 创建新的终端面板
        let new_pane = Pane::new_terminal(&title);
        let pane_id = new_pane.id;

        // 创建 TerminalView 实体
        let terminal_view = cx.new(|cx| TerminalView::new(coordinator.clone(), cx));

        // 初始化终端主题（确保新创建的终端使用当前应用主题）
        let terminal_theme = self.theme_manager.terminal_theme().clone();
        terminal_view.update(cx, |view, _cx| {
            view.set_theme(terminal_theme);
        });

        // 存储终端面板数据
        self.terminal_panes.insert(
            pane_id,
            TerminalPaneData {
                terminal_view: terminal_view.clone(),
                coordinator,
                tab_id,
            },
        );

        // 注册到对应的 SplitView
        if let Some(split_view) = self.split_views.get(&tab_id) {
            split_view.update(cx, |view, _cx| {
                view.register_terminal_view(pane_id, terminal_view.clone());
            });
        }

        // 注册到 TabView
        self.tab_view.update(cx, |view, _cx| {
            view.register_terminal_view(tab_id, pane_id, terminal_view);
        });

        // 添加到 SplitManager
        let has_panes = split_manager.read(cx).has_panes();
        if !has_panes {
            // 没有面板，设置为根面板
            split_manager.update(cx, |manager, cx| {
                manager.set_root(new_pane, cx);
                manager.focus_pane(pane_id, cx);
            });
        } else {
            // 已有面板，在当前焦点面板旁边水平分屏
            if let Some(focused_id) = split_manager.read(cx).focused_pane() {
                split_manager.update(cx, |manager, cx| {
                    manager.split_horizontal(focused_id, cx);
                });
            }
        }

        info!(
            "Added terminal pane: {} (id: {}) to tab: {}",
            title, pane_id, tab_id
        );
        cx.notify();

        Some(pane_id)
    }

    /// 在指定 Tab 中添加 SSH 终端面板
    pub fn add_ssh_terminal_pane(
        &mut self,
        tab_id: TabId,
        host_config: &zeterm_core::entities::HostConfig,
        coordinator: Arc<SessionCoordinator>,
        cx: &mut Context<Self>,
    ) -> Option<PaneId> {
        // 更新状态栏用户主机信息
        self.update_user_host(
            Some(host_config.username.clone()),
            Some(host_config.host.clone()),
            cx,
        );

        let title = format!("{}@{}", host_config.username, host_config.host);

        self.add_terminal_pane(tab_id, title, coordinator, cx)
    }

    /// 关闭指定终端面板
    pub fn close_terminal_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) {
        if let Some(pane_data) = self.terminal_panes.remove(&pane_id) {
            let tab_id = pane_data.tab_id;

            // 从 SplitView 中移除
            if let Some(split_view) = self.split_views.get(&tab_id) {
                split_view.update(cx, |view, _cx| {
                    view.unregister_terminal_view(pane_id);
                });
            }

            // 从 TabView 中移除
            self.tab_view.update(cx, |view, _cx| {
                view.unregister_terminal_view(tab_id, pane_id);
            });

            // 从 SplitManager 中移除
            let split_manager = self.tab_manager.read(cx).get_split_manager(tab_id).cloned();
            if let Some(split_manager) = split_manager {
                split_manager.update(cx, |manager, cx| {
                    manager.close_pane(pane_id, cx);
                });
            }

            info!("Closed terminal pane: {}", pane_id);
        }

        // 如果当前 Tab 没有更多面板，更新状态
        if let Some(tab_id) = self.active_tab_id(cx) {
            let has_panes = self
                .terminal_panes
                .values()
                .any(|data| data.tab_id == tab_id);

            if !has_panes {
                self.update_connection_status(ConnectionStatus::Disconnected, cx);
                self.update_user_host(None, None, cx);
            }
        }
    }

    /// 获取指定面板的 TerminalView
    pub fn get_terminal_view(&self, pane_id: PaneId) -> Option<&Entity<TerminalView>> {
        self.terminal_pane_manager.get_terminal_view(pane_id)
    }

    /// 获取指定面板的 SessionCoordinator
    pub fn get_coordinator(&self, pane_id: PaneId) -> Option<&Arc<SessionCoordinator>> {
        self.terminal_pane_manager.get_coordinator(pane_id)
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.terminal_pane_manager.is_connected()
    }

    // ==================== 分屏操作 ====================

    /// 在当前活动 Tab 中水平分屏
    pub fn split_horizontal(&mut self, cx: &mut Context<Self>) {
        self.split_with_direction(SplitDirection::Horizontal, cx);
    }

    /// 在当前活动 Tab 中垂直分屏
    pub fn split_vertical(&mut self, cx: &mut Context<Self>) {
        self.split_with_direction(SplitDirection::Vertical, cx);
    }

    /// 使用指定方向进行分屏（内部方法）
    ///
    /// 创建完整的终端视图并执行分屏操作，包括：
    /// 1. 创建新的 SessionCoordinator
    /// 2. 创建新的 TerminalView Entity
    /// 3. 创建新的 Pane 数据结构
    /// 4. 注册到各个管理器
    /// 5. 执行分屏布局
    fn split_with_direction(&mut self, direction: SplitDirection, cx: &mut Context<Self>) {
        // 获取当前活动 Tab
        let tab_id = match self.tab_manager.read(cx).active_tab_id() {
            Some(id) => id,
            None => {
                info!("Cannot split: no active tab");
                return;
            },
        };

        // 获取 SplitManager
        let split_manager = match self.active_split_manager(cx) {
            Some(sm) => sm,
            None => {
                info!("Cannot split: no split manager for tab {}", tab_id);
                return;
            },
        };

        // 获取当前焦点面板
        let focused_pane_id = match split_manager.read(cx).focused_pane() {
            Some(id) => id,
            None => {
                info!("Cannot split: no focused pane");
                return;
            },
        };

        // 创建新的终端配置和协调器（本地终端）
        let config = TerminalConfig::default();
        let coordinator = Arc::new(SessionCoordinator::new(config));

        // 创建新的终端面板
        let new_pane = Pane::new_terminal("Terminal");
        let new_pane_id = new_pane.id;

        // 创建 TerminalView
        let terminal_view = cx.new(|cx| TerminalView::new(coordinator.clone(), cx));

        // 初始化终端主题（确保新创建的终端使用当前应用主题）
        let terminal_theme = self.theme_manager.terminal_theme().clone();
        terminal_view.update(cx, |view, _cx| {
            view.set_theme(terminal_theme);
        });

        // 存储终端面板数据
        self.terminal_panes.insert(
            new_pane_id,
            TerminalPaneData {
                terminal_view: terminal_view.clone(),
                coordinator,
                tab_id,
            },
        );

        // 注册到 SplitView
        if let Some(split_view) = self.split_views.get(&tab_id) {
            split_view.update(cx, |view, _cx| {
                view.register_terminal_view(new_pane_id, terminal_view.clone());
            });
        }

        // 注册到 TabView
        self.tab_view.update(cx, |view, _cx| {
            view.register_terminal_view(tab_id, new_pane_id, terminal_view);
        });

        // 执行分屏
        split_manager.update(cx, |manager, cx| {
            manager.split_with_pane(focused_pane_id, new_pane, direction, 0.5, cx);
        });

        let direction_name = match direction {
            SplitDirection::Horizontal => "horizontal",
            SplitDirection::Vertical => "vertical",
        };
        info!(
            "Split {} with new terminal pane: {} in tab {}",
            direction_name, new_pane_id, tab_id
        );
        cx.notify();
    }

    /// 关闭当前焦点面板
    pub fn close_focused_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(split_manager) = self.active_split_manager(cx) {
            if let Some(pane_id) = split_manager.read(cx).focused_pane() {
                self.close_terminal_pane(pane_id, cx);
            }
        }
    }

    /// 切换到下一个面板
    pub fn focus_next_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(split_manager) = self.active_split_manager(cx) {
            split_manager.update(cx, |manager, cx| {
                manager.focus_next_pane(cx);
            });
        }
    }

    /// 切换到上一个面板
    pub fn focus_prev_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(split_manager) = self.active_split_manager(cx) {
            split_manager.update(cx, |manager, cx| {
                manager.focus_prev_pane(cx);
            });
        }
    }

    // ==================== 状态栏管理 ====================

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
        for pane_data in self.terminal_panes.values() {
            pane_data.terminal_view.update(cx, |view, _cx| {
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

    /// 是否显示侧边栏
    pub fn is_sidebar_visible(&self) -> bool {
        self.show_sidebar
    }

    /// 设置侧边栏可见性
    pub fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.show_sidebar = visible;
        cx.notify();
    }

    // ==================== Tab 栏管理 ====================

    /// 切换 Tab 栏
    pub fn toggle_tab_bar(&mut self, cx: &mut Context<Self>) {
        self.show_tab_bar = !self.show_tab_bar;
        cx.notify();
    }

    /// 是否显示 Tab 栏
    pub fn is_tab_bar_visible(&self) -> bool {
        self.show_tab_bar
    }

    /// 设置 Tab 栏可见性
    pub fn set_tab_bar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.show_tab_bar = visible;
        cx.notify();
    }

    // ==================== 渲染 ====================

    /// 渲染欢迎界面（无 Tab 时显示）
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
                    .text_2xl()
                    .mb_6()
                    .text_color(theme.foreground)
                    .child("🚀"),
            )
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
                    .text_lg()
                    .text_center()
                    .text_color(theme.muted_foreground)
                    .mb_6()
                    .child("现代化的 SSH 终端管理器"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("• 从左侧主机列表选择主机连接")
                    .child("• 按 Ctrl+T 新建标签页")
                    .child("• 按 Ctrl+\\ 水平分屏")
                    .child("• 按 Ctrl+Shift+- 垂直分屏"),
            )
    }

    /// 渲染 Tab 区域
    fn render_tab_area(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.show_tab_bar {
            div()
                .id("tab-area")
                .flex_1()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .child(self.tab_view.clone())
                .into_any_element()
        } else {
            // 不显示 Tab 栏时，直接渲染当前活动 Tab 的内容
            if let Some(tab_id) = self.active_tab_id(cx) {
                if let Some(split_view) = self.split_views.get(&tab_id) {
                    return div()
                        .id("content-area")
                        .flex_1()
                        .w_full()
                        .h_full()
                        .child(split_view.clone())
                        .into_any_element();
                }
            }
            // 无内容时显示欢迎界面
            div()
                .id("welcome-area")
                .flex_1()
                .w_full()
                .h_full()
                .child(self.render_welcome(cx))
                .into_any_element()
        }
    }
    /// 显示新建主机对话框
    fn show_new_host_dialog(&mut self, cx: &mut Context<Self>) {
        tracing::info!("显示新建主机对话框");

        // 获取全局数据库实例
        let Some(database) = crate::app::global_database() else {
            tracing::error!("数据库未初始化，无法显示对话框");
            return;
        };

        // 创建 SqliteHostRepository
        let repository = Arc::new(SqliteHostRepository::new(database.pool().clone()));

        // 获取主机列表的共享状态
        let hosts = self.host_list_view.read(cx).hosts().clone();

        let dialog_ref = self.connection_dialog.clone();
        let dialog_ref_for_cancel = self.connection_dialog.clone();

        let dialog = cx.new(|cx| {
            HostConnectionDialog::new_create(cx)
                .with_on_save(move |config| {
                    Self::save_host_async(repository.clone(), hosts.clone(), config, true);
                    // 关闭对话框
                    *dialog_ref.lock().unwrap() = None;
                })
                .with_on_cancel(move || {
                    tracing::info!("取消新建主机");
                    // 关闭对话框
                    *dialog_ref_for_cancel.lock().unwrap() = None;
                })
        });

        // 存储对话框引用
        *self.connection_dialog.lock().unwrap() = Some(dialog);

        cx.notify();
    }

    /// 显示编辑主机对话框
    fn show_edit_host_dialog(&mut self, host: HostConfig, cx: &mut Context<Self>) {
        tracing::info!("显示编辑主机对话框: {}", host.name);

        // 获取全局数据库实例
        let Some(database) = crate::app::global_database() else {
            tracing::error!("数据库未初始化，无法显示对话框");
            return;
        };

        // 创建 SqliteHostRepository
        let repository = Arc::new(SqliteHostRepository::new(database.pool().clone()));

        // 获取主机列表的共享状态
        let hosts = self.host_list_view.read(cx).hosts().clone();

        let host_clone = host.clone();
        let dialog_ref = self.connection_dialog.clone();
        let dialog_ref_for_cancel = self.connection_dialog.clone();

        let dialog = cx.new(|cx| {
            HostConnectionDialog::new_edit(host_clone, cx)
                .with_on_save(move |config| {
                    Self::save_host_async(repository.clone(), hosts.clone(), config, false);
                    // 关闭对话框
                    *dialog_ref.lock().unwrap() = None;
                })
                .with_on_cancel(move || {
                    tracing::info!("取消编辑主机");
                    // 关闭对话框
                    *dialog_ref_for_cancel.lock().unwrap() = None;
                })
        });

        // 存储对话框引用
        *self.connection_dialog.lock().unwrap() = Some(dialog);

        cx.notify();
    }

    /// 异步保存主机配置（静态方法）
    fn save_host_async(
        repository: Arc<SqliteHostRepository>,
        hosts: Arc<RwLock<Vec<HostConfig>>>,
        config: HostConfig,
        is_new: bool,
    ) {
        let host_name = config.name.clone();

        if is_new {
            tracing::info!("保存新主机: {}", host_name);
        } else {
            tracing::info!("更新主机: {}", host_name);
        }

        // 使用共享 Runtime 执行异步保存
        runtime::spawn_blocking(async move {
            if is_new {
                // 新建主机
                match repository.create(&config).await {
                    Ok(new_id) => {
                        tracing::info!("成功创建主机: {} (ID: {})", host_name, new_id);

                        // 创建包含新 ID 的配置
                        let mut saved_config = config.clone();
                        saved_config.id = Some(new_id);

                        // 添加到内存列表
                        if let Ok(mut hosts_guard) = hosts.write() {
                            hosts_guard.push(saved_config);
                        }
                    },
                    Err(e) => {
                        tracing::error!("创建主机失败: {:?}", e);
                    },
                }
            } else {
                // 更新主机
                match repository.update(&config).await {
                    Ok(_) => {
                        tracing::info!("成功更新主机: {}", host_name);

                        // 更新内存中的配置
                        if let Ok(mut hosts_guard) = hosts.write() {
                            if let Some(pos) = hosts_guard.iter().position(|h| h.id == config.id) {
                                hosts_guard[pos] = config.clone();
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("更新主机失败: {:?}", e);
                    },
                }
            }
        });
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 注意：不在 render 中修改状态
        // 新建 Tab 请求通过 TabViewEvent 事件处理
        // 状态栏同步在 TabSwitched 事件中处理

        // 处理待显示的错误
        let errors: Vec<String> = self.pending_errors.lock().unwrap().drain(..).collect();
        for error in errors {
            self.show_connection_error("连接失败", error, cx);
        }

        let has_tabs = self.has_tabs(cx);

        // 获取主题颜色（在可变借用之后）
        let theme = cx.theme();
        let background = theme.background;
        let secondary = theme.secondary;
        let border = theme.border;

        // 主内容区域
        let main_content = if has_tabs {
            // 有 Tab 时渲染 Tab 区域
            self.render_tab_area(cx).into_any_element()
        } else {
            // 无 Tab 时渲染欢迎界面
            div()
                .id("welcome-panel")
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .bg(background)
                .child(self.render_welcome(cx))
                .into_any_element()
        };

        // 错误通知（浮动在右上角）
        let error_notification = self.error_notification.clone();

        // 构建主布局（垂直布局：内容区 + 状态栏）
        let mut root = div()
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
                            div()
                                .id("sidebar")
                                .w(px(280.0))
                                .h_full()
                                .flex_shrink_0()
                                .border_r_1()
                                .border_color(border)
                                .bg(secondary)
                                .child(self.host_list_view.clone())
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    })
                    .child(
                        // 右侧主区域
                        div()
                            .id("main-area")
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .bg(background)
                            .child(main_content),
                    ),
            )
            .child(
                // 底部状态栏
                self.status_bar.clone(),
            );

        // 如果有连接对话框，添加到根元素（作为覆盖层）
        if let Some(ref dialog) = *self.connection_dialog.lock().unwrap() {
            root = root.child(dialog.clone());
        }

        // 如果有错误通知，显示在右上角
        if let Some(ref notification) = error_notification {
            root = root.child(
                div()
                    .absolute()
                    .top(px(16.0))
                    .right(px(16.0))
                    .child(notification.clone()),
            );
        }

        root
    }
}

impl MainWindow {
    /// 根据当前活动 pane 的 SessionCoordinator 状态同步状态栏
    ///
    /// 这是方案 3 的核心实现：利用现有的 SessionCoordinator 状态
    /// 作为单一数据源，确保 UI 状态与实际连接状态一致。
    fn sync_status_bar_from_coordinator(&mut self, cx: &mut Context<Self>) {
        // 获取当前活动 pane 的协调器
        if let Some(pane_id) = self.active_pane_id(cx) {
            if let Some(coordinator) = self.terminal_pane_manager.get_coordinator(pane_id) {
                // 使用 ConnectionManager 来同步状态栏
                ConnectionManager::sync_status_bar_from_coordinator(
                    coordinator,
                    &self.status_bar,
                    cx,
                );
            }
        }
    }
}
