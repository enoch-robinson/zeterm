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
use std::process;
use tracing::{error, info, warn};
use ui::MainWindow;
use zeterm_storage::{ConnectionHistoryRepository, Database, SqliteConnectionHistoryRepository};

// 引入共享 Runtime 模块
use app::runtime;

// 引入 GPUI Component
use gpui_component;

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

/// 初始化数据库并清理僵尸连接记录
///
/// 在应用启动时执行以下操作：
/// 1. 创建/连接数据库
/// 2. 运行数据库迁移
/// 3. 清理上次崩溃遗留的"连接中"状态记录
///
/// # Returns
///
/// 返回初始化完成的数据库实例，如果失败则返回错误
fn init_database() -> Result<std::sync::Arc<Database>, Box<dyn std::error::Error + Send + Sync>> {
    info!("Initializing database...");

    // 使用共享 Runtime 执行异步初始化
    runtime::block_on(async {
        // 1. 创建数据库连接
        let db = Database::with_default_path().await?;

        // 2. 运行数据库迁移
        db.init().await?;
        info!("Database migrations completed");

        // 3. 清理僵尸连接记录（上次崩溃遗留的"连接中"状态）
        let history_repo = SqliteConnectionHistoryRepository::new(db.pool().clone());
        match history_repo.mark_all_active_as_disconnected().await {
            Ok(count) => {
                if count > 0 {
                    warn!(
                        "Cleaned up {} zombie connection record(s) from previous session",
                        count
                    );
                } else {
                    info!("No zombie connection records found");
                }
            },
            Err(e) => {
                error!("Failed to clean up zombie connections: {}", e);
            },
        }

        info!("Database initialization completed");

        // 返回数据库实例
        Ok(std::sync::Arc::new(db))
    })
}

fn main() {
    // 初始化日志系统
    logging::init_logging(logging::LogConfig::development());

    info!("Zeterm starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // 初始化共享 Tokio Runtime（必须在其他异步操作前调用）
    runtime::init();
    info!("Shared Tokio runtime initialized");

    // 初始化数据库（在 GPUI 启动前完成）
    let database = match init_database() {
        Ok(db) => db,
        Err(e) => {
            error!("Database initialization failed: {}", e);
            eprintln!("Fatal error: Failed to initialize database: {}", e);
            eprintln!("Please check database permissions and configuration.");
            process::exit(1);
        },
    };
    app::init_global_database(database);
    info!("Global database initialized and ready for use");

    // 初始化 GPUI 应用
    Application::new().run(|cx| {
        info!("GPUI App initialized");

        // 初始化 GPUI Component 主题系统（必须在使用任何 GPUI Component 功能之前调用）
        gpui_component::init(cx);

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
        let window = match cx.open_window(window_options, |_window, cx| {
            // 创建 MainWindow Entity（内部自动管理 Tab 和 SplitManager）
            cx.new(|cx| MainWindow::new(cx))
        }) {
            Ok(window) => window,
            Err(e) => {
                error!("Failed to create main window: {:?}", e);
                cx.quit();
                return;
            },
        };

        info!("Main window created");

        // 激活窗口
        if let Err(e) = window.update(cx, |_view, window, _cx| {
            window.activate_window();
        }) {
            error!("Failed to activate window: {:?}", e);
        }

        info!("Zeterm initialized successfully");
        info!("Click'Connect Mock' button to start a mock session");
    });
}
