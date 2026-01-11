//! Zeterm Mock - Mock连接实现
//!
//! 提供用于测试和开发的 Mock 终端连接后端。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::BoxStream;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tracing::{debug, info};

use zeterm_core::errors::ConnectionError;
use zeterm_core::traits::{ConnectionInfo, ConnectionType, TerminalConnection};

/// Mock 连接配置
#[derive(Debug, Clone)]
pub struct MockConfig {
    /// 自动输出间隔（毫秒），None 表示不自动输出
    pub auto_output_interval_ms: Option<u64>,
    /// 自动输出的内容
    pub auto_output_content: String,
    /// 是否回显输入
    pub echo_input: bool,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            auto_output_interval_ms: Some(1000),
            auto_output_content: "Hello from MockConnection!\r\n".to_string(),
            echo_input: true,
        }
    }
}

/// Mock 终端连接
///
/// 用于测试和开发阶段，无需真实SSH 服务器。
pub struct MockConnection {
    /// 配置
    config: MockConfig,
    /// 数据接收通道发送端
    data_tx: mpsc::Sender<Vec<u8>>,
    /// 数据接收通道接收端（只能取出一次）
    data_rx: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
    /// 是否已连接
    connected: AtomicBool,
    /// 连接时间
    connected_at: Mutex<Option<Instant>>,
    /// 终端尺寸
    size: Mutex<(u16, u16)>,
    /// 自动输出任务取消标志
    cancel_auto_output: AtomicBool,
}

impl MockConnection {
    /// 创建新的 Mock 连接
    pub fn new(config: MockConfig) -> Self {
        let (data_tx, data_rx) = mpsc::channel(1024);

        Self {
            config,
            data_tx,
            data_rx: Mutex::new(Some(data_rx)),
            connected: AtomicBool::new(false),
            connected_at: Mutex::new(None),
            size: Mutex::new((24, 80)),
            cancel_auto_output: AtomicBool::new(false),
        }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(MockConfig::default())
    }

    /// 连接（模拟）
    pub async fn connect(&self) -> Result<(), ConnectionError> {
        info!("MockConnection: Connecting...");

        // 模拟连接延迟
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        self.connected.store(true, Ordering::SeqCst);
        *self.connected_at.lock().await = Some(Instant::now());

        // 发送欢迎消息
        let welcome = format!(
            "\x1b[32m=== Mock Terminal ===\x1b[0m\r\nConnected at: {:?}\r\n\
             Type anything to echo...\r\n\r\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let _ = self.data_tx.send(welcome.into_bytes()).await;

        // 启动自动输出任务
        if let Some(interval_ms) = self.config.auto_output_interval_ms {
            self.start_auto_output(interval_ms).await;
        }

        info!("MockConnection: Connected");
        Ok(())
    }

    /// 启动自动输出任务
    async fn start_auto_output(&self, interval_ms: u64) {
        let tx = self.data_tx.clone();
        let content = self.config.auto_output_content.clone();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // 保存取消标志的引用
        self.cancel_auto_output.store(false, Ordering::SeqCst);

        let cancel = cancel_flag.clone();
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(interval_ms));

            loop {
                interval.tick().await;

                if cancel.load(Ordering::SeqCst) {
                    debug!("MockConnection: Auto output cancelled");
                    break;
                }

                if tx.send(content.clone().into_bytes()).await.is_err() {
                    debug!("MockConnection: Channel closed, stopping auto output");
                    break;
                }
            }
        });
    }

    /// 注入输出数据（用于测试）
    pub async fn inject_output(&self, data: &[u8]) -> Result<(), ConnectionError> {
        self.data_tx
            .send(data.to_vec())
            .await
            .map_err(|_| ConnectionError::ChannelClosed)
    }

    /// 注入 ANSI 彩色文本
    pub async fn inject_colored_text(
        &self,
        text: &str,
        color_code: u8,
    ) -> Result<(), ConnectionError> {
        let colored = format!("\x1b[{}m{}\x1b[0m", color_code, text);
        self.inject_output(colored.as_bytes()).await
    }
}

#[async_trait]
impl TerminalConnection for MockConnection {
    async fn write(&self, bytes: &[u8]) -> Result<(), ConnectionError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(ConnectionError::Disconnected);
        }

        debug!("MockConnection: Write {} bytes", bytes.len());

        // 如果启用了回显，将输入回显到输出
        if self.config.echo_input {
            let _ = self.data_tx.send(bytes.to_vec()).await;
        }

        Ok(())
    }

    async fn resize(&self, rows: u16, cols: u16) -> Result<(), ConnectionError> {
        if !self.connected.load(Ordering::SeqCst) {
            return Err(ConnectionError::Disconnected);
        }

        debug!("MockConnection: Resize to {}x{}", cols, rows);
        *self.size.lock().await = (rows, cols);

        // 发送尺寸变化通知
        let msg = format!("\x1b[33m[Terminal resized to {}x{}]\x1b[0m\r\n", cols, rows);
        let _ = self.data_tx.send(msg.into_bytes()).await;

        Ok(())
    }

    fn receive_stream(&self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>> {
        //尝试获取接收端（只能获取一次）
        let rx = {
            let mut guard = self.data_rx.blocking_lock();
            guard.take()
        };

        match rx {
            Some(rx) => {
                let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
                stream.map(Ok).boxed()
            },
            None => {
                // 已经被取走了，返回空流
                futures::stream::empty().boxed()
            },
        }
    }

    async fn close(&self) -> Result<(), ConnectionError> {
        info!("MockConnection: Closing...");

        // 取消自动输出
        self.cancel_auto_output.store(true, Ordering::SeqCst);

        // 标记为断开
        self.connected.store(false, Ordering::SeqCst);

        info!("MockConnection: Closed");
        Ok(())
    }
}

impl ConnectionInfo for MockConnection {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    fn connection_type(&self) -> ConnectionType {
        ConnectionType::Mock
    }

    fn remote_address(&self) -> Option<String> {
        Some("mock://localhost".to_string())
    }

    fn connected_at(&self) -> Option<Instant> {
        // 使用 try_lock 避免阻塞
        self.connected_at.try_lock().ok().and_then(|guard| *guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_connection_create() {
        let conn = MockConnection::with_defaults();
        assert!(!conn.is_connected());
        assert_eq!(conn.connection_type(), ConnectionType::Mock);
    }

    #[tokio::test]
    async fn test_mock_connection_connect() {
        let conn = MockConnection::new(MockConfig {
            auto_output_interval_ms: None,
            ..Default::default()
        });

        conn.connect().await.unwrap();
        assert!(conn.is_connected());
        assert!(conn.connected_at().is_some());
    }

    #[tokio::test]
    async fn test_mock_connection_write() {
        let conn = MockConnection::new(MockConfig {
            auto_output_interval_ms: None,
            echo_input: true,
            ..Default::default()
        });

        conn.connect().await.unwrap();
        conn.write(b"test").await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_connection_resize() {
        let conn = MockConnection::new(MockConfig {
            auto_output_interval_ms: None,
            ..Default::default()
        });

        conn.connect().await.unwrap();
        conn.resize(30, 100).await.unwrap();

        let size = *conn.size.lock().await;
        assert_eq!(size, (30, 100));
    }

    #[tokio::test]
    async fn test_mock_connection_close() {
        let conn = MockConnection::new(MockConfig {
            auto_output_interval_ms: None,
            ..Default::default()
        });

        conn.connect().await.unwrap();
        assert!(conn.is_connected());

        conn.close().await.unwrap();
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_mock_connection_inject_output() {
        let conn = MockConnection::new(MockConfig {
            auto_output_interval_ms: None,
            ..Default::default()
        });

        conn.inject_output(b"test data").await.unwrap();
    }
}
