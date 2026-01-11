//! 错误类型定义
//!
//! 定义 Zeterm 的错误类型体系。

use thiserror::Error;

/// 连接错误
///
/// 表示终端连接过程中可能发生的各种错误。
#[derive(Debug, Error)]
pub enum ConnectionError {
    /// DNS 解析失败
    #[error("DNS resolution failed: {0}")]
    DnsResolution(String),

    /// 连接超时
    #[error("Connection timeout")]
    Timeout,

    /// 连接被拒绝
    #[error("Connection refused")]
    Refused,

    /// 网络不可达
    #[error("Network unreachable")]
    NetworkUnreachable,

    /// 连接已断开
    #[error("Connection disconnected")]
    Disconnected,

    ///未连接
    #[error("Not connected")]
    NotConnected,

    /// 连接错误
    #[error("Connection error: {0}")]
    Connection(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// 认证错误
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(String),

    /// SSH 协议错误
    #[error("SSH error: {0}")]
    Ssh(String),

    /// 通道已关闭
    #[error("Channel closed")]
    ChannelClosed,

    /// 无效的终端大小
    #[error("Invalid terminal size: {rows}x{cols}")]
    InvalidSize { rows: u16, cols: u16 },

    /// 其他错误
    #[error("Other error: {0}")]
    Other(String),
}

impl ConnectionError {
    /// 检查错误是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Disconnected | Self::NetworkUnreachable
        )
    }

    /// 检查是否是致命错误
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::Refused | Self::DnsResolution(_) | Self::Authentication(_)
        )
    }
}

/// 认证错误
#[derive(Debug, Error)]
pub enum AuthError {
    /// 密码错误
    #[error("Invalid password")]
    InvalidPassword,

    /// 私钥无效
    #[error("Invalid private key")]
    InvalidPrivateKey,

    /// 私钥密码错误
    #[error("Invalid key passphrase")]
    InvalidKeyPassphrase,

    /// 密钥文件不存在
    #[error("Key file not found: {0}")]
    KeyFileNotFound(String),

    /// 主机密钥验证失败
    #[error("Host key verification failed")]
    HostKeyVerificationFailed,

    /// 不支持的认证方式
    #[error("Unsupported authentication method")]
    UnsupportedMethod,

    /// 认证被取消
    #[error("Authentication cancelled")]
    Cancelled,

    /// 认证超时
    #[error("Authentication timeout")]
    Timeout,
}

impl AuthError {
    /// 检查是否可以重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::InvalidPassword | Self::InvalidKeyPassphrase | Self::Timeout
        )
    }
}

/// 存储错误
#[derive(Debug, Error)]
pub enum StorageError {
    /// 数据库错误
    #[error("Database error: {0}")]
    Database(String),

    /// 记录未找到
    #[error("Record not found: {0}")]
    NotFound(String),

    /// 重复键
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// 配置错误
#[derive(Debug, Error)]
pub enum ConfigError {
    /// 文件未找到
    #[error("Config file not found: {0}")]
    FileNotFound(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    ParseError(String),

    /// 无效值
    #[error("Invalid value for {key}: {message}")]
    InvalidValue { key: String, message: String },

    /// 缺少必填项
    #[error("Missing required field: {0}")]
    MissingRequired(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_retryable() {
        assert!(ConnectionError::Timeout.is_retryable());
        assert!(ConnectionError::Disconnected.is_retryable());
        assert!(!ConnectionError::Refused.is_retryable());
    }

    #[test]
    fn test_connection_error_fatal() {
        assert!(ConnectionError::Refused.is_fatal());
        assert!(!ConnectionError::Timeout.is_fatal());
    }

    #[test]
    fn test_auth_error_retryable() {
        assert!(AuthError::InvalidPassword.is_retryable());
        assert!(!AuthError::UnsupportedMethod.is_retryable());
    }
}
