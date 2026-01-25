//! 数据仓库模块
//!
//! 提供数据访问接口和实现。

mod connection_history;
mod host_repository;

pub use connection_history::{
    ConnectionHistoryError, ConnectionHistoryRecord, ConnectionHistoryRepository,
    ConnectionHistoryResult, ConnectionHistoryWithHost, ConnectionStats, ConnectionStatus,
    SqliteConnectionHistoryRepository,
};
pub use host_repository::{HostRepository, SqliteHostRepository};
