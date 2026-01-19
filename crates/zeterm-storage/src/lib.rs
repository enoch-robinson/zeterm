//! Zeterm 数据持久化层
//!
//! 提供数据库连接管理、主机配置存储、连接历史记录等功能。
//!
//! # 模块结构
//!
//! - `database` - 数据库连接管理
//! - `repository` - 数据仓库接口和实现
//! - `secret` - 敏感信息存储（待实现）
//!
//! # 示例
//!
//! ```no_run
//! use zeterm_storage::database::Database;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 使用默认路径创建数据库连接
//! let db = Database::with_default_path().await?;
//!
//! // 初始化数据库（运行迁移）
//! db.init().await?;
//!
//! // 健康检查
//! db.health_check().await?;
//! # Ok(())
//! # }
//! ```

pub mod database;
pub mod repository;

// 导出常用类型
pub use database::Database;
pub use repository::{HostRepository, SqliteHostRepository};

// 预留模块声明（待实现）
// pub mod secret;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // 验证模块导出正常
        let _ = Database::default_db_path();
    }
}
