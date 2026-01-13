//! 会话管理模块
//!
//! 包含 SessionCoordinator 和 ConnectionManager，负责协调终端连接和数据流。

mod connection_manager;
mod session_coordinator;

pub use connection_manager::ConnectionManager;
pub use session_coordinator::SessionCoordinator;

