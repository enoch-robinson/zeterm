//! 连接状态机模块
//!
//! 实现连接状态的管理和转换逻辑。

use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use super::{ConnectionEvent, ConnectionState, DisconnectReason, UiConnectionStatus};

/// 状态变更回调类型
pub type StateChangeCallback = Box<dyn Fn(&ConnectionState, &ConnectionState) + Send + Sync>;

/// 连接状态机
///
/// 管理连接的状态转换，确保状态转换的正确性和一致性。
pub struct ConnectionStateMachine {
    /// 当前状态
    state: Arc<RwLock<ConnectionState>>,
    /// 状态广播发送端
    state_tx: watch::Sender<ConnectionState>,
    /// 状态广播接收端（用于克隆给订阅者）
    state_rx: watch::Receiver<ConnectionState>,
    /// 最大重连次数
    max_reconnect_attempts: u32,
    /// 状态变更回调
    on_state_change: Option<StateChangeCallback>,
}

impl ConnectionStateMachine {
    /// 创建新的状态机
    pub fn new() -> Self {
        let initial_state = ConnectionState::Idle;
        let (state_tx, state_rx) = watch::channel(initial_state.clone());

        Self {
            state: Arc::new(RwLock::new(initial_state)),
            state_tx,
            state_rx,
            max_reconnect_attempts: 3,
            on_state_change: None,
        }
    }

    /// 设置最大重连次数
    pub fn with_max_reconnect_attempts(mut self, max: u32) -> Self {
        self.max_reconnect_attempts = max;
        self
    }

    /// 设置状态变更回调
    pub fn with_state_change_callback(mut self, callback: StateChangeCallback) -> Self {
        self.on_state_change = Some(callback);
        self
    }

    /// 获取当前状态
    pub fn current_state(&self) -> ConnectionState {
        self.state.read().clone()
    }

    /// 获取当前 UI 状态
    pub fn ui_status(&self) -> UiConnectionStatus {
        self.state.read().to_ui_status()
    }

    /// 订阅状态变更
    ///
    /// 返回一个 watch::Receiver，可以用于监听状态变更。
    pub fn subscribe(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }

    /// 处理事件，执行状态转换
    ///
    /// 返回转换后的新状态，如果转换无效则返回 None。
    pub fn handle_event(&self, event: ConnectionEvent) -> Option<ConnectionState> {
        let current = self.state.read().clone();
        let new_state = self.transition(&current, &event);

        if let Some(ref new) = new_state {
            if new != &current {
                // 更新状态
                *self.state.write() = new.clone();

                // 广播状态变更
                if let Err(e) = self.state_tx.send(new.clone()) {
                    warn!("Failed to broadcast state change: {}", e);
                }

                // 调用回调
                if let Some(ref callback) = self.on_state_change {
                    callback(&current, new);
                }

                info!(
                    "State transition: {:?} --[{:?}]--> {:?}",
                    current, event, new
                );
            }
        } else {
            debug!(
                "Invalid state transition: {:?} --[{:?}]--> (rejected)",
                current, event
            );
        }

        new_state
    }

    /// 状态转换规则
    ///
    /// 根据当前状态和事件，返回新状态。
    /// 如果转换无效，返回 None。
    fn transition(
        &self,
        current: &ConnectionState,
        event: &ConnectionEvent,
    ) -> Option<ConnectionState> {
        match (current, event) {
            // === 从Idle 状态的转换 ===
            (ConnectionState::Idle, ConnectionEvent::Connect) => {
                Some(ConnectionState::Connecting { attempt: 1 })
            },

            // === 从 Connecting 状态的转换 ===
            (ConnectionState::Connecting { .. }, ConnectionEvent::TcpConnected) => {
                Some(ConnectionState::Authenticating)
            },
            (ConnectionState::Connecting { .. }, ConnectionEvent::Timeout) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::Timeout,
                })
            },
            (ConnectionState::Connecting { .. }, ConnectionEvent::Disconnect) => {
                Some(ConnectionState::Idle)
            },
            // 连接过程中连接丢失（如网络问题），转为断开状态
            (ConnectionState::Connecting { .. }, ConnectionEvent::ConnectionLost) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::NetworkError,
                })
            },

            // === 从 Authenticating 状态的转换 ===
            (ConnectionState::Authenticating, ConnectionEvent::AuthSuccess) => {
                Some(ConnectionState::Connected {
                    connected_at: Instant::now(),
                })
            },
            (ConnectionState::Authenticating, ConnectionEvent::AuthFailed) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::AuthFailed,
                })
            },
            (ConnectionState::Authenticating, ConnectionEvent::Timeout) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::Timeout,
                })
            },
            (ConnectionState::Authenticating, ConnectionEvent::Disconnect) => {
                Some(ConnectionState::Disconnecting)
            },

            // === 从 Connected 状态的转换 ===
            (ConnectionState::Connected { .. }, ConnectionEvent::Disconnect) => {
                Some(ConnectionState::Disconnecting)
            },
            (ConnectionState::Connected { .. }, ConnectionEvent::ConnectionLost) => {
                Some(ConnectionState::Reconnecting { attempt: 1 })
            },
            (ConnectionState::Connected { .. }, ConnectionEvent::Timeout) => {
                Some(ConnectionState::Reconnecting { attempt: 1 })
            },

            // === 从 Disconnecting 状态的转换 ===
            (ConnectionState::Disconnecting, ConnectionEvent::Disconnect) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::UserInitiated,
                })
            },
            //断开过程中连接丢失，视为正常断开
            (ConnectionState::Disconnecting, ConnectionEvent::ConnectionLost) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::UserInitiated,
                })
            },

            // === 从 Disconnected 状态的转换 ===
            (ConnectionState::Disconnected { .. }, ConnectionEvent::Connect) => {
                Some(ConnectionState::Connecting { attempt: 1 })
            },

            // === 从 Reconnecting 状态的转换 ===
            (ConnectionState::Reconnecting { .. }, ConnectionEvent::TcpConnected) => {
                Some(ConnectionState::Authenticating)
            },
            (ConnectionState::Reconnecting { .. }, ConnectionEvent::ReconnectSuccess) => {
                Some(ConnectionState::Connected {
                    connected_at: Instant::now(),
                })
            },
            (ConnectionState::Reconnecting { attempt }, ConnectionEvent::ReconnectFailed) => {
                if *attempt >= self.max_reconnect_attempts {
                    Some(ConnectionState::Disconnected {
                        reason: DisconnectReason::MaxRetriesExceeded,
                    })
                } else {
                    Some(ConnectionState::Reconnecting {
                        attempt: attempt + 1,
                    })
                }
            },
            (ConnectionState::Reconnecting { .. }, ConnectionEvent::Disconnect) => {
                Some(ConnectionState::Disconnected {
                    reason: DisconnectReason::UserInitiated,
                })
            },
            (ConnectionState::Reconnecting { attempt }, ConnectionEvent::Timeout) => {
                if *attempt >= self.max_reconnect_attempts {
                    Some(ConnectionState::Disconnected {
                        reason: DisconnectReason::MaxRetriesExceeded,
                    })
                } else {
                    Some(ConnectionState::Reconnecting {
                        attempt: attempt + 1,
                    })
                }
            },

            // === 无效转换 ===
            _ => None,
        }
    }

    /// 强制设置状态（用于恢复或测试）
    ///
    /// 注意：这会绕过状态转换规则，应谨慎使用。
    pub fn force_state(&self, state: ConnectionState) {
        let old_state = self.state.read().clone();
        *self.state.write() = state.clone();

        if let Err(e) = self.state_tx.send(state.clone()) {
            warn!("Failed to broadcast forced state change: {}", e);
        }

        if let Some(ref callback) = self.on_state_change {
            callback(&old_state, &state);
        }

        warn!("State forcefully set to: {:?}", state);
    }

    /// 重置状态机到初始状态
    pub fn reset(&self) {
        self.force_state(ConnectionState::Idle);
        info!("State machine reset to Idle");
    }

    /// 检查是否可以发起连接
    pub fn can_connect(&self) -> bool {
        let state = self.state.read();
        matches!(
            *state,
            ConnectionState::Idle | ConnectionState::Disconnected { .. }
        )
    }

    /// 检查是否可以断开连接
    pub fn can_disconnect(&self) -> bool {
        let state = self.state.read();
        matches!(
            *state,
            ConnectionState::Connecting { .. }
                | ConnectionState::Authenticating
                | ConnectionState::Connected { .. }
                | ConnectionState::Reconnecting { .. }
        )
    }

    /// 检查是否处于活跃连接状态
    pub fn is_connected(&self) -> bool {
        self.state.read().is_active()
    }

    /// 检查是否正在连接中
    pub fn is_connecting(&self) -> bool {
        self.state.read().is_connecting()
    }
}

impl Default for ConnectionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ConnectionStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionStateMachine")
            .field("state", &*self.state.read())
            .field("max_reconnect_attempts", &self.max_reconnect_attempts)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let sm = ConnectionStateMachine::new();
        assert_eq!(sm.current_state(), ConnectionState::Idle);
        assert!(sm.can_connect());
        assert!(!sm.can_disconnect());
    }

    #[test]
    fn test_connect_flow() {
        let sm = ConnectionStateMachine::new();

        // Idle -> Connecting
        let state = sm.handle_event(ConnectionEvent::Connect);
        assert!(matches!(
            state,
            Some(ConnectionState::Connecting { attempt: 1 })
        ));

        // Connecting -> Authenticating
        let state = sm.handle_event(ConnectionEvent::TcpConnected);
        assert!(matches!(state, Some(ConnectionState::Authenticating)));

        // Authenticating -> Connected
        let state = sm.handle_event(ConnectionEvent::AuthSuccess);
        assert!(matches!(state, Some(ConnectionState::Connected { .. })));
        assert!(sm.is_connected());
    }

    #[test]
    fn test_disconnect_flow() {
        let sm = ConnectionStateMachine::new();

        // 先连接
        sm.handle_event(ConnectionEvent::Connect);
        sm.handle_event(ConnectionEvent::TcpConnected);
        sm.handle_event(ConnectionEvent::AuthSuccess);

        // Connected -> Disconnecting
        let state = sm.handle_event(ConnectionEvent::Disconnect);
        assert!(matches!(state, Some(ConnectionState::Disconnecting)));

        // Disconnecting -> Disconnected
        let state = sm.handle_event(ConnectionEvent::Disconnect);
        assert!(matches!(
            state,
            Some(ConnectionState::Disconnected {
                reason: DisconnectReason::UserInitiated
            })
        ));
    }

    #[test]
    fn test_auth_failure() {
        let sm = ConnectionStateMachine::new();

        sm.handle_event(ConnectionEvent::Connect);
        sm.handle_event(ConnectionEvent::TcpConnected);

        // Authenticating -> Disconnected (AuthFailed)
        let state = sm.handle_event(ConnectionEvent::AuthFailed);
        assert!(matches!(
            state,
            Some(ConnectionState::Disconnected {
                reason: DisconnectReason::AuthFailed
            })
        ));
    }

    #[test]
    fn test_reconnect_flow() {
        let sm = ConnectionStateMachine::new().with_max_reconnect_attempts(3);

        // 先连接
        sm.handle_event(ConnectionEvent::Connect);
        sm.handle_event(ConnectionEvent::TcpConnected);
        sm.handle_event(ConnectionEvent::AuthSuccess);

        // Connected -> Reconnecting (连接丢失)
        let state = sm.handle_event(ConnectionEvent::ConnectionLost);
        assert!(matches!(
            state,
            Some(ConnectionState::Reconnecting { attempt: 1 })
        ));

        // 重连失败，尝试次数增加
        let state = sm.handle_event(ConnectionEvent::ReconnectFailed);
        assert!(matches!(
            state,
            Some(ConnectionState::Reconnecting { attempt: 2 })
        ));

        // 再次失败
        let state = sm.handle_event(ConnectionEvent::ReconnectFailed);
        assert!(matches!(
            state,
            Some(ConnectionState::Reconnecting { attempt: 3 })
        ));

        // 达到最大重连次数
        let state = sm.handle_event(ConnectionEvent::ReconnectFailed);
        assert!(matches!(
            state,
            Some(ConnectionState::Disconnected {
                reason: DisconnectReason::MaxRetriesExceeded
            })
        ));
    }

    #[test]
    fn test_reconnect_success() {
        let sm = ConnectionStateMachine::new();

        // 先连接
        sm.handle_event(ConnectionEvent::Connect);
        sm.handle_event(ConnectionEvent::TcpConnected);
        sm.handle_event(ConnectionEvent::AuthSuccess);

        // 连接丢失
        sm.handle_event(ConnectionEvent::ConnectionLost);

        // 重连成功
        let state = sm.handle_event(ConnectionEvent::ReconnectSuccess);
        assert!(matches!(state, Some(ConnectionState::Connected { .. })));
    }

    #[test]
    fn test_invalid_transition() {
        let sm = ConnectionStateMachine::new();

        // 从 Idle 状态不能直接 AuthSuccess
        let state = sm.handle_event(ConnectionEvent::AuthSuccess);
        assert!(state.is_none());

        // 状态应该保持不变
        assert_eq!(sm.current_state(), ConnectionState::Idle);
    }

    #[test]
    fn test_timeout_during_connect() {
        let sm = ConnectionStateMachine::new();

        sm.handle_event(ConnectionEvent::Connect);

        // Connecting -> Disconnected (Timeout)
        let state = sm.handle_event(ConnectionEvent::Timeout);
        assert!(matches!(
            state,
            Some(ConnectionState::Disconnected {
                reason: DisconnectReason::Timeout
            })
        ));
    }

    #[test]
    fn test_subscribe() {
        let sm = ConnectionStateMachine::new();
        let mut rx = sm.subscribe();

        // 初始状态
        assert_eq!(*rx.borrow(), ConnectionState::Idle);

        // 触发状态变更
        sm.handle_event(ConnectionEvent::Connect);

        // 检查是否收到更新
        assert!(rx.has_changed().unwrap());
        assert!(matches!(
            *rx.borrow_and_update(),
            ConnectionState::Connecting { .. }
        ));
    }

    #[test]
    fn test_force_state() {
        let sm = ConnectionStateMachine::new();

        sm.force_state(ConnectionState::Connected {
            connected_at: Instant::now(),
        });

        assert!(sm.is_connected());
    }

    #[test]
    fn test_reset() {
        let sm = ConnectionStateMachine::new();

        // 先进入某个状态
        sm.handle_event(ConnectionEvent::Connect);
        sm.handle_event(ConnectionEvent::TcpConnected);

        // 重置
        sm.reset();

        assert_eq!(sm.current_state(), ConnectionState::Idle);
    }

    #[test]
    fn test_ui_status() {
        let sm = ConnectionStateMachine::new();

        assert_eq!(sm.ui_status(), UiConnectionStatus::Offline);

        sm.handle_event(ConnectionEvent::Connect);
        assert_eq!(sm.ui_status(), UiConnectionStatus::Connecting);

        sm.handle_event(ConnectionEvent::TcpConnected);
        sm.handle_event(ConnectionEvent::AuthSuccess);
        assert_eq!(sm.ui_status(), UiConnectionStatus::Online);
    }

    #[test]
    fn test_connection_lost_during_connecting() {
        let sm = ConnectionStateMachine::new();

        // 开始连接
        sm.handle_event(ConnectionEvent::Connect);
        assert!(matches!(
            sm.current_state(),
            ConnectionState::Connecting { .. }
        ));

        // 连接过程中连接丢失
        sm.handle_event(ConnectionEvent::ConnectionLost);

        // 应该转为 Disconnected 状态，原因是 NetworkError
        assert!(matches!(
            sm.current_state(),
            ConnectionState::Disconnected {
                reason: DisconnectReason::NetworkError
            }
        ));
    }
}
