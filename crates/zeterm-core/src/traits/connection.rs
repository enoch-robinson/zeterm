//! Terminal Connection Trait
//!
//!定义终端连接的核心抽象接口，所有连接后端（SSH、Local PTY、Mock）都必须实现此Trait。

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::errors::ConnectionError;

/// 终端连接抽象 Trait
///
/// 这是整个系统最关键的抽象，定义了终端连接的统一接口。
/// 通过这个 Trait，UI 层与网络层实现彻底解耦。
///
/// # 实现要求
///
/// - 必须是 `Send + Sync`，支持跨线程共享
/// - 所有 IO 操作都是异步的
/// - `receive_stream()` 只能调用一次
///
/// # 示例
///
/// ```ignore
/// async fn run_session(mut conn: Box<dyn TerminalConnection>) {
///     // 获取输出流
///     let mut stream = conn.receive_stream();
///
///     // 发送命令
///     conn.write(b"ls -la\n").await?;
///
///     // 调整窗口大小
///     conn.resize(24, 80).await?;
///
///     // 关闭连接
///     conn.close().await?;
/// }
/// ```
#[async_trait]
pub trait TerminalConnection: Send + Sync {
    /// 发送数据到远端
    ///
    /// 将字节数据发送到远端终端。通常是用户键盘输入转换后的 ANSI 转义序列。
    ///
    /// # Arguments
    ///
    /// * `bytes` - 要发送的字节数据///
    /// # Returns
    ///
    /// * `Ok(())` - 发送成功
    /// * `Err(ConnectionError)` - 发送失败
    ///
    /// # Example
    ///
    /// ```ignore
    /// // 发送命令
    /// conn.write(b"ls -la\n").await?;
    ///
    /// // 发送 Ctrl+C
    /// conn.write(&[0x03]).await?;
    /// ```
    async fn write(&self, bytes: &[u8]) -> Result<(), ConnectionError>;

    /// 调整终端窗口大小
    ///
    /// 通知远端终端窗口大小发生变化。这会触发远端的SIGWINCH 信号。
    ///
    /// # Arguments
    ///
    /// * `rows` - 终端行数
    /// * `cols` - 终端列数
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 调整成功
    /// * `Err(ConnectionError)` - 调整失败
    ///
    /// # Example
    ///
    /// ```ignore
    /// conn.resize(24, 80).await?;
    /// ```
    async fn resize(&self, rows: u16, cols: u16) -> Result<(), ConnectionError>;

    /// 获取数据接收流
    ///
    /// 返回一个异步字节流，用于接收远端发送的数据。
    ///
    /// # 重要
    ///
    /// **此方法只能调用一次**，后续调用将返回空流。
    /// 这是因为底层的数据通道只能被消费一次。
    ///
    /// # Returns
    ///
    /// 返回一个 `BoxStream`，产生 `Result<Vec<u8>, ConnectionError>`。
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut stream = conn.receive_stream();
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(data) => terminal.advance_bytes(&data),
    ///         Err(e) => handle_error(e),
    ///     }
    /// }
    /// ```
    fn receive_stream(&self) -> BoxStream<'static, Result<Vec<u8>, ConnectionError>>;

    /// 关闭连接
    ///
    /// 优雅地关闭连接，释放所有资源。
    ///
    /// # Returns
    ///
    /// * `Ok(())` - 关闭成功
    /// * `Err(ConnectionError)` - 关闭失败
    ///
    /// # Example
    ///
    /// ```ignore
    /// conn.close().await?;
    /// ```
    async fn close(&self) -> Result<(), ConnectionError>;
}

/// 连接信息查询 Trait
///
/// 提供连接状态和元信息的查询接口。
pub trait ConnectionInfo {
    /// 检查连接是否活跃
    fn is_connected(&self) -> bool;

    /// 获取连接类型
    fn connection_type(&self) -> ConnectionType;

    /// 获取远端地址
    ///
    /// 对于 SSH 连接，返回 `host:port` 格式
    /// 对于本地 PTY，返回 `None`
    fn remote_address(&self) -> Option<String>;

    /// 获取连接建立时间
    fn connected_at(&self) -> Option<std::time::Instant>;
}

/// 连接类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionType {
    /// SSH 连接
    Ssh,
    /// 本地 PTY
    LocalPty,
    /// Telnet 连接
    Telnet,
    /// 串口连接
    Serial,
    /// Mock 连接（测试用）
    Mock,
}

impl std::fmt::Display for ConnectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionType::Ssh => write!(f, "SSH"),
            ConnectionType::LocalPty => write!(f, "Local PTY"),
            ConnectionType::Telnet => write!(f, "Telnet"),
            ConnectionType::Serial => write!(f, "Serial"),
            ConnectionType::Mock => write!(f, "Mock"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_type_display() {
        assert_eq!(ConnectionType::Ssh.to_string(), "SSH");
        assert_eq!(ConnectionType::LocalPty.to_string(), "Local PTY");
        assert_eq!(ConnectionType::Mock.to_string(), "Mock");
    }
}
