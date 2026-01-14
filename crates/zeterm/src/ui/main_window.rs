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
    ActiveTheme, Sizable, Size,
    button::{Button, ButtonVariants},
    theme,
};
use parking_lot::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::app::session::SessionCoordinator;
use crate::ui::dialogs::{
    HostKeyConfirmChannel, HostKeyDialog, HostKeyInfo, HostKeyRequestReceiver, HostKeyResponse,
};
use crate::ui::terminal_view::TerminalView;
use zeterm_core::ConnectionState;
use zeterm_mock::{MockConfig, MockConnection};
use zeterm_ssh::{HostKeyConfirmCallback, SshConfig, SshConnection};

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
        }
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

    /// 渲染主内容区域
    fn render_content(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _theme = cx.theme();
        let is_connected = self.coordinator.is_connected();

        if is_connected {
            // 确保 TerminalView 已创建
            if self.terminal_view.is_none() {
                let coordinator = self.coordinator.clone();
                self.terminal_view = Some(cx.new(|cx| TerminalView::new(coordinator, cx)));
                info!("TerminalView created");
            }

            // 渲染终端视图
            div()
                .id("content")
                .flex_1()
                .w_full()
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
        } else {
            // 未连接时清理 terminal_view 并显示欢迎界面
            if self.terminal_view.is_some() {
                self.terminal_view = None;
                info!("TerminalView cleared");
            }

            div()
                .id("content")
                .flex_1()
                .w_full()
                .flex()
                .flex_col()
                .child(self.render_welcome(cx))
                .into_any_element()
        }
    }

    /// 渲染欢迎界面
    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
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
                            .text_color(theme.muted_foreground)
                            .child("Phase 3: SSH集成测试"),
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
                            .border_color(theme.border)
                            .bg(theme.secondary)
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("SSH Connection Settings"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("Host: {}:{}", ssh_host, ssh_port)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted_foreground)
                                    .child(format!("Username: {}", ssh_username)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted_foreground)
                                    .child("Password: ********"),
                            ),
                    )
                    // 连接按钮组
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                Button::new("btn-ssh-connect")
                                    .label("Connect SSH")
                                    .primary()
                                    .with_size(Size::Medium)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.start_ssh_session(cx);
                                    })),
                            )
                            .child(
                                Button::new("btn-mock-connect")
                                    .label("Connect Mock")
                                    .with_size(Size::Medium)
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.start_mock_session(cx);
                                    })),
                            ),
                    )
                    // 终端尺寸信息
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "Terminal Size: {}x{}",
                                terminal_size.cols, terminal_size.rows
                            )),
                    )
                    // 提示信息
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme.muted_foreground)
                            .child("提示: 修改 main_window.rs 中的 ssh_host/username/password 来配置连接"),
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
