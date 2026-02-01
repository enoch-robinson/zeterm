//! 连接状态模块
//!
//! 定义连接的生命周期状态与转换规则。

mod state_machine;

pub use state_machine::{ConnectionStateMachine, StateChangeCallback};

use std::time::Instant;

use serde::{Deserialize, Serialize};

/// 连接状态
///
/// 表示终端连接的生命周期状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// 空闲状态，未连接
    Idle,

    /// 正在建立 TCP 连接
    Connecting {
        /// 当前尝试次数
        attempt: u32,
    },

    /// TCP 已连接，正在进行 SSH 认证
    Authenticating,

    /// 已连接，会话可用
    Connected {
        /// 连接建立时间
        connected_at: Instant,
    },

    /// 正在优雅关闭
    Disconnecting,

    /// 已断开
    Disconnected {
        /// 断开原因
        reason: DisconnectReason,
    },

    /// 自动重连中
    Reconnecting {
        /// 当前重连尝试次数
        attempt: u32,
    },
}

impl ConnectionState {
    /// 检查是否处于活跃连接状态
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// 检查是否可以发送数据
    pub fn can_send(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// 检查是否正在连接中
    pub fn is_connecting(&self) -> bool {
        matches!(
            self,
            Self::Connecting { .. } | Self::Authenticating | Self::Reconnecting { .. }
        )
    }

    /// 检查是否已断开
    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Idle | Self::Disconnected { .. })
    }

    /// 转换为 UI 显示状态
    pub fn to_ui_status(&self) -> UiConnectionStatus {
        match self {
            Self::Idle | Self::Disconnected { .. } | Self::Disconnecting => {
                UiConnectionStatus::Offline
            },
            Self::Connecting { .. } | Self::Authenticating => UiConnectionStatus::Connecting,
            Self::Connected { .. } => UiConnectionStatus::Online,
            Self::Reconnecting { .. } => UiConnectionStatus::Reconnecting,
        }
    }
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Idle
    }
}

/// 断开原因
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisconnectReason {
    /// 用户主动断开
    UserInitiated,

    /// 服务器关闭连接
    ServerClosed,

    /// 网络错误
    NetworkError,

    /// 认证失败
    AuthFailed,

    /// 连接超时
    Timeout,

    /// 主机密钥验证失败
    HostKeyRejected,

    /// 达到最大重连次数
    MaxRetriesExceeded,

    /// 其他原因
    Other(String),
}

impl std::fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserInitiated => write!(f, "User initiated"),
            Self::ServerClosed => write!(f, "Server closed connection"),
            Self::NetworkError => write!(f, "Network error"),
            Self::AuthFailed => write!(f, "Authentication failed"),
            Self::Timeout => write!(f, "Connection timeout"),
            Self::HostKeyRejected => write!(f, "Host key rejected"),
            Self::MaxRetriesExceeded => write!(f, "Max retries exceeded"),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

/// 连接事件
///
/// 触发状态转换的事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// 发起连接
    Connect,

    /// TCP 连接成功
    TcpConnected,

    ///认证成功
    AuthSuccess,

    /// 认证失败
    AuthFailed,

    /// 主动断开
    Disconnect,

    /// 连接丢失
    ConnectionLost,

    /// 重连成功
    ReconnectSuccess,

    /// 重连失败
    ReconnectFailed,

    /// 超时
    Timeout,
}

/// UI 连接状态（简化版）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiConnectionStatus {
    /// 离线
    Offline,
    /// 连接中
    Connecting,
    /// 在线
    Online,
    /// 重连中
    Reconnecting,
}

impl UiConnectionStatus {
    /// 获取状态图标
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Offline => "🔴",
            Self::Connecting => "🟡",
            Self::Online => "🟢",
            Self::Reconnecting => "🟠",
        }
    }

    /// 获取状态文本
    pub fn text(&self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Connecting => "Connecting",
            Self::Online => "Connected",
            Self::Reconnecting => "Reconnecting",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_default() {
        assert_eq!(ConnectionState::default(), ConnectionState::Idle);
    }

    #[test]
    fn test_connection_state_is_active() {
        assert!(!ConnectionState::Idle.is_active());
        assert!(!ConnectionState::Connecting { attempt: 1 }.is_active());
        assert!(
            ConnectionState::Connected {
                connected_at: Instant::now()
            }
            .is_active()
        );
    }

    #[test]
    fn test_connection_state_can_send() {
        assert!(!ConnectionState::Idle.can_send());
        assert!(
            ConnectionState::Connected {
                connected_at: Instant::now()
            }
            .can_send()
        );
    }

    #[test]
    fn test_ui_status_conversion() {
        assert_eq!(
            ConnectionState::Idle.to_ui_status(),
            UiConnectionStatus::Offline
        );
        assert_eq!(
            ConnectionState::Connecting { attempt: 1 }.to_ui_status(),
            UiConnectionStatus::Connecting
        );
        assert_eq!(
            ConnectionState::Connected {
                connected_at: Instant::now()
            }
            .to_ui_status(),
            UiConnectionStatus::Online
        );
    }

    #[test]
    fn test_disconnect_reason_display() {
        assert_eq!(
            DisconnectReason::UserInitiated.to_string(),
            "User initiated"
        );
        assert_eq!(DisconnectReason::Timeout.to_string(), "Connection timeout");
    }
}
