//! Traits 模块
//!
//! 定义 Zeterm 核心抽象接口。

mod connection;

pub use connection::{ConnectionInfo, ConnectionType, TerminalConnection};
