# 连接状态机设计

> 定义 Zeterm 连接的生命周期状态与转换规则

---

## 一、状态定义

### 1.1 状态枚举

```rust
/// 连接状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// 空闲状态 - 未建立连接
    Idle,
    /// 连接中 - 正在建立 TCP 连接
    Connecting {
        attempt: u32,
        started_at: Instant,
    },
    
    /// 认证中 - TCP 已连接，正在进行 SSH 握手/认证
    Authenticating {
        method: AuthMethod,
    },
    
    /// 已连接 - 认证成功，会话可用
    Connected {
        connected_at: Instant,
    },
    
    /// 断开中 - 正在优雅关闭连接
    Disconnecting,
    
    /// 已断开 - 连接已关闭
    Disconnected {
        reason: DisconnectReason,
    },
    /// 重连中 - 自动重连
    Reconnecting {
        attempt: u32,
        next_retry_at: Instant,
    },
}
```

### 1.2 辅助类型

```rust
/// 认证方式
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    PublicKey { key_path: String },
    KeyboardInteractive,Agent,
}

/// 断开原因
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisconnectReason {
    /// 用户主动断开
    UserInitiated,
    /// 服务器关闭连接
    ServerClosed,
    /// 网络错误
    NetworkError(String),
    /// 认证失败
    AuthFailed,
    /// 超时
    Timeout,
    /// 主机密钥验证失败
    HostKeyRejected,
}
```

---

## 二、状态转换图

```    ┌─────────────────────────────────────────┐
                    │                                         │
                    ▼                                         │
              ┌──────────┐                                    │
              │   Idle   │◄───────────────────────┐           │
              └────┬─────┘                        │           │
                   │                              │           │
                   │ connect()│           │
                   ▼                              │           │┌─────────────┐    timeout/error│           │
            │ Connecting  ├───────────────────────┤           │
            └──────┬──────┘                       │           │
                   │                 tcp_connected               │           │
                   ▼                              │           │
          ┌────────────────┐   auth_failed        │           │
          │ Authenticating ├──────────────────────┤           │
          └───────┬────────┘                      │           │
                  │                               │           │
                  │ auth_success                  │           │
                  ▼                               │           │
            ┌───────────┐     disconnect()┌────┴──────┐    │
            │ Connected ├───────────────────►│Disconnecting│   │
            └─────┬─────┘                    └─────┬──────┘    │
                  │                                │           │
                  │ connection_lost                │ done│
                  ▼                                ▼           │
          ┌──────────────┐                 ┌──────────────┐    │
          │ Reconnecting ├────────────────►│ Disconnected ├────┘
          └──────────────┘  max_retries    └──────────────┘│                               ▲
                  │ reconnect_success│
                  └───────────► Connected ────────┘
                (on error)
```

---

## 三、状态转换事件

### 3.1 事件定义

```rust
/// 状态转换事件
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// 用户发起连接
    Connect,
    /// TCP 连接建立成功
    TcpConnected,
    
    /// 认证成功
    AuthSuccess,
    
    /// 认证失败
    AuthFailed(AuthError),
    
    /// 连接超时
    Timeout,
    
    /// 网络错误
    NetworkError(String),
    
    /// 用户发起断开
    Disconnect,
    
    /// 连接丢失 (非用户主动)
    ConnectionLost,
    
    /// 重连成功
    ReconnectSuccess,
    
    /// 重连失败
    ReconnectFailed,
    
    /// 断开完成
    DisconnectComplete,
}
```

### 3.2 转换规则表

| 当前状态 | 事件 | 目标状态 | 副作用 |
|----------|------|----------|--------|
| `Idle` | `Connect` | `Connecting` | 启动 TCP 连接 |
| `Connecting` | `TcpConnected` | `Authenticating` | 开始认证流程 |
| `Connecting` | `Timeout` | `Disconnected` | 记录错误 |
| `Connecting` | `NetworkError` | `Disconnected` | 记录错误 |
| `Authenticating` | `AuthSuccess` | `Connected` | 启动数据泵 |
| `Authenticating` | `AuthFailed` | `Disconnected` | 通知用户 |
| `Connected` | `Disconnect` | `Disconnecting` | 发送关闭信号 |
| `Connected` | `ConnectionLost` | `Reconnecting` | 启动重连 |
| `Disconnecting` | `DisconnectComplete` | `Disconnected` | 清理资源 |
| `Reconnecting` | `ReconnectSuccess` | `Connected` | 恢复会话 |
| `Reconnecting` | `ReconnectFailed` | `Disconnected` | 通知用户 |
| `Disconnected` | `Connect` | `Connecting` | 重新连接 |

---

## 四、状态机实现

### 4.1 核心结构

```rust
use tokio::sync::watch;

pub struct ConnectionStateMachine {
    /// 当前状态
    state: ConnectionState,
    
    /// 状态变更通知
    state_tx: watch::Sender<ConnectionState>,
    state_rx: watch::Receiver<ConnectionState>,
    
    /// 重连策略
    reconnect_policy: ReconnectPolicy,
}

impl ConnectionStateMachine {
    pub fn new() -> Self {
        let (state_tx, state_rx) = watch::channel(ConnectionState::Idle);
        Self {
            state: ConnectionState::Idle,
            state_tx,
            state_rx,
            reconnect_policy: ReconnectPolicy::default(),
        }
    }
    
    /// 订阅状态变更
    pub fn subscribe(&self) -> watch::Receiver<ConnectionState> {
        self.state_rx.clone()
    }
    
    /// 获取当前状态
    pub fn current_state(&self) -> &ConnectionState {
        &self.state
    }
}
```

### 4.2 状态转换方法

```rust
impl ConnectionStateMachine {
    /// 处理事件，返回是否发生状态转换
    pub fn handle_event(&mut self, event: ConnectionEvent) -> bool {
        let new_state = self.next_state(&event);
        
        if let Some(state) = new_state {
            let old_state = std::mem::replace(&mut self.state, state.clone());
            
            // 通知订阅者
            let _ = self.state_tx.send(state);
            
            tracing::info!(
                old = ?old_state,
                new = ?self.state,
                event = ?event,
                "状态转换"
            );
            
            true
        } else {
            tracing::warn!(
                state = ?self.state,
                event = ?event,
                "无效的状态转换"
            );
            false
        }
    }
    
    fn next_state(&self, event: &ConnectionEvent) -> Option<ConnectionState> {
        use ConnectionState::*;
        use ConnectionEvent::*;
        
        match (&self.state, event) {
            (Idle, Connect) => Some(Connecting {
                attempt: 1,
                started_at: Instant::now(),
            }),
            (Connecting { .. }, TcpConnected) => Some(Authenticating {
                method: AuthMethod::Password, // 由外部设置
            }),
            
            (Connecting { .. }, Timeout) => Some(Disconnected {
                reason: DisconnectReason::Timeout,
            }),
            
            (Authenticating { .. }, AuthSuccess) => Some(Connected {
                connected_at: Instant::now(),
            }),
            
            (Authenticating { .. }, AuthFailed(err)) => Some(Disconnected {
                reason: DisconnectReason::AuthFailed,
            }),
            
            (Connected { .. }, Disconnect) => Some(Disconnecting),
            
            (Connected { .. }, ConnectionLost) => {
                if self.reconnect_policy.enabled {
                    Some(Reconnecting {
                        attempt: 1,
                        next_retry_at: Instant::now() + self.reconnect_policy.initial_delay,
                    })
                } else {
                    Some(Disconnected {
                        reason: DisconnectReason::NetworkError("连接丢失".into()),
                    })
                }
            }
            
            (Disconnecting, DisconnectComplete) => Some(Disconnected {
                reason: DisconnectReason::UserInitiated,
            }),
            
            (Reconnecting { attempt, .. }, ReconnectFailed) => {
                if *attempt < self.reconnect_policy.max_attempts {
                    let delay = self.reconnect_policy.delay_for_attempt(*attempt + 1);
                    Some(Reconnecting {
                        attempt: attempt + 1,
                        next_retry_at: Instant::now() + delay,
                    })
                } else {
                    Some(Disconnected {
                        reason: DisconnectReason::NetworkError("重连失败".into()),
                    })
                }
            }
            
            (Reconnecting { .. }, ReconnectSuccess) => Some(Connected {
                connected_at: Instant::now(),
            }),
            
            (Disconnected { .. }, Connect) => Some(Connecting {
                attempt: 1,
                started_at: Instant::now(),
            }),
            
            _ => None,
        }
    }
}
```

---

## 五、重连策略

```rust
use std::time::Duration;

/// 重连策略配置
pub struct ReconnectPolicy {
    /// 是否启用自动重连
    pub enabled: bool,
    
    /// 最大重试次数
    pub max_attempts: u32,
    
    /// 初始延迟
    pub initial_delay: Duration,
    
    /// 最大延迟
    pub max_delay: Duration,
    
    /// 退避倍数
    pub backoff_factor: f64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
        }
    }
}

impl ReconnectPolicy {
    /// 计算第n 次重试的延迟
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_secs = self.initial_delay.as_secs_f64() 
            * self.backoff_factor.powi(attempt as i32 - 1);
        
        Duration::from_secs_f64(delay_secs.min(self.max_delay.as_secs_f64()))
    }
}
```

---

## 六、UI 状态映射

```rust
/// UI 显示的简化状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Offline,
    Connecting,
    Online,
    Reconnecting,
}

impl From<&ConnectionState> for ConnectionStatus {
    fn from(state: &ConnectionState) -> Self {
        match state {
            ConnectionState::Idle => Self::Offline,
            ConnectionState::Connecting { .. } => Self::Connecting,
            ConnectionState::Authenticating { .. } => Self::Connecting,
            ConnectionState::Connected { .. } => Self::Online,
            ConnectionState::Disconnecting => Self::Offline,
            ConnectionState::Disconnected { .. } => Self::Offline,
            ConnectionState::Reconnecting { .. } => Self::Reconnecting,
        }
    }
}
```

---

## 七、相关文档

- [错误处理策略](./error-handling.md) - 状态转换触发的错误
- [SessionModel](../modules/session-model.md) - 状态机的使用者
- [SSH 后端](../modules/ssh-backend.md) - 产生状态事件的来源