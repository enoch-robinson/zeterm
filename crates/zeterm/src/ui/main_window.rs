//! 主窗口视图组件
//!
//! 应用程序的主窗口，包含终端视图和其他 UI 元素。
//! 集成 gpui-component 的Root 和 Theme 系统。
//! 集成 SessionCoordinator 实现数据流管理。

use std::env;
use std::sync::Arc;

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable, Size,
    button::{Button, ButtonVariants},
    theme,
};
use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::app::session::SessionCoordinator;
use crate::ui::terminal_view::TerminalElement;
use crate::ui::terminal_view::{Modifiers, keystroke_to_bytes};
use zeterm_core::ConnectionState;
use zeterm_mock::{MockConfig, MockConnection};
use zeterm_ssh::{SshConfig, SshConnection};

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

        Self {
            focus_handle: cx.focus_handle(),
            coordinator,
            status_text: Arc::new(RwLock::new("Ready".to_string())),
            data_pump_started: Arc::new(RwLock::new(false)),
            ssh_host: Arc::new(RwLock::new(ssh_host)),
            ssh_username: Arc::new(RwLock::new(ssh_username)),
            ssh_password: Arc::new(RwLock::new(ssh_password)),
            ssh_port: Arc::new(RwLock::new(ssh_port)),
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

        // 使用 std::thread::spawn 启动独立线程
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async move {
                // 创建 SSH 配置
                let config = SshConfig::new(&host, &username)
                    .with_port(port)
                    .with_password(&password)
                    .with_terminal_size(80, 24);

                // 创建 SSH 连接
                let ssh_conn = SshConnection::new(config);

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
            .child(self.render_status_bar(window, cx, status, status_color))
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
        let _theme = cx.theme();
        let is_connected = self.coordinator.is_connected();
        let coordinator = self.coordinator.clone();

        div()
            .id("content")
            .flex_1()
            .w_full()
            .flex()
            .flex_col()
            .child(if is_connected {
                // 连接后显示终端
                div()
                    .id("terminal-container")
                    .flex_1()
                    .w_full()
                    .track_focus(&self.focus_handle)
                    .on_key_down(
                        cx.listener(move |this, event: &KeyDownEvent, _window, _cx| {
                            let key = event.keystroke.key.as_str();
                            let modifiers = Modifiers::new(
                                event.keystroke.modifiers.control,
                                event.keystroke.modifiers.alt,
                                event.keystroke.modifiers.shift,
                            );

                            let mapping = keystroke_to_bytes(key, modifiers);
                            if !mapping.is_empty() {
                                debug!("Key pressed: {} -> {:?}", key, mapping.bytes);
                                this.coordinator.send_input_sync(&mapping.bytes);
                            }
                        }),
                    )
                    .child(TerminalElement::new(
                        coordinator,
                        true, // focused
                        true, // cursor_visible
                    ))
                    .into_any_element()
            } else {
                // 未连接时显示欢迎界面
                self.render_welcome(cx).into_any_element()
            })
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
}
