//! 会话协调器
//!
//! 协调终端状态机和连接管理器，实现数据流转。

use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::ConnectionManager;
use crate::app::terminal::{TerminalConfig, TerminalEvent, TerminalState};
use zeterm_core::{ConnectionState, TerminalConnection, TerminalSize};

/// 会话协调器
///
/// 负责协调终端状态机和后端连接，实现数据的双向流转。
pub struct SessionCoordinator {
    /// 终端状态机
    terminal: Arc<TerminalState>,
    /// 连接管理器
    connection_manager: Arc<ConnectionManager>,
    /// 终端事件接收器
    event_rx: RwLock<Option<mpsc::UnboundedReceiver<TerminalEvent>>>,
}

impl SessionCoordinator {
    /// 创建新的会话协调器
    pub fn new(config: TerminalConfig) -> Self {
        let (terminal, event_rx) = TerminalState::new(config);

        Self {
            terminal: Arc::new(terminal),
            connection_manager: Arc::new(ConnectionManager::new()),
            event_rx: RwLock::new(Some(event_rx)),
        }
    }

    /// 使用默认配置创建会话协调器
    pub fn with_defaults() -> Self {
        Self::new(TerminalConfig::default())
    }

    /// 获取终端状态机引用
    pub fn terminal(&self) -> Arc<TerminalState> {
        Arc::clone(&self.terminal)
    }

    /// 获取连接管理器引用
    pub fn connection_manager(&self) -> Arc<ConnectionManager> {
        Arc::clone(&self.connection_manager)
    }

    /// 设置后端连接
    pub fn set_connection(&self, conn: Box<dyn TerminalConnection>) {
        self.connection_manager.set_connection(conn);
        info!("Session connection established");
    }

    /// 获取连接状态
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_manager.state()
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.connection_manager.is_connected()
    }

    /// 获取终端尺寸
    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal.size()
    }

    /// 调整终端大小
    pub fn resize(&self, rows: u16, cols: u16) {
        self.terminal.resize(rows, cols);
    }

    /// 处理输入数据
    pub fn advance_bytes(&self, bytes: &[u8]) {
        self.terminal.advance_bytes(bytes);
    }

    /// 关闭会话
    pub async fn close(&self) {
        self.connection_manager.close().await;
        info!("Session closed");
    }
}
