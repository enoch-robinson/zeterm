//! 会话管理模块
//!
//! 包含 SessionCoordinator、ConnectionManager 和 ReconnectController，
//! 负责协调终端连接、数据流和自动重连。

mod connection_manager;
mod reconnect_controller;
mod session_coordinator;

pub use connection_manager::ConnectionManager;
pub use reconnect_controller::{
    ConnectionFactory, ReconnectController, ReconnectControllerConfig, ReconnectControllerEvent,
    SshConnectionFactory,
};
pub use session_coordinator::SessionCoordinator;
