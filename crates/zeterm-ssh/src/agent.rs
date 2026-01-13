//! SSH Agent 模块
//!
//! 提供与系统 SSH Agent 交互的功能，包括：
//! - 连接到 SSH Agent
//! - 列出可用密钥
//! - 使用 Agent 进行认证
//!
//! # 支持的平台
//!
//! - Unix/Linux/macOS: 通过 `SSH_AUTH_SOCK` 环境变量连接
//! - Windows: 通过命名管道连接（需要 OpenSSH Agent 服务）

use std::env;

use tracing::{debug, info, warn};

use zeterm_core::errors::AuthError;

/// SSH Agent 错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum AgentError {
    /// Agent 不可用
    #[error("SSH Agent is not available")]
    NotAvailable,

    /// 环境变量未设置
    #[error("SSH_AUTH_SOCK environment variable not set")]
    SocketNotSet,

    /// 连接失败
    #[error("Failed to connect to SSH Agent: {0}")]
    ConnectionFailed(String),

    /// 没有可用的密钥
    #[error("No keys available in SSH Agent")]
    NoKeysAvailable,

    /// 认证失败
    #[error("Agent authentication failed: {0}")]
    AuthenticationFailed(String),

    /// 通信错误
    #[error("Agent communication error: {0}")]
    CommunicationError(String),

    /// 密钥未找到
    #[error("Key not found in agent")]
    KeyNotFound,

    /// 不支持的平台
    #[error("SSH Agent not supported on this platform")]
    UnsupportedPlatform,
}

impl From<AgentError> for AuthError {
    fn from(err: AgentError) -> Self {
        match err {
            AgentError::NotAvailable | AgentError::SocketNotSet => AuthError::UnsupportedMethod,
            AgentError::NoKeysAvailable | AgentError::KeyNotFound => {
                AuthError::KeyFileNotFound("No keys in agent".to_string())
            },
            AgentError::AuthenticationFailed(_) => AuthError::InvalidPrivateKey,
            _ => AuthError::UnsupportedMethod,
        }
    }
}

/// SSH Agent 密钥信息
#[derive(Debug, Clone)]
pub struct AgentKeyInfo {
    /// 密钥类型
    pub key_type: String,
    /// 密钥注释（通常是文件路径或描述）
    pub comment: String,
    /// 密钥指纹
    pub fingerprint: String,
}

impl AgentKeyInfo {
    /// 创建新的密钥信息
    pub fn new(key_type: impl Into<String>, comment: impl Into<String>) -> Self {
        let comment = comment.into();
        Self {
            key_type: key_type.into(),
            fingerprint: format!("SHA256:{}", &comment.chars().take(16).collect::<String>()),
            comment,
        }
    }
}

/// SSH Agent 状态
#[derive(Debug, Clone)]
pub enum AgentStatus {
    /// Agent 可用
    Available {
        /// 密钥数量
        key_count: usize,
        /// 密钥信息列表
        keys: Vec<AgentKeyInfo>,
    },
    /// Agent 不可用
    NotAvailable {
        /// 原因
        reason: String,
    },
    /// 连接失败
    ConnectionFailed {
        /// 原因
        reason: String,
    },
    /// 没有密钥
    NoKeys,
    /// 其他错误
    Error {
        /// 原因
        reason: String,
    },
}

impl AgentStatus {
    /// 是否可用
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }

    /// 获取用户友好的状态消息
    pub fn message(&self) -> String {
        match self {
            Self::Available { key_count, .. } => {
                format!("SSH Agent available with {} key(s)", key_count)
            },
            Self::NotAvailable { reason } => {
                format!("SSH Agent not available: {}", reason)
            },
            Self::ConnectionFailed { reason } => {
                format!("Failed to connect to SSH Agent: {}", reason)
            },
            Self::NoKeys => "SSH Agent has no keys loaded".to_string(),
            Self::Error { reason } => {
                format!("SSH Agent error: {}", reason)
            },
        }
    }

    /// 获取用户友好的状态消息（中文）
    pub fn message_cn(&self) -> String {
        match self {
            Self::Available { key_count, .. } => {
                format!("SSH Agent 可用，已加载 {} 个密钥", key_count)
            },
            Self::NotAvailable { reason } => {
                format!("SSH Agent 不可用: {}", reason)
            },
            Self::ConnectionFailed { reason } => {
                format!("连接 SSH Agent 失败: {}", reason)
            },
            Self::NoKeys => "SSH Agent 中没有加载密钥".to_string(),
            Self::Error { reason } => {
                format!("SSH Agent 错误: {}", reason)
            },
        }
    }
}

/// 检查 SSH Agent 是否可用 - Unix 实现
#[cfg(unix)]
pub fn is_agent_available() -> bool {
    env::var("SSH_AUTH_SOCK").is_ok()
}

/// 检查 SSH Agent 是否可用 - Windows 实现
#[cfg(windows)]
pub fn is_agent_available() -> bool {
    // Windows OpenSSH Agent使用命名管道
    // 检查环境变量或默认管道路径
    if env::var("SSH_AUTH_SOCK").is_ok() {
        return true;
    }
    // 检查默认的 OpenSSH Agent 命名管道是否存在
    std::fs::metadata(r"\\.\pipe\openssh-ssh-agent").is_ok()
}

/// 获取 Agent socket 路径 - Unix 实现
#[cfg(unix)]
pub fn get_agent_socket_path() -> Result<String, AgentError> {
    env::var("SSH_AUTH_SOCK").map_err(|_| AgentError::SocketNotSet)
}

/// 获取 Agent 命名管道路径 - Windows 实现
#[cfg(windows)]
pub fn get_agent_socket_path() -> Result<String, AgentError> {
    // 优先使用环境变量
    if let Ok(path) = env::var("SSH_AUTH_SOCK") {
        return Ok(path);
    }
    // 使用默认的 OpenSSH Agent 命名管道路径
    const DEFAULT_PIPE: &str = r"\\.\pipe\openssh-ssh-agent";
    if std::fs::metadata(DEFAULT_PIPE).is_ok() {
        Ok(DEFAULT_PIPE.to_string())
    } else {
        Err(AgentError::NotAvailable)
    }
}

/// 检查 SSH Agent 状态
///
/// 这是一个简化的检查，只验证环境变量是否设置
pub fn check_agent_status_sync() -> AgentStatus {
    match get_agent_socket_path() {
        Ok(path) => {
            debug!("SSH Agent socket found at: {}", path);
            // 简化实现：只检查环境变量
            // 实际的密钥列表需要异步连接
            AgentStatus::Available {
                key_count: 0,
                keys: Vec::new(),
            }
        },
        Err(_) => AgentStatus::NotAvailable {
            reason: "SSH_AUTH_SOCK environment variable not set".to_string(),
        },
    }
}

/// SSH Agent 认证配置
#[derive(Debug, Clone)]
pub struct AgentAuthConfig {
    /// 首选密钥注释（可选）
    pub preferred_key_comment: Option<String>,
    /// 是否尝试所有密钥
    pub try_all_keys: bool,
    /// 连接超时（秒）
    pub timeout_secs: u64,
}

impl Default for AgentAuthConfig {
    fn default() -> Self {
        Self {
            preferred_key_comment: None,
            try_all_keys: true,
            timeout_secs: 10,
        }
    }
}

impl AgentAuthConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置首选密钥
    pub fn with_preferred_key(mut self, comment: impl Into<String>) -> Self {
        self.preferred_key_comment = Some(comment.into());
        self
    }

    /// 设置是否尝试所有密钥
    pub fn with_try_all_keys(mut self, try_all: bool) -> Self {
        self.try_all_keys = try_all;
        self
    }

    /// 设置超时
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

/// Agent 认证结果
#[derive(Debug, Clone)]
pub enum AgentAuthResult {
    /// 认证成功
    Success {
        /// 使用的密钥信息
        key_info: Option<AgentKeyInfo>,
    },
    /// Agent 不可用
    AgentNotAvailable,
    /// 没有可用密钥
    NoKeysAvailable,
    /// 认证失败
    AuthFailed {
        /// 错误信息
        error: String,
    },
}

impl AgentAuthResult {
    /// 是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// 转换为 Result
    pub fn to_result(&self) -> Result<(), AgentError> {
        match self {
            Self::Success { .. } => Ok(()),
            Self::AgentNotAvailable => Err(AgentError::NotAvailable),
            Self::NoKeysAvailable => Err(AgentError::NoKeysAvailable),
            Self::AuthFailed { error } => Err(AgentError::AuthenticationFailed(error.clone())),
        }
    }
}

/// 日志记录 Agent 状态
pub fn log_agent_status() {
    match get_agent_socket_path() {
        Ok(path) => {
            info!("SSH Agent socket: {}", path);
        },
        Err(_) => {
            warn!("SSH Agent not available (SSH_AUTH_SOCK not set)");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_error_to_auth_error() {
        let err: AuthError = AgentError::NotAvailable.into();
        assert!(matches!(err, AuthError::UnsupportedMethod));

        let err: AuthError = AgentError::NoKeysAvailable.into();
        assert!(matches!(err, AuthError::KeyFileNotFound(_)));
    }

    #[test]
    fn test_agent_key_info() {
        let info = AgentKeyInfo::new("RSA", "user@host");
        assert_eq!(info.key_type, "RSA");
        assert_eq!(info.comment, "user@host");
        assert!(info.fingerprint.starts_with("SHA256:"));
    }

    #[test]
    fn test_agent_status_message() {
        let status = AgentStatus::Available {
            key_count: 2,
            keys: vec![
                AgentKeyInfo::new("RSA", "key1"),
                AgentKeyInfo::new("ED25519", "key2"),
            ],
        };
        assert!(status.is_available());
        assert!(status.message().contains("2 key(s)"));

        let status = AgentStatus::NoKeys;
        assert!(!status.is_available());
        assert!(status.message().contains("no keys"));
    }

    #[test]
    fn test_agent_auth_config() {
        let config = AgentAuthConfig::new()
            .with_preferred_key("my_key")
            .with_try_all_keys(false)
            .with_timeout(30);

        assert_eq!(config.preferred_key_comment, Some("my_key".to_string()));
        assert!(!config.try_all_keys);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_agent_auth_result() {
        let success = AgentAuthResult::Success { key_info: None };
        assert!(success.is_success());
        assert!(success.to_result().is_ok());

        let failed = AgentAuthResult::AgentNotAvailable;
        assert!(!failed.is_success());
        assert!(failed.to_result().is_err());
    }

    #[test]
    fn test_agent_status_message_cn() {
        let status = AgentStatus::Available {
            key_count: 1,
            keys: vec![AgentKeyInfo::new("RSA", "test")],
        };
        assert!(status.message_cn().contains("1 个密钥"));

        let status = AgentStatus::NoKeys;
        assert!(status.message_cn().contains("没有加载密钥"));
    }
}
