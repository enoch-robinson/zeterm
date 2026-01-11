//! Zeterm -现代化SSH终端客户端
//!
//! 基于 Rust + GPUI 的跨平台 SSH 终端平台。

mod app;
mod logging;
mod ui;

use gpui::{
    Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowKind, WindowOptions,
    actions, px, size,
};
use tracing::{debug, info};
use ui::MainWindow;

use app::terminal::{TerminalConfig, TerminalState};
use zeterm_core::traits::{ConnectionInfo, TerminalConnection};
use zeterm_mock::{MockConfig, MockConnection};

// 定义应用程序 Actions
actions!(zeterm, [Quit]);

///窗口默认宽度
const DEFAULT_WINDOW_WIDTH: f32 = 1200.0;
/// 窗口默认高度
const DEFAULT_WINDOW_HEIGHT: f32 = 800.0;
/// 窗口最小宽度
const MIN_WINDOW_WIDTH: f32 = 640.0;
/// 窗口最小高度
const MIN_WINDOW_HEIGHT: f32 = 480.0;

fn main() {
    // 初始化日志系统
    logging::init_logging(logging::LogConfig::development());

    info!("Zeterm starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // 初始化 GPUI 应用
    Application::new().run(|cx| {
        info!("GPUI App initialized");

        // 注册全局快捷键绑定
        cx.bind_keys([
            // Ctrl+Q 退出应用
            KeyBinding::new("ctrl-q", Quit, None),
            // Ctrl+Shift+Q 也可以退出
            KeyBinding::new("ctrl-shift-q", Quit, None),
        ]);
        info!("Keybindings registered");

        // 注册退出 Action
        cx.on_action(|_: &Quit, cx| {
            info!("Quit action triggered, exiting...");
            cx.quit();
        });
        info!("Actions registered");

        // 配置窗口选项
        let window_options = WindowOptions {
            // 窗口边界：设置默认尺寸
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)),
                cx,
            ))),
            // 窗口最小尺寸
            window_min_size: Some(size(px(MIN_WINDOW_WIDTH), px(MIN_WINDOW_HEIGHT))),
            // 窗口类型：普通窗口
            kind: WindowKind::Normal,
            // 标题栏选项
            titlebar: Some(TitlebarOptions {
                title: Some("Zeterm".into()),
                ..Default::default()
            }),
            // 窗口是否可移动
            is_movable: true,
            // 应用 ID
            app_id: Some("zeterm".to_string()),
            ..Default::default()
        };

        // 创建窗口
        let window = cx
            .open_window(window_options, |window, cx| MainWindow::build(window, cx))
            .expect("Failed to create main window");

        info!("Main window created");

        // 激活窗口
        window
            .update(cx, |_view, window, _cx| {
                window.activate_window();
            })
            .expect("Failed to activate window");

        info!("Zeterm initialized successfully");

        // 启动 Phase 1 里程碑演示
        start_milestone_demo();
    });
}

/// Phase 1 里程碑演示
///
/// 验证 MockConnection → TerminalState 数据流
fn start_milestone_demo() {
    info!("=== Phase 1 Milestone Demo ===");

    // 在后台线程运行演示
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            run_demo().await;
        });
    });
}

/// 运行演示
async fn run_demo() {
    use futures::StreamExt;

    info!("Creating MockConnection...");

    // 创建 MockConnection
    let conn = MockConnection::new(MockConfig {
        auto_output_interval_ms: Some(1000),
        auto_output_content: "Hello World\n".to_string(),
        echo_input: true,
    });

    // 连接
    conn.connect().await.expect("Failed to connect");
    info!("MockConnection connected: {}", conn.is_connected());

    // 创建 TerminalState
    let (terminal, mut event_rx) = TerminalState::new(TerminalConfig::default());
    info!(
        "TerminalState created: {}x{}",
        terminal.size().cols,
        terminal.size().rows
    );

    // 获取数据流
    let mut stream = conn.receive_stream();

    // 处理终端事件的任务
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            debug!("Terminal event: {:?}", event);
        }
    });

    // 接收数据并送入终端状态机
    info!("Starting data pump (will run for 5 seconds)...");
    let start = std::time::Instant::now();

    while start.elapsed() < std::time::Duration::from_secs(5) {
        tokio::select! {
            Some(result) = stream.next() => {
                match result {
                    Ok(data) => {
                        let text = String::from_utf8_lossy(&data);
                        info!("[Mock Output] {}", text.trim());

                        // 送入 Alacritty 终端状态机
                        terminal.advance_bytes(&data);
                        debug!("Advanced {} bytes to terminal", data.len());
                    }
                    Err(e) => {
                        info!("Stream error: {}", e);
                break;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
    }

    // 关闭连接
    conn.close().await.expect("Failed to close");
    info!("MockConnection closed");
    info!("=== Phase 1 Milestone Demo Complete ===");
}
