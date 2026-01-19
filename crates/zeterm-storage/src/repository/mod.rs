//! 数据仓库模块
//!
//! 提供数据访问接口和实现。

mod host_repository;

pub use host_repository::{HostRepository, SqliteHostRepository};

// 预留其他仓库模块
// mod connection_history_repository;
