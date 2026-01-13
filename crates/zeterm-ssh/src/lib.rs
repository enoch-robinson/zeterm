//! Zeterm SSH 模块
//!
//! 提供基于 russh 的 SSH 连接实现。
//!
//! # 模块结构
//!
//! - `config` - SSH 连接配置
//! - `handler` - SSH 事件处理器
//! - `connection` - SSH 连接实现
//! - `auth` - 认证重试与策略
//! - `agent` - SSH Agent 支持
//!
//! # 示例
//!
//! ```ignore
//! use zeterm_ssh::{SshConfig, SshConnection};
//!
//! // 创建配置
//! let config = SshConfig::new("example.com", "user")
//!     .with_port(22)
//!     .with_password("secret");
//!
//! // 创建连接
//! let conn = SshConnection::new(config);
//!
//! // 建立连接
//! conn.connect().await?;
//!
//! // 使用连接...
//! conn.write(b"ls -la\n").await?;
//!
//! // 关闭连接
//! conn.close().await?;
//! ```

mod agent;
mod auth;
mod config;
mod connection;
mod handler;
mod keepalive;
mod known_hosts;
mod reconnect;

// 重新导出主要类型
pub use agent::{
    AgentAuthConfig, AgentAuthResult, AgentError, AgentKeyInfo, AgentStatus,
    check_agent_status_sync, get_agent_socket_path, is_agent_available, log_agent_status,
};
pub use auth::{
    AuthAttemptResult, AuthCredential, AuthMethodType, AuthProgressCallback, AuthRetryConfig,
    AuthState, AuthStrategy, DEFAULT_MAX_RETRIES, DEFAULT_RETRY_DELAY_MS, LoggingAuthCallback,
    PasswordRetrier, convert_auth_error,
};
pub use config::{AuthMethod, HostKeyVerification, SshConfig, SshConfigError};
pub use connection::SshConnection;
pub use handler::{
    DataReceiver, DataSender, HandlerState, HostKeyConfirmCallback, SshHandler, create_data_channel,
};
pub use reconnect::{
    DEFAULT_BACKOFF_MULTIPLIER, DEFAULT_INITIAL_DELAY_MS, DEFAULT_MAX_DELAY_MS,
    DEFAULT_MAX_RECONNECT_ATTEMPTS, ExponentialBackoff, LoggingReconnectCallback,
    ReconnectCallback, ReconnectEvent, ReconnectPolicy, ReconnectState,
};

pub use keepalive::{
    DEFAULT_KEEPALIVE_TIMEOUT_SECS, DEFAULT_MAX_MISSED_KEEPALIVES, KeepaliveCallback,
    KeepaliveConfig, KeepaliveEvent, KeepaliveManager, KeepaliveState, KeepaliveStats,
    LoggingKeepaliveCallback,
};

pub use known_hosts::{
    HostKeyEntry, KNOWN_HOSTS_FILENAME, KeyType, KnownHostsError, KnownHostsStore,
    VerificationResult,
};

// 重新导出常量
pub use config::{DEFAULT_CONNECT_TIMEOUT_SECS, DEFAULT_KEEPALIVE_INTERVAL_SECS, DEFAULT_SSH_PORT};
