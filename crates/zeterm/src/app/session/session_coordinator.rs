//! 会话协调器
//!
//! 协调终端状态机和连接管理器，实现数据流转。
//! 这是连接后端数据流与UI渲染的关键桥梁。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::Term;
use futures::StreamExt;
use futures::stream::BoxStream;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::ConnectionManager;
use crate::app::terminal::{EventProxy, TerminalConfig, TerminalEvent, TerminalState};
use zeterm_core::{ConnectionError, ConnectionState, TerminalConnection, TerminalSize};

/// 会话协调器
///
/// 负责协调终端状态机和后端连接，实现数据的双向流转。
///
/// # 职责
///
/// - 管理终端状态机 (TerminalState)
/// - 管理后端连接 (ConnectionManager)
/// - 实现数据泵 (Data Pump)：后端数据 → 终端状态机
/// - 提供用户输入接口：用户输入 → 后端连接
///
/// # 示例
///
/// ```ignore
/// let coordinator = SessionCoordinator::with_defaults();
///
/// // 设置连接
/// coordinator.set_connection(Box::new(mock_connection));
///
/// // 启动数据泵（需要在 GPUI 上下文中）
/// coordinator.start_data_pump(cx);
///
/// // 发送用户输入
/// coordinator.send_input(b"ls -la\n").await;
/// ```
pub struct SessionCoordinator {
    /// 终端状态机
    terminal: Arc<TerminalState>,
    /// 连接管理器
    connection_manager: Arc<ConnectionManager>,
    /// 终端事件接收器
    event_rx: RwLock<Option<mpsc::UnboundedReceiver<TerminalEvent>>>,
    /// 数据泵是否正在运行
    data_pump_running: RwLock<bool>,
    /// 数据泵取消标志
    data_pump_cancel: RwLock<bool>,
    /// 数据更新脏标记，用于通知 UI 需要重绘
    dirty: AtomicBool,
    /// 上次重绘时间戳（用于帧率限制）
    last_render_time: AtomicU64,
    /// 程序启动时间（用于计算相对时间戳）
    start_instant: Instant,
}

/// 最小重绘间隔（毫秒），约60fps
const MIN_RENDER_INTERVAL_MS: u64 = 16;

impl SessionCoordinator {
    /// 创建新的会话协调器
    pub fn new(config: TerminalConfig) -> Self {
        let (terminal, event_rx) = TerminalState::new(config);

        Self {
            terminal: Arc::new(terminal),
            connection_manager: Arc::new(ConnectionManager::new()),
            event_rx: RwLock::new(Some(event_rx)),
            data_pump_running: RwLock::new(false),
            data_pump_cancel: RwLock::new(false),
            dirty: AtomicBool::new(false),
            last_render_time: AtomicU64::new(0),
            start_instant: Instant::now(),
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

    /// 设置后端连接并返回数据接收流
    ///
    /// # Arguments
    ///
    /// * `conn` - 后端连接实例
    ///
    /// # Returns
    ///
    /// 返回数据接收流，调用者需要将其传递给 `start_data_pump`。
    pub async fn set_connection(
        &self,
        conn: Box<dyn TerminalConnection>,
    ) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>> {
        let stream = self.connection_manager.set_connection(conn).await;
        info!("Session connection established");
        stream
    }

    /// 获取连接状态
    pub async fn connection_state(&self) -> ConnectionState {
        self.connection_manager.state().await
    }

    /// 检查是否已连接
    pub async fn is_connected(&self) -> bool {
        self.connection_manager.is_connected().await
    }

    /// 检查是否已连接（同步版本，用于非异步上下文）
    ///
    /// 注意：此方法会阻塞当前线程，仅在无法使用异步版本时使用。
    pub fn is_connected_sync(&self) -> bool {
        // 尝试立即获取锁状态，如果无法获取则返回 false
        // 这是一个简化实现，用于兼容同步代码
        futures::executor::block_on(self.connection_manager.is_connected())
    }

    /// 获取连接状态（同步版本）
    pub fn connection_state_sync(&self) -> ConnectionState {
        futures::executor::block_on(self.connection_manager.state())
    }

    /// 获取终端尺寸
    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal.size()
    }

    //========== Data Pump 实现 (2.6.3) ==========

    /// 启动数据泵
    ///
    /// 数据泵负责从后端连接接收数据，并送入终端状态机进行处理。
    /// 这是一个异步任务，会在后台持续运行直到连接关闭或被取消。
    ///
    /// # Arguments
    ///
    /// * `stream` - 数据接收流，从`set_connection` 返回
    /// * `notify_callback` - 当有新数据时调用的回调函数，用于触发 UI 重绘
    ///
    /// # 实现细节
    ///
    /// 1. 循环接收数据
    /// 2. 调用 terminal.advance_bytes() 处理数据
    /// 3. 调用 notify_callback 触发重绘
    /// 4. 支持优雅取消
    pub async fn start_data_pump<F>(
        &self,
        mut stream: BoxStream<'static, Result<Vec<u8>, ConnectionError>>,
        notify_callback: F,
    ) where
        F: Fn() + Send + Sync + 'static,
    {
        // 检查是否已经在运行
        {
            let running = self.data_pump_running.read();
            if *running {
                warn!("Data pump is already running");
                return;
            }
        }

        // 标记为运行中
        {
            *self.data_pump_running.write() = true;
            *self.data_pump_cancel.write() = false;
        }

        info!("Starting data pump...");

        let terminal = self.terminal.clone();

        // 数据接收循环
        loop {
            // 检查取消标志
            if *self.data_pump_cancel.read() {
                info!("Data pump cancelled");
                break;
            }

            // 使用 select! 实现超时检查
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(data)) => {
                            if data.is_empty() {
                                continue;
                            }

                            debug!("Data pump received {} bytes", data.len());

                            // 送入终端状态机
                            terminal.advance_bytes(&data);

                            // 标记有新数据需要重绘
                            self.mark_dirty();

                            // 触发 UI 重绘
                            notify_callback();
                        }
                        Some(Err(e)) => {
                            error!("Data pump error: {}", e);
                            // 根据错误类型决定是否继续
                            if !e.is_retryable() {
                                // 标记连接已断开
                                self.connection_manager.mark_disconnected(
                                    zeterm_core::DisconnectReason::NetworkError,
                                ).await;
                                break;
                            }
                        }
                        None => {
                            info!("Data stream ended");
                            // 流结束，标记连接已断开（服务器关闭）
                            self.connection_manager.mark_disconnected_by_server().await;
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // 定期检查取消标志
                    if *self.data_pump_cancel.read() {
                        info!("Data pump cancelled during wait");
                        break;
                    }
                }
            }
        }

        // 标记为停止
        {
            *self.data_pump_running.write() = false;
        }

        info!("Data pump stopped");
    }

    /// 停止数据泵
    pub fn stop_data_pump(&self) {
        info!("Stopping data pump...");
        *self.data_pump_cancel.write() = true;
    }

    /// 检查数据泵是否正在运行
    pub fn is_data_pump_running(&self) -> bool {
        *self.data_pump_running.read()
    }

    // ========== 对外接口 (2.6.4) ==========

    /// 发送用户输入到后端连接
    ///
    /// 将用户的键盘输入发送到远端终端。
    ///
    /// # Arguments
    ///
    /// * `data` - 要发送的字节数据
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 发送成功
    /// * `Err(ConnectionError)` - 发送失败
    ///
    /// # Example
    ///
    /// ```ignore
    /// // 发送命令
    /// coordinator.send_input(b"ls -la\n").await?;
    ///
    /// // 发送 Ctrl+C
    /// coordinator.send_input(&[0x03]).await?;
    /// ```
    pub async fn send_input(&self, data: &[u8]) -> Result<(), ConnectionError> {
        if !self.is_connected().await {
            return Err(ConnectionError::Disconnected);
        }

        debug!("Sending {} bytes to backend", data.len());
        self.connection_manager.write(data).await
    }

    /// 同步发送用户输入到后端连接
    ///
    ///这是`send_input` 的同步版本，用于在 GPUI 事件处理器中调用。
    /// 内部使用后台线程执行异步操作。
    ///
    /// # Arguments
    ///
    /// * `data` - 要发送的字节数据
    pub fn send_input_sync(&self, data: &[u8]) {
        if !self.is_connected_sync() {
            warn!("Cannot send input: not connected");
            return;
        }

        let data = data.to_vec();
        let connection_manager = self.connection_manager.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                if let Err(e) = connection_manager.write(&data).await {
                    error!("Failed to send input: {}", e);
                }
            });
        });
    }

    /// 调整终端大小
    ///
    /// 同时调整本地终端状态机和远端终端的大小。
    ///
    /// # Arguments
    ///
    /// * `rows` - 新的行数
    /// * `cols` - 新的列数
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 调整成功
    /// * `Err(ConnectionError)` - 调整失败（仅当连接已建立但调整失败时）
    pub async fn resize(&self, rows: u16, cols: u16) -> Result<(), ConnectionError> {
        // 先调整本地终端状态机
        self.terminal.resize(rows, cols);
        info!("Local terminal resized to {}x{}", cols, rows);

        // 如果已连接，同步到远端
        if self.is_connected().await {
            self.connection_manager.resize(rows, cols).await?;
            info!("Remote terminal resized to {}x{}", cols, rows);
        }

        Ok(())
    }

    /// 获取可渲染的终端内容
    ///
    /// 返回终端实例的引用，用于渲染终端内容。
    ///
    /// # Returns
    ///
    /// 返回 `Arc<FairMutex<Term<EventProxy>>>`，调用者需要锁定后访问。
    ///
    /// # Example
    ///
    /// ```ignore
    /// let term = coordinator.renderable_content();
    /// let guard = term.lock();
    /// let content = guard.renderable_content();
    /// //渲染 content...
    /// ```
    pub fn renderable_content(&self) -> Arc<FairMutex<Term<EventProxy>>> {
        self.terminal.term()
    }

    /// 处理输入数据（直接送入终端状态机）
    ///
    /// 这个方法用于直接将数据送入终端状态机，不经过后端连接。
    /// 主要用于测试或本地回显。
    pub fn advance_bytes(&self, bytes: &[u8]) {
        self.terminal.advance_bytes(bytes);
    }

    /// 关闭会话
    ///
    /// 停止数据泵并关闭后端连接。
    pub async fn close(&self) {
        info!("Closing session...");

        // 停止数据泵
        self.stop_data_pump();

        // 等待数据泵停止
        let mut attempts = 0;
        while self.is_data_pump_running() && attempts < 10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            attempts += 1;
        }

        if self.is_data_pump_running() {
            warn!("Data pump did not stop gracefully");
        }

        // 关闭连接
        self.connection_manager.close().await;

        info!("Session closed");
    }

    /// 获取终端事件接收器
    ///
    /// 返回终端事件接收器，用于处理终端产生的事件（如 PtyWrite、Bell 等）。
    /// 此方法只能调用一次。
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<TerminalEvent>> {
        self.event_rx.write().take()
    }

    /// 滚动终端
    pub fn scroll(&self, delta: i32) {
        self.terminal.scroll(delta);
    }

    /// 重置滚动位置
    pub fn reset_scroll(&self) {
        self.terminal.reset_scroll();
    }

    /// 标记有新数据需要重绘///
    /// 当数据泵接收到新数据时调用此方法，
    /// UI 层可以通过 `check_and_clear_dirty()` 检查是否需要重绘。
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// 检查并清除脏标记（带帧率限制）
    ///
    /// 如果有新数据需要重绘且距离上次重绘超过最小间隔，返回 `true` 并清除标记；
    /// 否则返回 `false`。
    ///
    /// 帧率限制约为 60fps（16ms 间隔），避免高频数据场景下 CPU 占用过高。
    ///
    /// # Returns
    ///
    /// * `true` - 有新数据且可以重绘
    /// * `false` - 无新数据或距离上次重绘时间太短
    pub fn check_and_clear_dirty(&self) -> bool {
        // 如果没有脏标记，直接返回
        if !self.dirty.load(Ordering::SeqCst) {
            return false;
        }

        // 检查帧率限制
        let now = self.start_instant.elapsed().as_millis() as u64;
        let last = self.last_render_time.load(Ordering::SeqCst);

        if now.saturating_sub(last) < MIN_RENDER_INTERVAL_MS {
            // 距离上次重绘时间太短，保留脏标记，稍后再重绘
            return false;
        }

        // 更新上次重绘时间并清除脏标记
        self.last_render_time.store(now, Ordering::SeqCst);
        self.dirty.swap(false, Ordering::SeqCst)
    }

    /// 检查是否有新数据（不清除标记）
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeterm_core::ConnectionState;

    #[tokio::test]
    async fn test_session_coordinator_new() {
        let coordinator = SessionCoordinator::with_defaults();

        assert!(!coordinator.is_connected().await);
        assert!(!coordinator.is_data_pump_running());
        assert!(matches!(
            coordinator.connection_state().await,
            ConnectionState::Idle
        ));
    }

    #[test]
    fn test_session_coordinator_terminal_size() {
        let config = TerminalConfig::new(120, 40);
        let coordinator = SessionCoordinator::new(config);

        let size = coordinator.terminal_size();
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);
    }

    #[test]
    fn test_session_coordinator_advance_bytes() {
        let coordinator = SessionCoordinator::with_defaults();

        // 应该不会 panic
        coordinator.advance_bytes(b"Hello, World!");
        coordinator.advance_bytes(b"\x1b[31mRed\x1b[0m");
    }

    #[test]
    fn test_session_coordinator_scroll() {
        let coordinator = SessionCoordinator::with_defaults();

        // 应该不会 panic
        coordinator.scroll(5);
        coordinator.scroll(-5);
        coordinator.reset_scroll();
    }

    #[test]
    fn test_session_coordinator_stop_data_pump() {
        let coordinator = SessionCoordinator::with_defaults();

        // 停止未运行的数据泵应该是安全的
        coordinator.stop_data_pump();
        assert!(!coordinator.is_data_pump_running());
    }

    #[test]
    fn test_session_coordinator_take_event_receiver() {
        let coordinator = SessionCoordinator::with_defaults();

        // 第一次应该能获取到
        let rx1 = coordinator.take_event_receiver();
        assert!(rx1.is_some());

        // 第二次应该返回 None
        let rx2 = coordinator.take_event_receiver();
        assert!(rx2.is_none());
    }

    #[tokio::test]
    async fn test_session_coordinator_send_input_disconnected() {
        let coordinator = SessionCoordinator::with_defaults();

        // 未连接时发送应该返回错误
        let result = coordinator.send_input(b"test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_coordinator_resize_disconnected() {
        let coordinator = SessionCoordinator::with_defaults();

        // 未连接时调整大小应该只影响本地
        let result = coordinator.resize(30, 100).await;
        assert!(result.is_ok());

        let size = coordinator.terminal_size();
        assert_eq!(size.rows, 30);
        assert_eq!(size.cols, 100);
    }
}
