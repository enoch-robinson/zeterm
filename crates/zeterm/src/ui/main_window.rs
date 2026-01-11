//! 主窗口视图组件
//!
//! 应用程序的主窗口，包含终端视图和其他 UI 元素。
//! 集成 gpui-component 的Root 和 Theme 系统。
//! 集成 SessionCoordinator 实现数据流管理。

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
use parking_lot::RwLock;
use tracing::{debug, info, warn};

use crate::app::session::SessionCoordinator;
use zeterm_core::ConnectionState;
use zeterm_mock::{MockConfig, MockConnection};

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
}

impl MainWindow {
    /// 创建新的主窗口视图
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 创建会话协调器
        let coordinator = Arc::new(SessionCoordinator::with_defaults());

        info!("MainWindow created with SessionCoordinator");

        Self {
            focus_handle: cx.focus_handle(),
            coordinator,
            status_text: Arc::new(RwLock::new("Ready".to_string())),
            data_pump_started: Arc::new(RwLock::new(false)),
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
        let theme = cx.theme();
        let is_connected = self.coordinator.is_connected();
        let terminal_size = self.coordinator.terminal_size();

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
                            .child("Phase1: SessionCoordinator + Data Pump 集成完成"),
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
                    // 连接按钮
                    .child(
                        div().flex().gap_2().child(
                            Button::new("btn-connect")
                                .label(if is_connected {
                                    "Disconnect"
                                } else {
                                    "Connect Mock"
                                })
                                .primary()
                                .with_size(Size::Medium)
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    if this.coordinator.is_connected() {
                                        // 断开连接
                                        this.disconnect(cx);
                                    } else {
                                        // 启动连接
                                        this.start_mock_session(cx);
                                    }
                                })),
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
