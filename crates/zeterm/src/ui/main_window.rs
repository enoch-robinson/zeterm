//! 主窗口视图组件
//!
//! 应用程序的主窗口，包含终端视图和其他 UI 元素。
//! 集成 gpui-component 的Root 和 Theme 系统。
//! 集成 SessionCoordinator 实现数据流管理。

use std::env;
use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme,
    theme,
};
use parking_lot::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::app::session::SessionCoordinator;
use crate::ui::dialogs::{
    HostKeyConfirmChannel, HostKeyDialog, HostKeyInfo, HostKeyRequestReceiver, HostKeyResponse,
};
use crate::ui::host_list::{HostListEvent, HostListView};
use crate::ui::tab_manager::{TabId, TabInfo, TabManager, TabManagerEvent};
use crate::ui::tab_view::TabView;
use crate::ui::terminal_view::TerminalView;
use std::collections::HashMap;
use zeterm_core::ConnectionState;
use zeterm_core::entities::HostConfig;
use zeterm_mock::{MockConfig, MockConnection};
use zeterm_ssh::{HostKeyConfirmCallback, SshConfig, SshConnection};
use zeterm_storage::Database;

/// 主窗口视图
///
/// 应用程序的顶层视图组件，负责：
/// - 管理整体布局
/// - 协调子视图
/// - 处理全局快捷键
/// - 管理会话协调器
pub struct MainWindow {
    /// 焦点句柄，用于键盘事件处理
    focus_handle: FocusHandle,
    /// 会话协调器
    coordinator: Arc<SessionCoordinator>,
    /// 终端视图
    terminal_view: Option<Entity<TerminalView>>,
    /// 连接状态显示文本
    status_text: Arc<RwLock<String>>,
    /// 是否已启动数据泵
    data_pump_started: Arc<RwLock<bool>>,
    /// SSH 主机地址
    ssh_host: Arc<RwLock<String>>,
    /// SSH 用户名
    ssh_username: Arc<RwLock<String>>,
    /// SSH 密码
    ssh_password: Arc<RwLock<String>>,
    /// SSH 端口
    ssh_port: Arc<RwLock<u16>>,
    /// 主机密钥对话框
    host_key_dialog: Option<Entity<HostKeyDialog>>,
    /// 主机密钥确认通道
    host_key_channel: Arc<HostKeyConfirmChannel>,
    /// 主机密钥请求接收端
    host_key_receiver: HostKeyRequestReceiver,
    /// 当前待处理的请求 ID
    pending_request_id: Arc<Mutex<Option<u64>>>,
    /// 数据库连接
    database: Option<Arc<Database>>,
    /// 主机列表视图
    host_list_view: Option<Entity<HostListView>>,
    /// 是否显示左侧面板（主机列表）
    show_sidebar: bool,
    /// Tab 管理器
    tab_manager: Entity<TabManager>,
    /// Tab 视图
    tab_view: Entity<TabView>,
    /// 每个 Tab 对应的终端视图
    terminal_views: HashMap<TabId, Entity<TerminalView>>,
}

impl MainWindow {
    /// 创建新的主窗口视图
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 创建会话协调器
        let coordinator = Arc::new(SessionCoordinator::with_defaults());

        // 从环境变量读取 SSH 配置，支持 ZETERM_SSH_* 和 SSH_* 两种前缀
        let ssh_host = env::var("ZETERM_SSH_HOST")
            .or_else(|_| env::var("SSH_HOST"))
            .unwrap_or_else(|_| "localhost".to_string());

        let ssh_username = env::var("ZETERM_SSH_USER")
            .or_else(|_| env::var("SSH_USER"))
            .unwrap_or_else(|_| "root".to_string());

        let ssh_password = env::var("ZETERM_SSH_PASSWORD")
            .or_else(|_| env::var("SSH_PASSWORD"))
            .unwrap_or_default();

        let ssh_port: u16 = env::var("ZETERM_SSH_PORT")
            .or_else(|_| env::var("SSH_PORT"))
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(22);

        info!("MainWindow created with SessionCoordinator");
        info!(
            "SSH config from env: host={}, user={}, port={}",
            ssh_host, ssh_username, ssh_port
        );

        // 创建主机密钥确认通道
        let host_key_channel = Arc::new(HostKeyConfirmChannel::new());
        let host_key_receiver = host_key_channel.request_receiver();

        // 初始化数据库和主机列表视图
        let (database, host_list_view) = Self::init_database_and_host_list(cx);

        // 创建 Tab 管理器和 Tab 视图
        let tab_manager = cx.new(|_cx| {
            let manager = TabManager::new();
            manager
        });
        let tab_view = cx.new(|cx| TabView::new(tab_manager.clone(), cx));

        // 订阅 Tab 管理器事件
        cx.subscribe(
            &tab_manager,
            |this, _manager, event: &TabManagerEvent, cx| {
                this.handle_tab_manager_event(event, cx);
            },
        )
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            coordinator,
            terminal_view: None,
            status_text: Arc::new(RwLock::new("Ready".to_string())),
            data_pump_started: Arc::new(RwLock::new(false)),
            ssh_host: Arc::new(RwLock::new(ssh_host)),
            ssh_username: Arc::new(RwLock::new(ssh_username)),
            ssh_password: Arc::new(RwLock::new(ssh_password)),
            ssh_port: Arc::new(RwLock::new(ssh_port)),
            host_key_dialog: None,
            host_key_channel,
            host_key_receiver,
            pending_request_id: Arc::new(Mutex::new(None)),
            database,
            host_list_view,
            show_sidebar: true,
            tab_manager,
            tab_view,
            terminal_views: HashMap::new(),
        }
    }

    /// 初始化数据库和主机列表视图
    fn init_database_and_host_list(
        cx: &mut Context<Self>,
    ) -> (Option<Arc<Database>>, Option<Entity<HostListView>>) {
        // 在独立线程中初始化数据库
        let db_result = std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(async {
                // 创建数据库连接
                match Database::with_default_path().await {
                    Ok(db) => {
                        // 运行迁移
                        if let Err(e) = db.init().await {
                            warn!("Failed to run database migrations: {:?}", e);
                            return None;
                        }
                        info!("Database initialized successfully");
                        Some(Arc::new(db))
                    },
                    Err(e) => {
                        warn!("Failed to initialize database: {:?}", e);
                        None
                    },
                }
            })
        })
        .join()
        .ok()
        .flatten();

        // 如果数据库初始化成功，创建主机列表视图
        let host_list_view = if let Some(ref database) = db_result {
            let db_clone = database.clone();
            let view = cx.new(|cx| HostListView::new(db_clone, cx));

            // 订阅主机列表事件
            cx.subscribe(&view, |this, _host_list, event: &HostListEvent, cx| {
                this.handle_host_list_event(event, cx);
            })
            .detach();

            Some(view)
        } else {
            None
        };

        (db_result, host_list_view)
    }

    /// 处理主机列表事件
    fn handle_host_list_event(&mut self, event: &HostListEvent, cx: &mut Context<Self>) {
        match event {
            HostListEvent::ConnectRequested(host_config) => {
                info!("Connect requested for host: {}", host_config.name);
                self.create_tab_for_host(host_config.clone(), cx);
            },
            HostListEvent::NewHostRequested => {
                info!("New host requested");
                // 对话框已在HostListView 中处理
            },
            HostListEvent::EditHostRequested(host_config) => {
                info!("Edit host requested: {}", host_config.name);
                // 对话框已在 HostListView 中处理
            },
            HostListEvent::HostDeleted(host_id) => {
                info!("Host deleted: {}", host_id);
            },
        }
    }

    /// 处理 Tab 管理器事件
    fn handle_tab_manager_event(&mut self, event: &TabManagerEvent, cx: &mut Context<Self>) {
        match event {
            TabManagerEvent::TabAdded(tab_info) => {
                info!("Tab added: {}", tab_info.title);
                // 为新 Tab 创建终端视图
                let terminal_view = cx.new(|cx| {
                    let coordinator = self.coordinator.clone();
                    TerminalView::new(coordinator, cx)
                });
                self.terminal_views.insert(tab_info.id, terminal_view);
                cx.notify();
            },
            TabManagerEvent::TabClosed(tab_id) => {
                info!("Tab closed: {}", tab_id);
                // 移除对应的终端视图
                self.terminal_views.remove(tab_id);
                cx.notify();
            },
            TabManagerEvent::TabSwitched(tab_id) => {
                info!("Tab switched: {}", tab_id);
                cx.notify();
            },
        }
    }

    /// 为指定主机创建新 Tab 并连接
    fn create_tab_for_host(&mut self, host_config: HostConfig, cx: &mut Context<Self>) {
        // 创建新的 SSH Tab
        let tab_info = TabInfo::new_ssh(host_config.clone());
        let tab_id = tab_info.id;

        // 添加 Tab 到管理器
        self.tab_manager.update(cx, |manager, cx| {
            manager.add_tab(tab_info, cx);
        });

        // 切换到新创建的 Tab
        self.tab_manager.update(cx, |manager, cx| {
            manager.switch_to_tab(tab_id, cx);
        });

        // 连接到主机
        self.connect_to_host_with_tab(host_config, cx);
    }

    /// 连接到指定主机（使用当前活动 Tab）
    fn connect_to_host_with_tab(&mut self, host_config: HostConfig, cx: &mut Context<Self>) {
        // 检查是否已连接
        {
            let started = self.data_pump_started.read();
            if *started {
                warn!("Session already started, disconnect first");
                return;
            }
        }

        info!(
            "Connecting to host: {}@{}:{}",
            host_config.username, host_config.host, host_config.port
        );

        // 更新 SSH 配置
        *self.ssh_host.write() = host_config.host.clone();
        *self.ssh_username.write() = host_config.username.clone();
        *self.ssh_port.write() = host_config.port;

        // 获取密码（从 AuthConfig）
        let password = match &host_config.auth_config {
            zeterm_core::entities::AuthConfig::Password { password_ref } => password_ref.clone(),
            _ => String::new(),
        };
        *self.ssh_password.write() = password;

        // 启动 SSH 会话
        self.start_ssh_session(cx);
    }

    /// 连接到指定主机（保持兼容性的旧方法）
    fn connect_to_host(&mut self, host_config: HostConfig, cx: &mut Context<Self>) {
        self.create_tab_for_host(host_config, cx);
    }

    /// 在窗口上下文中构建主窗口视图
    pub fn build(window: &mut Window, cx: &mut App) -> Entity<Self> {
        // 初始化 gpui-component 主题系统
        theme::init(cx);

        cx.new(|cx| Self::new(window, cx))
    }

    /// 启动 Mock 连接和数据泵
    pub fn start_mock_session(&self, cx: &mut Context<Self>) {
        // 检查是否已启动
        {
            let started = self.data_pump_started.read();
            if *started {
                warn!("Mock session already started");
                return;
            }
        }

        info!("Starting mock session...");

        // 更新状态
        {
            *self.status_text.write() = "Connecting...".to_string();
        }

        let coordinator = self.coordinator.clone();
        let status_text = self.status_text.clone();
        let data_pump_started = self.data_pump_started.clone();

        // 使用 std::thread::spawn 启动独立线程，避免 Send + Sync 问题
        std::thread::spawn(move || {
            // 创建新的 tokio runtime
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                // 创建 MockConnection
                let mock_conn = MockConnection::new(MockConfig {
                    auto_output_interval_ms: Some(1000),
                    auto_output_content: "Hello World!\r\n".to_string(),
                    echo_input: true,
                });

                // 连接
                if let Err(e) = mock_conn.connect().await {
                    *status_text.write() = format!("Connection failed: {}", e);
                    return;
                }

                // 设置连接并获取数据流
                let stream = coordinator.set_connection(Box::new(mock_conn));
                *status_text.write() = "Connected".to_string();
                *data_pump_started.write() = true;

                info!("Mock connection established, starting data pump...");

                // 启动数据泵
                coordinator
                    .start_data_pump(stream, move || {
                        // 通知回调 - 在实际应用中这里应该触发 UI 重绘
                        debug!("Data pump notify callback triggered");
                    })
                    .await;

                info!("Data pump stopped");
                *status_text.write() = "Disconnected".to_string();
                *data_pump_started.write() = false;
            });
        });

        cx.notify();
    }

    /// 启动 SSH 连接
    pub fn start_ssh_session(&self, cx: &mut Context<Self>) {
        // 检查是否已启动
        {
            let started = self.data_pump_started.read();
            if *started {
                warn!("Session already started");
                return;
            }
        }

        info!("Starting SSH session...");

        // 获取 SSH 配置
        let host = self.ssh_host.read().clone();
        let username = self.ssh_username.read().clone();
        let password = self.ssh_password.read().clone();
        let port = *self.ssh_port.read();

        if host.is_empty() || username.is_empty() {
            *self.status_text.write() = "Please enter host and username".to_string();
            cx.notify();
            return;
        }

        // 更新状态
        {
            *self.status_text.write() = format!("Connecting to {}@{}:{}...", username, host, port);
        }

        let coordinator = self.coordinator.clone();
        let status_text = self.status_text.clone();
        let data_pump_started = self.data_pump_started.clone();

        // 获取主机密钥确认通道的发送端
        let host_key_sender = self.host_key_channel.request_sender();

        // 使用 std::thread::spawn 启动独立线程
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                // 创建 SSH 配置，使用 AskOnFirstConnect 模式进行主机密钥验证
                let config = SshConfig::new(&host, &username)
                    .with_port(port)
                    .with_password(&password)
                    .with_terminal_size(80, 24)
                    .with_host_key_verification(zeterm_ssh::HostKeyVerification::AskOnFirstConnect);

                info!("SSH config created with host key verification enabled");

                // 创建主机密钥确认回调
                // 当 SshHandler 需要确认时，通过 channel 发送请求到UI 线程
                let host_key_callback: HostKeyConfirmCallback = Arc::new(
                    move |hostname: &str,
                          port: u16,
                          key_type: &str,
                          fingerprint: &str|
                          -> Option<bool> {
                        use crate::ui::dialogs::HostKeyConfirmRequest;
                        use std::time::Duration;

                        info!(
                            "Host key confirmation requested for {}:{} ({}: {})",
                            hostname, port, key_type, fingerprint
                        );

                        // 创建确认请求
                        let request =
                            HostKeyConfirmRequest::new(hostname, port, key_type, fingerprint);

                        // 发送请求并等待响应（超时 60 秒）
                        // 返回值: None=拒绝, Some(true)=接受并保存, Some(false)=接受但不保存
                        match host_key_sender.request_and_wait(request, Duration::from_secs(60)) {
                            Some(response) => {
                                info!(
                                    "Host key confirmation response: accepted={}, remember={}",
                                    response.accepted, response.remember
                                );
                                if response.accepted {
                                    Some(response.remember)
                                } else {
                                    None
                                }
                            },
                            None => {
                                warn!("Host key confirmation timed out or channel closed");
                                None
                            },
                        }
                    },
                );

                // 创建 SSH 连接并设置回调
                let ssh_conn =
                    SshConnection::new(config).with_host_key_confirm_callback(host_key_callback);

                // 连接
                info!("Connecting to SSH server...");
                if let Err(e) = ssh_conn.connect().await {
                    *status_text.write() = format!("SSH connection failed: {}", e);
                    return;
                }

                // 设置连接并获取数据流
                let stream = coordinator.set_connection(Box::new(ssh_conn));
                *status_text.write() = format!("Connected to {}@{}:{}", username, host, port);
                *data_pump_started.write() = true;

                info!("SSH connection established, starting data pump...");

                // 启动数据泵
                coordinator
                    .start_data_pump(stream, move || {
                        debug!("Data pump notify callback triggered");
                    })
                    .await;

                info!("Data pump stopped");
                *status_text.write() = "Disconnected".to_string();
                *data_pump_started.write() = false;
            });
        });

        cx.notify();
    }
    /// 断开连接
    pub fn disconnect(&self, _cx: &mut Context<Self>) {
        let coordinator = self.coordinator.clone();
        let status_text = self.status_text.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(async move {
                coordinator.close().await;
                *status_text.write() = "Disconnected".to_string();
            });
        });
    }

    /// 获取连接状态文本
    fn get_status_text(&self) -> String {
        self.status_text.read().clone()
    }

    /// 获取连接状态
    fn get_connection_state(&self) -> ConnectionState {
        self.coordinator.connection_state()
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 先检查是否需要显示主机密钥对话框（需要可变借用）
        self.check_pending_host_key_dialog(cx);

        // 检查是否有新数据需要重绘（脏标记机制）
        // 如果数据泵接收到新数据，会设置脏标记，这里检查并触发重绘循环
        if self.coordinator.check_and_clear_dirty() {
            cx.notify();
        }

        let theme = cx.theme();
        let status = self.get_status_text();
        let connection_state = self.get_connection_state();

        // 状态指示器颜色
        let status_color = match connection_state {
            ConnectionState::Connected { .. } => gpui::rgb(0x22c55e), // green
            ConnectionState::Connecting { .. } => gpui::rgb(0xeab308), // yellow
            ConnectionState::Disconnected { .. } => gpui::rgb(0xef4444), // red
            ConnectionState::Idle => gpui::rgb(0x6b7280),             // gray
            _ => gpui::rgb(0x6b7280),                                 // gray for other states
        };

        // 主窗口容器，使用 gpui-component 主题颜色
        let mut container = div()
            .id("main-window")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_header(window, cx))
            .child(self.render_content(window, cx))
            .child(self.render_status_bar(window, cx, status, status_color));

        // 如果有对话框，添加对话框覆盖层
        if let Some(dialog) = &self.host_key_dialog {
            container = container.child(dialog.clone());
        }

        container
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

    /// 渲染主内容区域（左侧主机列表 + 右侧终端/欢迎界面）
    fn render_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 提前克隆 theme 值以避免借用冲突
        let theme = cx.theme();
        let _background = theme.background;
        let secondary = theme.secondary;
        let border = theme.border;
        let is_connected = self.coordinator.is_connected();

        // 获取当前活动 Tab 的终端视图
        let active_terminal_view = self
            .tab_manager
            .read(cx)
            .active_tab_id()
            .and_then(|tab_id| self.terminal_views.get(&tab_id).cloned());

        // 右侧主区域容器（包含 Tab 栏和终端/欢迎界面）
        let mut main_area = div().id("main-area").flex_1().flex().flex_col();

        // 渲染 Tab 栏（如果有 Tab）
        if self.tab_manager.read(cx).has_tabs() {
            main_area = main_area.child(self.tab_view.clone());
        }

        // 终端或欢迎界面
        let content_area = if is_connected {
            // 使用活动 Tab 的终端视图
            if let Some(terminal_view) = active_terminal_view {
                div()
                    .id("terminal-panel")
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("terminal-container")
                            .flex_1()
                            .w_full()
                            .child(terminal_view),
                    )
                    .into_any_element()
            } else {
                // 回退到旧的 terminal_view（兼容性）
                if self.terminal_view.is_none() {
                    let coordinator = self.coordinator.clone();
                    self.terminal_view = Some(cx.new(|cx| TerminalView::new(coordinator, cx)));
                    info!("TerminalView created (fallback)");
                }

                div()
                    .id("terminal-panel")
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("terminal-container")
                            .flex_1()
                            .w_full()
                            .child(self.terminal_view.clone().unwrap()),
                    )
                    .into_any_element()
            }
        } else {
            // 未连接时显示欢迎界面
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

    /// 切换侧边栏显示
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.show_sidebar = !self.show_sidebar;
        info!("Sidebar visibility toggled: {}", self.show_sidebar);
        cx.notify();
    }

    /// 渲染欢迎界面
    fn render_welcome(&self) -> impl IntoElement {
        let terminal_size = self.coordinator.terminal_size();

        // 获取当前 SSH 配置值
        let ssh_host = self.ssh_host.read().clone();
        let ssh_username = self.ssh_username.read().clone();
        let ssh_port = *self.ssh_port.read();

        div()
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
                    .gap_6()
                    // 欢迎标题
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
                            .text_color(gpui::rgb(0x9ca3af))
                            .child("Phase 5: Tab 管理与多会话支持"),
                    )
                    // SSH 连接信息显示
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_4()
                            .rounded_lg()
                            .border_1()
                            .border_color(gpui::rgb(0x374151))
                            .bg(gpui::rgb(0x1f2937))
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("SSH Connection Settings"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(gpui::rgb(0x9ca3af))
                                    .child(format!("Host: {}:{}", ssh_host, ssh_port)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(gpui::rgb(0x9ca3af))
                                    .child(format!("Username: {}", ssh_username)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(gpui::rgb(0x9ca3af))
                                    .child("Password: ********"),
                            ),
                    )
                    // 提示信息
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(gpui::rgb(0x9ca3af))
                            .child("从左侧主机列表选择主机进行连接"),
                    )
                    // 终端尺寸信息
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(gpui::rgb(0x9ca3af))
                            .child(format!(
                                "Terminal Size: {}x{}",
                                terminal_size.cols, terminal_size.rows
                            )),
                    ),
            )
    }

    /// 渲染底部状态栏
    fn render_status_bar(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        status: String,
        status_color: gpui::Rgba,
    ) -> impl IntoElement {
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
                    .flex()
                    .items_center()
                    .gap_2()
                    // 状态指示器圆点
                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(status_color))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.muted_foreground)
                            .child(status),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted_foreground)
                    .child("Theme: Dark"),
            )
    }

    /// 检查是否有待确认的主机密钥对话框需要显示
    fn check_pending_host_key_dialog(&mut self, cx: &mut Context<Self>) {
        // 如果已经有对话框显示，检查是否需要关闭
        if let Some(dialog) = &self.host_key_dialog {
            let response = dialog.read(cx).response();
            if response != HostKeyResponse::Pending {
                info!(
                    "Host key dialog response received: {:?}, dismissing dialog",
                    response
                );
                self.dismiss_host_key_dialog(cx);
                return;
            }
            // 对话框仍在等待用户响应，不检查新请求
            return;
        }

        // 尝试从通道接收请求（非阻塞）
        if let Some(request) = self.host_key_receiver.try_recv() {
            info!(
                "Received host key confirmation request for {}:{}",
                request.hostname, request.port
            );

            // 保存请求 ID
            *self.pending_request_id.lock() = Some(request.request_id);

            // 创建 HostKeyInfo
            let info = HostKeyInfo::new(
                &request.hostname,
                request.port,
                &request.key_type,
                &request.fingerprint,
            );

            // 显示对话框
            self.show_host_key_dialog(info, cx);
        }
    }

    /// 显示主机密钥确认对话框
    fn show_host_key_dialog(&mut self, info: HostKeyInfo, cx: &mut Context<Self>) {
        info!(
            "Showing host key dialog for {}:{}",
            info.hostname, info.port
        );

        // 获取请求 ID 和响应发送端
        let request_id = self.pending_request_id.lock().unwrap_or(0);
        let receiver = self.host_key_receiver.clone();

        let dialog = cx.new(|cx| {
            HostKeyDialog::new(info, cx).with_on_response(
                move |response, remember| match response {
                    HostKeyResponse::Accept => {
                        info!("User accepted host key, sending response");
                        receiver.accept(request_id, remember);
                    },
                    HostKeyResponse::Reject => {
                        info!("User rejected host key, sending response");
                        receiver.reject(request_id);
                    },
                    HostKeyResponse::Pending => {},
                },
            )
        });

        self.host_key_dialog = Some(dialog);
        cx.notify();
    }

    /// 关闭主机密钥对话框
    fn dismiss_host_key_dialog(&mut self, cx: &mut Context<Self>) {
        self.host_key_dialog = None;
        *self.pending_request_id.lock() = None;
        cx.notify();
    }
}
