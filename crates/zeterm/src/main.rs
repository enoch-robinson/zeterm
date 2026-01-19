//! Zeterm - 现代化SSH 终端客户端
//!
//! 基于 Rust + GPUI 的跨平台 SSH 终端平台。

mod app;
mod logging;
mod ui;

use gpui::{
    Application, Bounds, KeyBinding, TitlebarOptions, WindowBounds, WindowKind, WindowOptions,
    actions, prelude::*, px, size,
};
use tracing::info;
use ui::MainWindow;

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
            .open_window(window_options, |_window, cx| {
                // 先创建 SplitManager
                let split_manager = cx.new(|_cx| crate::ui::split_pane::SplitManager::new());
                // 创建 MainWindow Entity
                cx.new(|cx| MainWindow::new(split_manager, cx))
            })
            .expect("Failed to create main window");

        info!("Main window created");

        // 激活窗口
        window
            .update(cx, |_view, window, _cx| {
                window.activate_window();
            })
            .expect("Failed to activate window");

        info!("Zeterm initialized successfully");
        info!("Click'Connect Mock' button to start a mock session");
    });
}
