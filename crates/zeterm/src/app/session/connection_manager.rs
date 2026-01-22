//! 连接管理器
//!
//! 管理终端连接的生命周期，包括连接建立、数据传输和断开。

use std::time::Instant;

use futures::stream::BoxStream;
use tokio::sync::RwLock;
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
    /// 后端连接
    connection: RwLock<Option<Box<dyn TerminalConnection>>>,
    /// 连接状态
    state: RwLock<ConnectionState>,
    /// 是否已取消
    cancelled: RwLock<bool>,
}

impl ConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            connection: RwLock::new(None),
            state: RwLock::new(ConnectionState::Idle),
            cancelled: RwLock::new(false),
        }
    }

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
        // 获取数据接收流
        let stream = conn.receive_stream();

        let mut connection = self.connection.write().await;
        *connection = Some(conn);

        *self.state.write().await = ConnectionState::Connected {
            connected_at: Instant::now(),
        };
        info!("Connection established");

        stream
    }

    /// 获取连接状态
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        self.state().await.is_active()
    }

    /// 检查是否已取消
    pub async fn is_cancelled(&self) -> bool {
        *self.cancelled.read().await
    }

    /// 取消操作
    pub async fn cancel(&self) {
        *self.cancelled.write().await = true;
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
        self.cancel().await;
        let conn = self.connection.write().await.take();
        if let Some(conn) = conn {
            if let Err(e) = conn.close().await {
                warn!("Error closing connection: {}", e);
            }
        }

        *self.state.write().await = ConnectionState::Disconnected {
            reason: DisconnectReason::UserInitiated,
        };
        info!("Connection closed");
    }

    /// 标记连接已断开（由数据流结束触发）
    ///
    /// 当receive_stream 返回的流结束时调用此方法，
    /// 用于更新连接状态，通知上层连接已断开。///
    /// # Arguments
    ///
    /// * `reason` - 断开原因
    pub async fn mark_disconnected(&self, reason: DisconnectReason) {
        let current_state = self.state.read().await.clone();

        // 只有在已连接状态才更新为断开
        if current_state.is_active() {
            *self.state.write().await = ConnectionState::Disconnected { reason };
            info!("Connection marked as disconnected");
        }
    }
    /// 标记连接已断开（服务器关闭）
    ///
    /// 简化版本，使用 ServerClosed 作为断开原因。
    pub async fn mark_disconnected_by_server(&self) {
        self.mark_disconnected(DisconnectReason::ServerClosed).await;
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// tokio::sync::RwLock 自动实现 Send + Sync，无需手动 unsafe impl

#[cfg(test)]
mod tests {
    use super::*;
    use zeterm_core::ConnectionState;

    #[tokio::test]
    async fn test_connection_manager_new() {
        let manager = ConnectionManager::new();
        assert!(!manager.is_connected().await);
        assert!(!manager.is_cancelled().await);
        assert!(matches!(manager.state().await, ConnectionState::Idle));
    }

    #[tokio::test]
    async fn test_connection_manager_default() {
        let manager = ConnectionManager::default();
        assert!(!manager.is_connected().await);
    }

    #[tokio::test]
    async fn test_connection_manager_cancel() {
        let manager = ConnectionManager::new();
        assert!(!manager.is_cancelled().await);

        manager.cancel().await;
        assert!(manager.is_cancelled().await);
    }

    #[tokio::test]
    async fn test_connection_manager_state_idle() {
        let manager = ConnectionManager::new();
        let state = manager.state().await;
        assert!(matches!(state, ConnectionState::Idle));
        assert!(!state.is_active());
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
