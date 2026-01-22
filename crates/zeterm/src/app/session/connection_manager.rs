//! 连接管理器
//!
//! 管理终端连接的生命周期，包括连接建立、数据传输和断开。
//!
//! # 锁设计
//!
//! 采用混合锁设计，根据数据访问模式选择合适的锁类型：
//!
//! | 数据 | 访问模式 | 锁类型 |
//! |------|----------|--------|
//! | `connection` | I/O 操作，需要 await | `tokio::sync::RwLock` |
//! | `state` | 频繁读取，纯内存操作 | `parking_lot::RwLock` |
//! | `cancelled` | 简单布尔标志 | `AtomicBool` |

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use futures::stream::BoxStream;
use parking_lot::RwLock as SyncRwLock;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};

use zeterm_core::{ConnectionError, ConnectionState, DisconnectReason, TerminalConnection};

/// 连接管理器
///
/// 负责管理单个终端连接的生命周期。
///
/// # 职责
///
/// - 持有后端连接实例
/// - 管理连接状态
/// - 提供数据发送接口
/// - 处理连接关闭
///
/// # 注意
///
/// 数据接收流(receive_stream) 不存储在此结构中，
/// 而是在 `set_connection` 时直接返回，由调用者负责处理。
/// 这样可以避免 `Send + Sync` 的问题。
pub struct ConnectionManager {
    /// 后端连接 - 需要异步 I/O，使用 tokio 锁
    connection: AsyncRwLock<Option<Box<dyn TerminalConnection>>>,
    /// 连接状态 - 纯内存读写，使用同步锁
    state: SyncRwLock<ConnectionState>,
    /// 是否已取消 - 简单布尔标志，使用原子操作
    cancelled: AtomicBool,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            connection: AsyncRwLock::new(None),
            state: SyncRwLock::new(ConnectionState::Idle),
            cancelled: AtomicBool::new(false),
        }
    }

    // ========================================================================
    // 同步方法 - 状态查询（无需 async）
    // ========================================================================

    /// 获取连接状态
    ///
    /// 这是一个同步方法，可以在任何上下文中调用。
    pub fn state(&self) -> ConnectionState {
        self.state.read().clone()
    }

    /// 检查是否已连接
    ///
    /// 这是一个同步方法，可以在任何上下文中调用。
    pub fn is_connected(&self) -> bool {
        self.state().is_active()
    }

    /// 检查是否已取消
    ///
    /// 使用原子操作，无锁且高效。
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// 取消操作
    ///
    /// 使用原子操作，无锁且高效。
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// 重置取消标志
    ///
    /// 用于在新连接建立前重置状态。
    pub fn reset_cancelled(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    /// 标记连接已断开
    ///
    /// 这是一个同步方法，用于更新连接状态。
    ///
    /// # Arguments
    ///
    /// * `reason` - 断开原因
    pub fn mark_disconnected(&self, reason: DisconnectReason) {
        let mut state = self.state.write();

        // 只有在已连接状态才更新为断开
        if state.is_active() {
            *state = ConnectionState::Disconnected { reason };
            info!("Connection marked as disconnected");
        }
    }

    /// 标记连接已断开（服务器关闭）
    ///
    /// 简化版本，使用 ServerClosed 作为断开原因。
    pub fn mark_disconnected_by_server(&self) {
        self.mark_disconnected(DisconnectReason::ServerClosed);
    }

    // ========================================================================
    // 异步方法 - I/O 操作（需要 async）
    // ========================================================================

    /// 设置连接并返回数据接收流
    ///
    /// 设置后端连接，同时返回数据接收流供调用者使用。
    ///
    /// # Arguments
    ///
    /// * `conn` - 后端连接实例
    ///
    /// # Returns
    ///
    /// 返回数据接收流，调用者需要负责处理这个流。
    pub async fn set_connection(
        &self,
        conn: Box<dyn TerminalConnection>,
    ) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>> {
        // 重置取消标志
        self.reset_cancelled();

        // 获取数据接收流
        let stream = conn.receive_stream();

        // 存储连接
        {
            let mut connection = self.connection.write().await;
            *connection = Some(conn);
        }

        // 更新状态（同步操作）
        {
            let mut state = self.state.write();
            *state = ConnectionState::Connected {
                connected_at: Instant::now(),
            };
        }

        info!("Connection established");
        stream
    }

    /// 发送数据到后端连接
    ///
    /// # Arguments
    ///
    /// * `data` - 要发送的字节数据
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 发送成功
    /// * `Err(ConnectionError)` - 发送失败
    pub async fn write(&self, data: &[u8]) -> Result<(), ConnectionError> {
        let conn = self.connection.read().await;
        match conn.as_ref() {
            Some(conn) => {
                debug!("Writing {} bytes to connection", data.len());
                conn.write(data).await
            },
            None => {
                warn!("Attempted to write to disconnected connection");
                Err(ConnectionError::Disconnected)
            },
        }
    }

    /// 调整远端终端大小
    ///
    /// # Arguments
    ///
    /// * `rows` - 新的行数
    /// * `cols` - 新的列数
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 调整成功
    /// * `Err(ConnectionError)` - 调整失败
    pub async fn resize(&self, rows: u16, cols: u16) -> Result<(), ConnectionError> {
        let conn = self.connection.read().await;
        match conn.as_ref() {
            Some(conn) => {
                debug!("Resizing connection to {}x{}", cols, rows);
                conn.resize(rows, cols).await
            },
            None => {
                warn!("Attempted to resize disconnected connection");
                Err(ConnectionError::Disconnected)
            },
        }
    }

    /// 关闭连接
    pub async fn close(&self) {
        // 先标记取消（同步操作）
        self.cancel();

        // 关闭连接（异步 I/O）
        let conn = self.connection.write().await.take();
        if let Some(conn) = conn {
            if let Err(e) = conn.close().await {
                warn!("Error closing connection: {}", e);
            }
        }

        // 更新状态（同步操作）
        {
            let mut state = self.state.write();
            *state = ConnectionState::Disconnected {
                reason: DisconnectReason::UserInitiated,
            };
        }

        info!("Connection closed");
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// parking_lot::RwLock 和 AtomicBool 都是 Send + Sync
// tokio::sync::RwLock<T> 在 T: Send 时也是 Send + Sync
// 因此 ConnectionManager 自动实现 Send + Sync，无需手动 unsafe impl

#[cfg(test)]
mod tests {
    use super::*;
    use zeterm_core::ConnectionState;

    #[test]
    fn test_connection_manager_new() {
        let manager = ConnectionManager::new();
        assert!(!manager.is_connected());
        assert!(!manager.is_cancelled());
        assert!(matches!(manager.state(), ConnectionState::Idle));
    }

    #[test]
    fn test_connection_manager_default() {
        let manager = ConnectionManager::default();
        assert!(!manager.is_connected());
    }

    #[test]
    fn test_connection_manager_cancel() {
        let manager = ConnectionManager::new();
        assert!(!manager.is_cancelled());

        manager.cancel();
        assert!(manager.is_cancelled());

        manager.reset_cancelled();
        assert!(!manager.is_cancelled());
    }

    #[test]
    fn test_connection_manager_state_idle() {
        let manager = ConnectionManager::new();
        let state = manager.state();
        assert!(matches!(state, ConnectionState::Idle));
        assert!(!state.is_active());
    }

    #[test]
    fn test_connection_manager_mark_disconnected() {
        let manager = ConnectionManager::new();

        // Idle 状态下调用 mark_disconnected 不应改变状态
        manager.mark_disconnected(DisconnectReason::ServerClosed);
        assert!(matches!(manager.state(), ConnectionState::Idle));
    }

    #[tokio::test]
    async fn test_connection_manager_write_disconnected() {
        let manager = ConnectionManager::new();

        // 未连接时写入应该返回错误
        let result = manager.write(b"test").await;
        assert!(matches!(result, Err(ConnectionError::Disconnected)));
    }

    #[tokio::test]
    async fn test_connection_manager_resize_disconnected() {
        let manager = ConnectionManager::new();

        // 未连接时调整大小应该返回错误
        let result = manager.resize(24, 80).await;
        assert!(matches!(result, Err(ConnectionError::Disconnected)));
    }
}
