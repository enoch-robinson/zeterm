# 连接状态机设计

> 定义 Zeterm 连接的生命周期状态与转换规则

---

## 一、状态定义

### 1.1 ConnectionState

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// 空闲状态
    Idle,
    /// 正在建立 TCP 连接
    Connecting { attempt: u32 },
    /// TCP 已连接，正在认证
    Authenticating,
    /// 认证成功，会话可用
    Connected { connected_at: Instant },
    /// 正在优雅关闭
    Disconnecting,
    /// 已断开
    Disconnected { reason: DisconnectReason },
    /// 自动重连中
    Reconnecting { attempt: u32 },
}
```

### 1.2 DisconnectReason

```rust
pub enum DisconnectReason {
    /// 用户主动断开
    UserInitiated,
    /// 服务器关闭
    ServerClosed,
    /// 网络错误
    NetworkError,
    /// 认证失败
    AuthFailed,
    /// 超时
    Timeout,
    /// 主机密钥验证失败
    HostKeyRejected,
}
```

---

## 二、状态转换规则

| 当前状态 | 事件 | 目标状态 | 副作用 |
|----------|------|----------|--------|
| `Idle` | `Connect` | `Connecting` | 启动 TCP 连接 |
| `Connecting` | `TcpConnected` | `Authenticating` | 开始认证 |
| `Connecting` | `Timeout/Error` | `Disconnected` | 记录错误 |
| `Authenticating` | `AuthSuccess` | `Connected` | 启动数据泵 |
| `Authenticating` | `AuthFailed` | `Disconnected` | 通知用户 |
| `Connected` | `Disconnect` | `Disconnecting` | 发送关闭信号 |
| `Connected` | `ConnectionLost` | `Reconnecting` | 启动重连 |
| `Disconnecting` | `Complete` | `Disconnected` | 清理资源 |
| `Reconnecting` | `Success` | `Connected` | 恢复会话 |
| `Reconnecting` | `MaxRetries` | `Disconnected` | 通知用户 |

---

## 三、核心实现

### 3.1 ConnectionStateMachine

```rust
pub struct ConnectionStateMachine {
    state: ConnectionState,
    state_tx: watch::Sender<ConnectionState>,
    reconnect_policy: ReconnectPolicy,
}

impl ConnectionStateMachine {
    pub fn new() -> Self;
    pub fn state(&self) -> &ConnectionState;
    pub fn subscribe(&self) -> watch::Receiver<ConnectionState>;
    pub fn handle_event(&mut self, event: ConnectionEvent) -> Option<StateTransition>;
}
```

### 3.2 重连策略

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `max_reconnect_attempts` | 3 | 最大重连次数 |
| `reconnect_initial_delay_ms` | 1000 | 初始延迟（毫秒） |
| `reconnect_max_delay_ms` | 30000 | 最大延迟（毫秒） |
| `reconnect_backoff_multiplier` | 2.0 | 退避倍数 |

退避算法：
```
delay = min(reconnect_initial_delay_ms * reconnect_backoff_multiplier^(attempt-1), 
            reconnect_max_delay_ms)
```

---

## 四、UI 状态映射

| 内部状态 | UI 状态 | 显示 |
|----------|---------|------|
| `Idle`, `Disconnected` | `Offline` | 🔴 |
| `Connecting`, `Authenticating` | `Connecting` | 🟡 |
| `Connected` | `Online` | 🟢 |
| `Reconnecting` | `Reconnecting` | 🟠 |

---

## 五、相关文档

- [API.md](../API.md) - 状态机 API 定义
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 架构总览
- [IMPLEMENTATION.md](../IMPLEMENTATION.md) - 实现指南