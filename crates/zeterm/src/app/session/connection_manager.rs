//! 连接管理器
//!
//! 管理终端连接的生命周期，包括连接建立、数据传输和断开。

use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use zeterm_core::{ConnectionState, DisconnectReason, TerminalConnection};

/// 连接管理器
///
/// 负责管理单个终端连接的生命周期。
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

    /// 设置连接
    pub fn set_connection(&self, conn: Box<dyn TerminalConnection>) {
        let mut connection = self.connection.write();
        *connection = Some(conn);
        *self.state.write() = ConnectionState::Connected {
            connected_at: Instant::now(),
        };
        info!("Connection established");
    }

    /// 获取连接状态
    pub fn state(&self) -> ConnectionState {
        self.state.read().clone()
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.state().is_active()
    }

    /// 检查是否已取消
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.read()
    }

    /// 取消操作
    pub fn cancel(&self) {
        *self.cancelled.write() = true;
    }

    /// 关闭连接
    pub async fn close(&self) {
        self.cancel();
        let conn = self.connection.write().take();
        if let Some(conn) = conn {
            if let Err(e) = conn.close().await {
                warn!("Error closing connection: {}", e);
            }
        }

        *self.state.write() = ConnectionState::Disconnected {
            reason: DisconnectReason::UserInitiated,
        };
        info!("Connection closed");
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

