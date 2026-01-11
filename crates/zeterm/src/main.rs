//! Zeterm -现代化SSH 终端客户端
//!
//! 基于 Rust + GPUI 的跨平台 SSH 终端平台。

mod logging;

use tracing::info;

fn main() {
    // 初始化日志系统
    logging::init_logging(logging::LogConfig::development());

    info!("Zeterm starting...");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // TODO: Phase 1 - 初始化 GPUI 应用
    // TODO: Phase 1 - 创建主窗口
    // TODO: Phase 1 - 启动事件循环

    info!("Zeterm initialized successfully");

    // 临时：保持程序运行以查看日志输出
    println!(
        "Zeterm v{} - Press Ctrl+C to exit",
        env!("CARGO_PKG_VERSION")
    );
}
