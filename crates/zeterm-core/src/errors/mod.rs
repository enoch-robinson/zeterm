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
#[derive(Debug, Clone, Error)]
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

    /// 获取用户友好的错误消息
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::InvalidPassword => "密码错误，请重新输入",
            Self::InvalidPrivateKey => "私钥文件无效或格式不正确",
            Self::InvalidKeyPassphrase => "私钥密码错误，请重新输入",
            Self::KeyFileNotFound(_) => "找不到指定的密钥文件",
            Self::HostKeyVerificationFailed => "主机密钥验证失败，可能存在安全风险",
            Self::UnsupportedMethod => "服务器不支持此认证方式",
            Self::Cancelled => "认证已取消",
            Self::Timeout => "认证超时，请检查网络连接",
        }
    }

    /// 获取用户友好的错误消息（英文）
    pub fn user_message_en(&self) -> &'static str {
        match self {
            Self::InvalidPassword => "Invalid password, please try again",
            Self::InvalidPrivateKey => "Invalid private key file or format",
            Self::InvalidKeyPassphrase => "Invalid key passphrase, please try again",
            Self::KeyFileNotFound(_) => "Key file not found",
            Self::HostKeyVerificationFailed => {
                "Host key verification failed, possible security risk"
            },
            Self::UnsupportedMethod => "Authentication method not supported by server",
            Self::Cancelled => "Authentication cancelled",
            Self::Timeout => "Authentication timeout, please check network connection",
        }
    }

    /// 获取建议的操作
    pub fn suggested_action(&self) -> &'static str {
        match self {
            Self::InvalidPassword => "请检查密码是否正确，注意大小写",
            Self::InvalidPrivateKey => "请确认密钥文件路径正确且格式为 OpenSSH 或 PEM",
            Self::InvalidKeyPassphrase => "请检查私钥密码是否正确",
            Self::KeyFileNotFound(_) => "请检查密钥文件路径是否正确",
            Self::HostKeyVerificationFailed => "请确认是否信任此主机，或检查 known_hosts 文件",
            Self::UnsupportedMethod => "请尝试其他认证方式（密码或其他密钥）",
            Self::Cancelled => "如需连接，请重新发起认证",
            Self::Timeout => "请检查网络连接后重试",
        }
    }

    /// 获取错误代码（用于日志和调试）
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::InvalidPassword => "AUTH_INVALID_PASSWORD",
            Self::InvalidPrivateKey => "AUTH_INVALID_KEY",
            Self::InvalidKeyPassphrase => "AUTH_INVALID_PASSPHRASE",
            Self::KeyFileNotFound(_) => "AUTH_KEY_NOT_FOUND",
            Self::HostKeyVerificationFailed => "AUTH_HOST_KEY_FAILED",
            Self::UnsupportedMethod => "AUTH_UNSUPPORTED_METHOD",
            Self::Cancelled => "AUTH_CANCELLED",
            Self::Timeout => "AUTH_TIMEOUT",
        }
    }

    /// 检查是否为用户可修复的错误
    pub fn is_user_fixable(&self) -> bool {
        matches!(
            self,
            Self::InvalidPassword
                | Self::InvalidKeyPassphrase
                | Self::KeyFileNotFound(_)
                | Self::Cancelled
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
        assert!(AuthError::InvalidKeyPassphrase.is_retryable());
        assert!(AuthError::Timeout.is_retryable());
        assert!(!AuthError::UnsupportedMethod.is_retryable());
        assert!(!AuthError::HostKeyVerificationFailed.is_retryable());
    }

    #[test]
    fn test_auth_error_user_message() {
        let err = AuthError::InvalidPassword;
        assert!(!err.user_message().is_empty());
        assert!(!err.user_message_en().is_empty());
        assert!(!err.suggested_action().is_empty());
    }

    #[test]
    fn test_auth_error_code() {
        assert_eq!(
            AuthError::InvalidPassword.error_code(),
            "AUTH_INVALID_PASSWORD"
        );
        assert_eq!(AuthError::Timeout.error_code(), "AUTH_TIMEOUT");
    }

    #[test]
    fn test_auth_error_user_fixable() {
        assert!(AuthError::InvalidPassword.is_user_fixable());
        assert!(AuthError::InvalidKeyPassphrase.is_user_fixable());
        assert!(!AuthError::UnsupportedMethod.is_user_fixable());
        assert!(!AuthError::HostKeyVerificationFailed.is_user_fixable());
    }
}
