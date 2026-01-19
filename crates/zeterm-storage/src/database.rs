//! 数据库连接管理模块
//!
//! 提供 SQLite 数据库的连接、初始化和迁移功能。

use anyhow::{Context, Result};
use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, info, warn};

/// 数据库连接管理器
///
/// 封装 SQLite 数据库连接池，提供初始化和迁移功能。
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    db_path: PathBuf,
}

impl Database {
    /// 创建新的数据库连接
    ///
    /// #参数
    ///
    /// * `db_path` - 数据库文件路径
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use zeterm_storage::database::Database;
    /// use std::path::PathBuf;///
    /// # async fn example() -> anyhow::Result<()> {
    /// let db = Database::new(PathBuf::from("zeterm.db")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(db_path: PathBuf) -> Result<Self> {
        info!("初始化数据库连接: {:?}", db_path);

        // 确保数据库目录存在
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).context("创建数据库目录失败")?;
                debug!("创建数据库目录: {:?}", parent);
            }
        }

        // 配置 SQLite 连接选项
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal) // 使用 WAL 模式提高并发性能
            .busy_timeout(std::time::Duration::from_secs(5))
            .disable_statement_logging(); // 禁用语句日志以提高性能

        // 创建连接池
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("创建数据库连接池失败")?;

        info!("数据库连接池创建成功");

        Ok(Self { pool, db_path })
    }

    /// 使用默认路径创建数据库连接
    ///
    /// 默认路径：
    /// - Linux: `~/.config/zeterm/zeterm.db`
    /// - macOS: `~/Library/Application Support/zeterm/zeterm.db`
    /// - Windows: `%APPDATA%\zeterm\zeterm.db`
    pub async fn with_default_path() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        Self::new(db_path).await
    }

    /// 获取默认数据库文件路径
    pub fn default_db_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("无法获取配置目录")?;
        let zeterm_dir = config_dir.join("zeterm");
        Ok(zeterm_dir.join("zeterm.db"))
    }

    /// 初始化数据库（运行迁移）
    ///
    /// 执行所有待执行的数据库迁移脚本。
    pub async fn init(&self) -> Result<()> {
        info!("开始数据库迁移");

        // 运行嵌入的迁移脚本
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("数据库迁移失败")?;

        info!("数据库迁移完成");
        Ok(())
    }

    /// 获取数据库连接池的引用
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// 获取数据库文件路径
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// 关闭数据库连接池
    pub async fn close(&self) {
        info!("关闭数据库连接池");
        self.pool.close().await;
    }

    /// 检查数据库连接是否正常
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .context("数据库健康检查失败")?;
        debug!("数据库健康检查通过");
        Ok(())
    }

    /// 获取数据库版本信息
    pub async fn version(&self) -> Result<String> {
        let row: (String,) = sqlx::query_as("SELECT sqlite_version()")
            .fetch_one(&self.pool)
            .await
            .context("获取数据库版本失败")?;

        Ok(row.0)
    }

    /// 清空所有表（仅用于测试）
    #[cfg(test)]
    pub async fn clear_all_tables(&self) -> Result<()> {
        warn!("清空所有数据库表（测试模式）");

        sqlx::query("DELETE FROM connection_history")
            .execute(&self.pool)
            .await?;

        sqlx::query("DELETE FROM hosts").execute(&self.pool).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = Database::new(db_path.clone()).await.unwrap();
        assert!(db_path.exists());
    }

    #[tokio::test]
    async fn test_database_init() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::new(db_path).await.unwrap();
        db.init().await.unwrap();

        // 验证表是否创建成功
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='hosts'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(result.0, 1);
    }

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::new(db_path).await.unwrap();
        db.health_check().await.unwrap();
    }

    #[tokio::test]
    async fn test_version() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::new(db_path).await.unwrap();
        let version = db.version().await.unwrap();

        assert!(!version.is_empty());
        println!("SQLite version: {}", version);
    }
}
