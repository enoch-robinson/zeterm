//! 连接历史仓库
//!
//! 提供连接历史的存储和查询功能。
//!
//! # 功能
//!
//! - 记录连接事件（开始、结束、失败）
//! - 查询最近连接记录
//! - 连接统计信息
//! - 清理旧记录
//!
//! # 示例
//!
//! ```ignore
//! use zeterm_storage::{Database, ConnectionHistoryRepository, SqliteConnectionHistoryRepository};
//!
//! let db = Database::with_default_path().await?;
//! let repo = SqliteConnectionHistoryRepository::new(db.pool().clone());
//!
//! // 记录连接开始
//! let session_id = repo.record_connection_start(host_id).await?;
//!
//! // 记录连接结束
//! repo.record_connection_end(&session_id, ConnectionStatus::Success).await?;
//!
//! // 查询最近连接
//! let recent = repo.get_recent(10).await?;
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// 错误类型
// ============================================================================

/// 连接历史仓库错误
#[derive(Debug, Error)]
pub enum ConnectionHistoryError {
    /// 数据库错误
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// 记录未找到
    #[error("Connection history record not found: {0}")]
    NotFound(String),

    /// 无效的状态
    #[error("Invalid connection status: {0}")]
    InvalidStatus(String),

    /// 其他错误
    #[error("Connection history error: {0}")]
    Other(String),
}

/// 连接历史仓库结果类型
pub type ConnectionHistoryResult<T> = Result<T, ConnectionHistoryError>;

// ============================================================================
// 数据模型
// ============================================================================

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionStatus {
    /// 连接成功
    Success,
    /// 连接失败
    Failed,
    /// 已断开
    Disconnected,
    /// 连接中（尚未结束）
    Connecting,
}

impl ConnectionStatus {
    /// 转换为数据库字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            ConnectionStatus::Success => "success",
            ConnectionStatus::Failed => "failed",
            ConnectionStatus::Disconnected => "disconnected",
            ConnectionStatus::Connecting => "connecting",
        }
    }

    /// 从数据库字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(ConnectionStatus::Success),
            "failed" => Some(ConnectionStatus::Failed),
            "disconnected" => Some(ConnectionStatus::Disconnected),
            "connecting" => Some(ConnectionStatus::Connecting),
            _ => None,
        }
    }

    /// 是否为终止状态
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ConnectionStatus::Success | ConnectionStatus::Failed | ConnectionStatus::Disconnected
        )
    }
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 连接历史记录
#[derive(Debug, Clone)]
pub struct ConnectionHistoryRecord {
    /// 记录 ID
    pub id: i64,
    /// 主机 ID
    pub host_id: i64,
    /// 连接时间（Unix 时间戳）
    pub connected_at: i64,
    /// 断开时间（Unix 时间戳），None 表示仍在连接
    pub disconnected_at: Option<i64>,
    /// 连接时长（秒），None 表示仍在连接
    pub duration: Option<i64>,
    /// 连接状态
    pub status: ConnectionStatus,
    /// 错误消息
    pub error_message: Option<String>,
    /// 会话 ID
    pub session_id: Option<String>,
}

impl ConnectionHistoryRecord {
    /// 是否仍在连接中
    pub fn is_active(&self) -> bool {
        self.disconnected_at.is_none() && self.status == ConnectionStatus::Connecting
    }

    /// 获取格式化的连接时间
    pub fn formatted_connected_at(&self) -> String {
        format_timestamp(self.connected_at)
    }

    /// 获取格式化的断开时间
    pub fn formatted_disconnected_at(&self) -> Option<String> {
        self.disconnected_at.map(format_timestamp)
    }

    /// 获取格式化的连接时长
    pub fn formatted_duration(&self) -> String {
        match self.duration {
            Some(secs) => format_duration(secs),
            None => "-".to_string(),
        }
    }
}

/// 带主机信息的连接历史记录（用于联表查询）
#[derive(Debug, Clone)]
pub struct ConnectionHistoryWithHost {
    /// 基础连接历史记录
    pub record: ConnectionHistoryRecord,
    /// 主机名称
    pub host_name: String,
    /// 主机地址
    pub host_address: String,
    /// 用户名
    pub username: String,
}

/// 连接统计信息
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// 总连接次数
    pub total_connections: i64,
    /// 成功连接次数
    pub successful_connections: i64,
    /// 失败连接次数
    pub failed_connections: i64,
    /// 平均连接时长（秒）
    pub average_duration: Option<f64>,
    /// 最长连接时长（秒）
    pub max_duration: Option<i64>,
    /// 最近连接时间
    pub last_connection_at: Option<i64>,
}

impl ConnectionStats {
    /// 获取成功率（百分比）
    pub fn success_rate(&self) -> f64 {
        if self.total_connections == 0 {
            0.0
        } else {
            (self.successful_connections as f64 / self.total_connections as f64) * 100.0
        }
    }

    /// 获取格式化的平均连接时长
    pub fn formatted_average_duration(&self) -> String {
        self.average_duration
            .map(|d| format_duration(d as i64))
            .unwrap_or_else(|| "-".to_string())
    }
}

// ============================================================================
// 仓库 Trait
// ============================================================================

/// 连接历史仓库接口
#[async_trait]
pub trait ConnectionHistoryRepository: Send + Sync {
    /// 记录连接开始
    ///
    /// 返回会话 ID，用于后续更新
    async fn record_connection_start(&self, host_id: i64) -> ConnectionHistoryResult<String>;

    /// 记录连接结束
    async fn record_connection_end(
        &self,
        session_id: &str,
        status: ConnectionStatus,
        error_message: Option<String>,
    ) -> ConnectionHistoryResult<()>;

    /// 根据 ID 获取记录
    async fn get_by_id(&self, id: i64) -> ConnectionHistoryResult<ConnectionHistoryRecord>;

    /// 根据会话 ID 获取记录
    async fn get_by_session_id(
        &self,
        session_id: &str,
    ) -> ConnectionHistoryResult<ConnectionHistoryRecord>;

    /// 获取指定主机的连接历史
    async fn get_by_host_id(
        &self,
        host_id: i64,
        limit: i64,
    ) -> ConnectionHistoryResult<Vec<ConnectionHistoryRecord>>;

    /// 获取最近的连接记录
    async fn get_recent(
        &self,
        limit: i64,
    ) -> ConnectionHistoryResult<Vec<ConnectionHistoryWithHost>>;

    /// 获取指定主机的连接统计
    async fn get_stats_by_host_id(&self, host_id: i64) -> ConnectionHistoryResult<ConnectionStats>;

    /// 获取全局连接统计
    async fn get_global_stats(&self) -> ConnectionHistoryResult<ConnectionStats>;

    /// 获取活跃连接（未断开的连接）
    async fn get_active_connections(&self)
    -> ConnectionHistoryResult<Vec<ConnectionHistoryRecord>>;

    /// 标记所有活跃连接为断开（用于应用启动时清理）
    async fn mark_all_active_as_disconnected(&self) -> ConnectionHistoryResult<i64>;

    /// 删除指定主机的所有连接历史
    async fn delete_by_host_id(&self, host_id: i64) -> ConnectionHistoryResult<i64>;

    /// 删除指定时间之前的记录（清理旧数据）
    async fn delete_before(&self, before_timestamp: i64) -> ConnectionHistoryResult<i64>;

    /// 获取总记录数
    async fn count(&self) -> ConnectionHistoryResult<i64>;
}

// ============================================================================
// SQLite 实现
// ============================================================================

/// SQLite 连接历史仓库实现
pub struct SqliteConnectionHistoryRepository {
    pool: SqlitePool,
}

impl SqliteConnectionHistoryRepository {
    /// 创建新的 SQLite 连接历史仓库
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 获取当前 Unix 时间戳
    fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// 生成会话 ID
    fn generate_session_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[async_trait]
impl ConnectionHistoryRepository for SqliteConnectionHistoryRepository {
    async fn record_connection_start(&self, host_id: i64) -> ConnectionHistoryResult<String> {
        let session_id = Self::generate_session_id();
        let connected_at = Self::current_timestamp();

        sqlx::query(
            r#"
            INSERT INTO connection_history (host_id, connected_at, status, session_id)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(host_id)
        .bind(connected_at)
        .bind(ConnectionStatus::Connecting.as_str())
        .bind(&session_id)
        .execute(&self.pool)
        .await?;

        Ok(session_id)
    }

    async fn record_connection_end(
        &self,
        session_id: &str,
        status: ConnectionStatus,
        error_message: Option<String>,
    ) -> ConnectionHistoryResult<()> {
        let disconnected_at = Self::current_timestamp();

        // 先获取连接开始时间以计算时长
        let record = self.get_by_session_id(session_id).await?;
        let duration = disconnected_at - record.connected_at;

        sqlx::query(
            r#"
            UPDATE connection_history
            SET disconnected_at = ?, duration = ?, status = ?, error_message = ?
            WHERE session_id = ?
            "#,
        )
        .bind(disconnected_at)
        .bind(duration)
        .bind(status.as_str())
        .bind(error_message)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_by_id(&self, id: i64) -> ConnectionHistoryResult<ConnectionHistoryRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, host_id, connected_at, disconnected_at, duration, status, error_message, session_id
            FROM connection_history
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ConnectionHistoryError::NotFound(format!("id={}", id)))?;

        Ok(row_to_record(&row))
    }

    async fn get_by_session_id(
        &self,
        session_id: &str,
    ) -> ConnectionHistoryResult<ConnectionHistoryRecord> {
        let row = sqlx::query(
            r#"
            SELECT id, host_id, connected_at, disconnected_at, duration, status, error_message, session_id
            FROM connection_history
            WHERE session_id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ConnectionHistoryError::NotFound(format!("session_id={}", session_id)))?;

        Ok(row_to_record(&row))
    }

    async fn get_by_host_id(
        &self,
        host_id: i64,
        limit: i64,
    ) -> ConnectionHistoryResult<Vec<ConnectionHistoryRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, host_id, connected_at, disconnected_at, duration, status, error_message, session_id
            FROM connection_history
            WHERE host_id = ?
            ORDER BY connected_at DESC
            LIMIT ?
            "#,
        )
        .bind(host_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_record).collect())
    }

    async fn get_recent(
        &self,
        limit: i64,
    ) -> ConnectionHistoryResult<Vec<ConnectionHistoryWithHost>> {
        let rows = sqlx::query(
            r#"
            SELECT
                ch.id, ch.host_id, ch.connected_at, ch.disconnected_at,
                ch.duration, ch.status, ch.error_message, ch.session_id,
                h.name as host_name, h.host as host_address, h.username
            FROM connection_history ch
            INNER JOIN hosts h ON ch.host_id = h.id
            ORDER BY ch.connected_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_record_with_host).collect())
    }

    async fn get_stats_by_host_id(&self, host_id: i64) -> ConnectionHistoryResult<ConnectionStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_connections,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as successful_connections,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_connections,
                AVG(duration) as average_duration,
                MAX(duration) as max_duration,
                MAX(connected_at) as last_connection_at
            FROM connection_history
            WHERE host_id = ?
            "#,
        )
        .bind(host_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_stats(&row))
    }

    async fn get_global_stats(&self) -> ConnectionHistoryResult<ConnectionStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_connections,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as successful_connections,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_connections,
                AVG(duration) as average_duration,
                MAX(duration) as max_duration,
                MAX(connected_at) as last_connection_at
            FROM connection_history
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row_to_stats(&row))
    }

    async fn get_active_connections(
        &self,
    ) -> ConnectionHistoryResult<Vec<ConnectionHistoryRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, host_id, connected_at, disconnected_at, duration, status, error_message, session_id
            FROM connection_history
            WHERE disconnected_at IS NULL AND status = 'connecting'
            ORDER BY connected_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(row_to_record).collect())
    }

    async fn mark_all_active_as_disconnected(&self) -> ConnectionHistoryResult<i64> {
        let now = Self::current_timestamp();

        let result = sqlx::query(
            r#"
            UPDATE connection_history
            SET
                disconnected_at = ?,
                duration = ? - connected_at,
                status = 'disconnected',
                error_message = 'Application terminated unexpectedly'
            WHERE disconnected_at IS NULL AND status = 'connecting'
            "#,
        )
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    async fn delete_by_host_id(&self, host_id: i64) -> ConnectionHistoryResult<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM connection_history WHERE host_id = ?
            "#,
        )
        .bind(host_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    async fn delete_before(&self, before_timestamp: i64) -> ConnectionHistoryResult<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM connection_history WHERE connected_at < ?
            "#,
        )
        .bind(before_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() as i64)
    }

    async fn count(&self) -> ConnectionHistoryResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM connection_history
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 从数据库行转换为连接历史记录
fn row_to_record(row: &sqlx::sqlite::SqliteRow) -> ConnectionHistoryRecord {
    let status_str: String = row.get("status");
    let status = ConnectionStatus::from_str(&status_str).unwrap_or(ConnectionStatus::Disconnected);

    ConnectionHistoryRecord {
        id: row.get("id"),
        host_id: row.get("host_id"),
        connected_at: row.get("connected_at"),
        disconnected_at: row.get("disconnected_at"),
        duration: row.get("duration"),
        status,
        error_message: row.get("error_message"),
        session_id: row.get("session_id"),
    }
}

/// 从数据库行转换为带主机信息的连接历史记录
fn row_to_record_with_host(row: &sqlx::sqlite::SqliteRow) -> ConnectionHistoryWithHost {
    ConnectionHistoryWithHost {
        record: row_to_record(row),
        host_name: row.get("host_name"),
        host_address: row.get("host_address"),
        username: row.get("username"),
    }
}

/// 从数据库行转换为连接统计
fn row_to_stats(row: &sqlx::sqlite::SqliteRow) -> ConnectionStats {
    ConnectionStats {
        total_connections: row.get("total_connections"),
        successful_connections: row.get("successful_connections"),
        failed_connections: row.get("failed_connections"),
        average_duration: row.get("average_duration"),
        max_duration: row.get("max_duration"),
        last_connection_at: row.get("last_connection_at"),
    }
}

/// 格式化 Unix 时间戳
fn format_timestamp(timestamp: i64) -> String {
    // 简单的时间格式化
    let secs = timestamp as u64;
    let days = secs / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;

    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hour, minute, second
    )
}

/// 格式化时长（秒）
fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return "-".to_string();
    }

    let secs = secs as u64;

    if secs >= 86400 {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        format!("{}d {}h", days, hours)
    } else if secs >= 3600 {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else if secs >= 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", secs)
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_status_as_str() {
        assert_eq!(ConnectionStatus::Success.as_str(), "success");
        assert_eq!(ConnectionStatus::Failed.as_str(), "failed");
        assert_eq!(ConnectionStatus::Disconnected.as_str(), "disconnected");
        assert_eq!(ConnectionStatus::Connecting.as_str(), "connecting");
    }

    #[test]
    fn test_connection_status_from_str() {
        assert_eq!(
            ConnectionStatus::from_str("success"),
            Some(ConnectionStatus::Success)
        );
        assert_eq!(
            ConnectionStatus::from_str("failed"),
            Some(ConnectionStatus::Failed)
        );
        assert_eq!(
            ConnectionStatus::from_str("disconnected"),
            Some(ConnectionStatus::Disconnected)
        );
        assert_eq!(
            ConnectionStatus::from_str("connecting"),
            Some(ConnectionStatus::Connecting)
        );
        assert_eq!(ConnectionStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_connection_status_is_terminal() {
        assert!(ConnectionStatus::Success.is_terminal());
        assert!(ConnectionStatus::Failed.is_terminal());
        assert!(ConnectionStatus::Disconnected.is_terminal());
        assert!(!ConnectionStatus::Connecting.is_terminal());
    }

    #[test]
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Success.to_string(), "success");
        assert_eq!(ConnectionStatus::Failed.to_string(), "failed");
    }

    #[test]
    fn test_connection_stats_success_rate() {
        let stats = ConnectionStats {
            total_connections: 10,
            successful_connections: 8,
            failed_connections: 2,
            ..Default::default()
        };
        assert_eq!(stats.success_rate(), 80.0);

        let empty_stats = ConnectionStats::default();
        assert_eq!(empty_stats.success_rate(), 0.0);
    }

    #[test]
    fn test_connection_stats_formatted_average_duration() {
        let stats = ConnectionStats {
            average_duration: Some(3725.0), // 1h 2m 5s
            ..Default::default()
        };
        assert_eq!(stats.formatted_average_duration(), "1h 2m");

        let no_duration = ConnectionStats::default();
        assert_eq!(no_duration.formatted_average_duration(), "-");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3661), "1h 1m");
        assert_eq!(format_duration(90061), "1d 1h");
        assert_eq!(format_duration(-1), "-");
    }

    #[test]
    fn test_format_timestamp() {
        // 2024-01-01 00:00:00 UTC (approximate)
        let ts = 1704067200;
        let formatted = format_timestamp(ts);
        assert!(formatted.contains("2024"));
    }

    #[test]
    fn test_connection_history_record_is_active() {
        let active = ConnectionHistoryRecord {
            id: 1,
            host_id: 1,
            connected_at: 1704067200,
            disconnected_at: None,
            duration: None,
            status: ConnectionStatus::Connecting,
            error_message: None,
            session_id: Some("test-session".to_string()),
        };
        assert!(active.is_active());

        let finished = ConnectionHistoryRecord {
            id: 2,
            host_id: 1,
            connected_at: 1704067200,
            disconnected_at: Some(1704070800),
            duration: Some(3600),
            status: ConnectionStatus::Success,
            error_message: None,
            session_id: Some("test-session-2".to_string()),
        };
        assert!(!finished.is_active());
    }

    #[test]
    fn test_connection_history_record_formatted_duration() {
        let record = ConnectionHistoryRecord {
            id: 1,
            host_id: 1,
            connected_at: 0,
            disconnected_at: Some(3600),
            duration: Some(3600),
            status: ConnectionStatus::Success,
            error_message: None,
            session_id: None,
        };
        assert_eq!(record.formatted_duration(), "1h 0m");

        let active = ConnectionHistoryRecord {
            id: 2,
            host_id: 1,
            connected_at: 0,
            disconnected_at: None,
            duration: None,
            status: ConnectionStatus::Connecting,
            error_message: None,
            session_id: None,
        };
        assert_eq!(active.formatted_duration(), "-");
    }

    #[test]
    fn test_generate_session_id() {
        let id1 = SqliteConnectionHistoryRepository::generate_session_id();
        let id2 = SqliteConnectionHistoryRepository::generate_session_id();

        // Session IDs should be unique
        assert_ne!(id1, id2);

        // Should be valid UUIDs
        assert!(Uuid::parse_str(&id1).is_ok());
        assert!(Uuid::parse_str(&id2).is_ok());
    }

    #[test]
    fn test_current_timestamp() {
        let ts1 = SqliteConnectionHistoryRepository::current_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = SqliteConnectionHistoryRepository::current_timestamp();

        // Timestamp should be positive and increasing
        assert!(ts1 > 0);
        assert!(ts2 >= ts1);
    }
}
