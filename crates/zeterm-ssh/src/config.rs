//! SSH 连接配置模块
//!
//! 定义 SSH 连接所需的配置参数。

use std::path::PathBuf;
use std::time::Duration;

///默认 SSH 端口
pub const DEFAULT_SSH_PORT: u16 = 22;

/// 默认连接超时时间（秒）
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;

/// 默认心跳间隔（秒）
pub const DEFAULT_KEEPALIVE_INTERVAL_SECS: u64 = 60;

/// SSH 连接配置
#[derive(Debug, Clone)]
pub struct SshConfig {
    /// 主机地址
    pub host: String,
    /// 端口号
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 认证方式
    pub auth_method: AuthMethod,
    /// 连接超时时间
    pub connect_timeout: Duration,
    /// 心跳间隔（None 表示禁用）
    pub keepalive_interval: Option<Duration>,
    /// 主机密钥验证策略
    pub host_key_verification: HostKeyVerification,
    /// 终端类型
    pub terminal_type: String,
    /// 初始终端宽度（列数）
    pub terminal_cols: u16,
    /// 初始终端高度（行数）
    pub terminal_rows: u16,
    /// 回退认证方法列表
    pub fallback_auth_methods: Vec<AuthMethod>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_SSH_PORT,
            username: String::new(),
            auth_method: AuthMethod::None,
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            keepalive_interval: Some(Duration::from_secs(DEFAULT_KEEPALIVE_INTERVAL_SECS)),
            host_key_verification: HostKeyVerification::AskOnFirstConnect,
            terminal_type: "xterm-256color".to_string(),
            terminal_cols: 80,
            terminal_rows: 24,
            fallback_auth_methods: Vec::new(),
        }
    }
}

impl SshConfig {
    /// 创建新的 SSH 配置
    pub fn new(host: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            username: username.into(),
            ..Default::default()
        }
    }

    /// 设置端口
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// 设置密码认证
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.auth_method = AuthMethod::Password(password.into());
        self
    }

    /// 设置公钥认证
    pub fn with_key_file(mut self, key_path: impl Into<PathBuf>) -> Self {
        self.auth_method = AuthMethod::PublicKey {
            key_path: key_path.into(),
            passphrase: None,
        };
        self
    }

    /// 设置带密码的公钥认证
    pub fn with_key_file_and_passphrase(
        mut self,
        key_path: impl Into<PathBuf>,
        passphrase: impl Into<String>,
    ) -> Self {
        self.auth_method = AuthMethod::PublicKey {
            key_path: key_path.into(),
            passphrase: Some(passphrase.into()),
        };
        self
    }

    /// 设置 SSH Agent 认证
    pub fn with_agent(mut self) -> Self {
        self.auth_method = AuthMethod::Agent;
        self
    }

    /// 添加回退认证方法
    /// 当主认证方法失败时，会按顺序尝试回退方法
    pub fn with_fallback(mut self, method: AuthMethod) -> Self {
        self.fallback_auth_methods.push(method);
        self
    }
    /// 添加密码作为回退认证
    pub fn with_password_fallback(mut self, password: impl Into<String>) -> Self {
        self.fallback_auth_methods
            .push(AuthMethod::Password(password.into()));
        self
    }

    /// 添加公钥作为回退认证
    pub fn with_key_fallback(mut self, key_path: impl Into<PathBuf>) -> Self {
        self.fallback_auth_methods.push(AuthMethod::PublicKey {
            key_path: key_path.into(),
            passphrase: None,
        });
        self
    }

    /// 添加 Agent 作为回退认证
    pub fn with_agent_fallback(mut self) -> Self {
        self.fallback_auth_methods.push(AuthMethod::Agent);
        self
    }

    /// 获取所有认证方法（主方法 + 回退方法）
    pub fn all_auth_methods(&self) -> Vec<&AuthMethod> {
        let mut methods = vec![&self.auth_method];
        methods.extend(self.fallback_auth_methods.iter());
        methods
    }
    /// 检查是否有回退认证方法
    pub fn has_fallback(&self) -> bool {
        !self.fallback_auth_methods.is_empty()
    }

    /// 设置连接超时
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// 设置心跳间隔
    pub fn with_keepalive(mut self, interval: Option<Duration>) -> Self {
        self.keepalive_interval = interval;
        self
    }

    /// 设置主机密钥验证策略
    pub fn with_host_key_verification(mut self, policy: HostKeyVerification) -> Self {
        self.host_key_verification = policy;
        self
    }

    /// 设置终端类型
    pub fn with_terminal_type(mut self, term_type: impl Into<String>) -> Self {
        self.terminal_type = term_type.into();
        self
    }

    /// 设置终端尺寸
    pub fn with_terminal_size(mut self, cols: u16, rows: u16) -> Self {
        self.terminal_cols = cols;
        self.terminal_rows = rows;
        self
    }

    /// 获取完整的连接地址
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// 验证配置是否有效
    pub fn validate(&self) -> Result<(), SshConfigError> {
        if self.host.is_empty() {
            return Err(SshConfigError::MissingHost);
        }
        if self.username.is_empty() {
            return Err(SshConfigError::MissingUsername);
        }
        if matches!(self.auth_method, AuthMethod::None) {
            return Err(SshConfigError::MissingAuthMethod);
        }
        if let AuthMethod::PublicKey { ref key_path, .. } = self.auth_method {
            if !key_path.exists() {
                return Err(SshConfigError::KeyFileNotFound(key_path.clone()));
            }
        }
        Ok(())
    }
}

/// 认证方式
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// 未设置认证方式
    None,
    /// 密码认证
    Password(String),
    /// 公钥认证
    PublicKey {
        /// 私钥文件路径
        key_path: PathBuf,
        /// 私钥密码（如果有）
        passphrase: Option<String>,
    },
    /// SSH Agent 认证
    Agent,
    /// 键盘交互认证
    KeyboardInteractive,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::None
    }
}

/// 主机密钥验证策略
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyVerification {
    /// 自动接受所有主机密钥（不安全，仅用于测试）
    AutoAccept,
    /// 严格验证，必须在 known_hosts 中存在
    Strict,
    /// 首次连接时询问用户
    AskOnFirstConnect,
    /// 使用指定的 known_hosts 文件
    KnownHostsFile(PathBuf),
}

impl Default for HostKeyVerification {
    fn default() -> Self {
        Self::AskOnFirstConnect
    }
}

/// SSH 配置错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum SshConfigError {
    #[error("Missing host address")]
    MissingHost,
    #[error("Missing username")]
    MissingUsername,
    #[error("Missing authentication method")]
    MissingAuthMethod,
    #[error("Key file not found: {0}")]
    KeyFileNotFound(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_config_default() {
        let config = SshConfig::default();
        assert_eq!(config.port, DEFAULT_SSH_PORT);
        assert_eq!(config.terminal_type, "xterm-256color");
        assert_eq!(config.terminal_cols, 80);
        assert_eq!(config.terminal_rows, 24);
    }

    #[test]
    fn test_ssh_config_builder() {
        let config = SshConfig::new("example.com", "user")
            .with_port(2222)
            .with_password("secret")
            .with_terminal_size(120, 40);

        assert_eq!(config.host, "example.com");
        assert_eq!(config.username, "user");
        assert_eq!(config.port, 2222);
        assert_eq!(config.terminal_cols, 120);
        assert_eq!(config.terminal_rows, 40);
        assert!(matches!(config.auth_method, AuthMethod::Password(_)));
    }

    #[test]
    fn test_ssh_config_address() {
        let config = SshConfig::new("example.com", "user").with_port(2222);
        assert_eq!(config.address(), "example.com:2222");
    }

    #[test]
    fn test_ssh_config_validate_missing_host() {
        let config = SshConfig::default();
        let result = config.validate();
        assert!(matches!(result, Err(SshConfigError::MissingHost)));
    }

    #[test]
    fn test_ssh_config_validate_missing_username() {
        let config = SshConfig {
            host: "example.com".to_string(),
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(SshConfigError::MissingUsername)));
    }

    #[test]
    fn test_ssh_config_validate_missing_auth() {
        let config = SshConfig::new("example.com", "user");
        let result = config.validate();
        assert!(matches!(result, Err(SshConfigError::MissingAuthMethod)));
    }

    #[test]
    fn test_ssh_config_validate_success() {
        let config = SshConfig::new("example.com", "user").with_password("secret");
        let result = config.validate();
        assert!(result.is_ok());
    }
}
