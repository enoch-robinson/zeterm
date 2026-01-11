# 连接状态机设计

> 定义 Zeterm 连接的生命周期状态与转换规则

---

## 一、状态定义

### 1.1 状态枚举

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,                    // 空闲，未连接
    Connecting { attempt: u32 },             // 正在建立 TCP 连接
    Authenticating,// TCP 已连接，正在认证
    Connected { connected_at: Instant },     // 认证成功，会话可用
    Disconnecting,                           // 正在优雅关闭
    Disconnected { reason: DisconnectReason }, // 已断开Reconnecting { attempt: u32 },           // 自动重连中
}
```

### 1.2 断开原因

```rust
pub enum DisconnectReason {
    UserInitiated,      // 用户主动断开
    ServerClosed,       // 服务器关闭
    NetworkError,       // 网络错误
    AuthFailed,         // 认证失败
    Timeout,            // 超时
    HostKeyRejected,    // 主机密钥验证失败
}
```

---

## 二、状态转换图

```┌──────────┐
              │   Idle   │◄───────────────────────┐
              └────┬─────┘                        │
                   │ connect()│
                   ▼                              │
            ┌─────────────┐    timeout/error      │
            │ Connecting  ├───────────────────────┤
            └──────┬──────┘                       │
                   │ tcp_connected                │
                   ▼                              │
          ┌────────────────┐   auth_failed        │
          │ Authenticating├──────────────────────┤
          └───────┬────────┘                      ││ auth_success                  │
                  ▼                │
            ┌───────────┐     disconnect()  ┌─────┴──────┐
            │ Connected ├──────────────────►│Disconnecting│
            └─────┬─────┘                   └─────┬──────┘
                  │ connection_lost│ done
                  ▼                               ┌──────────────┐                 ┌──────────────┐
          │ Reconnecting ├────────────────►│ Disconnected │
          └──────────────┘  max_retries    └──────────────┘
```

---

## 三、状态转换规则

| 当前状态 | 事件 | 目标状态 | 副作用 |
|----------|------|----------|--------|
| `Idle` | `Connect` | `Connecting` | 启动 TCP 连接 |
| `Connecting` | `TcpConnected` | `Authenticating` | 开始认证 |
| `Connecting` | `Timeout/Error` | `Disconnected` | 记录错误 |
| `Authenticating` | `AuthSuccess` | `Connected` | 启动数据泵 |
| `Authenticating` | `AuthFailed` | `Disconnected` | 通知用户 |
| `Connected` | `Disconnect` | `Disconnecting` | 发送关闭信号 |
| `Connected` | `ConnectionLost` | `Reconnecting` | 启动重连 (如启用) |
| `Disconnecting` | `Complete` | `Disconnected` | 清理资源 |
| `Reconnecting` | `Success` | `Connected` | 恢复会话 |
| `Reconnecting` | `MaxRetries` | `Disconnected` | 通知用户 |
| `Disconnected` | `Connect` | `Connecting` | 重新连接 |

---

## 四、核心实现要点

### 4.1 状态机结构

```rust
pub struct ConnectionStateMachine {
    state: ConnectionState,
    state_tx: watch::Sender<ConnectionState>,  // 状态广播
    reconnect_policy: ReconnectPolicy,
}
```

### 4.2 关键方法

| 方法 | 说明 |
|------|------|
| `handle_event(event)` | 处理事件，执行状态转换 |
| `subscribe()` | 订阅状态变更通知 |
| `current_state()` | 获取当前状态 |

### 4.3 实现要点

1. **状态广播**: 使用 `watch::channel` 通知订阅者
2. **无效转换**: 记录 WARN 日志，不执行转换
3. **副作用分离**: 状态机只负责状态转换，副作用由调用方执行

---

## 五、重连策略

### 5.1 配置参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `enabled` | true | 是否启用自动重连 |
| `max_attempts` | 5 | 最大重试次数 |
| `initial_delay` | 1s | 初始延迟 |
| `max_delay` | 60s | 最大延迟 |
| `backoff_factor` | 2.0 | 退避倍数 |

### 5.2 退避算法

```
delay = min(initial_delay * backoff_factor^(attempt-1), max_delay)
```

---

## 六、UI 状态映射

将内部状态映射为简化的 UI 显示状态：

| 内部状态 | UI 状态 | 显示 |
|----------|---------|------|
| `Idle`, `Disconnected`, `Disconnecting` | `Offline` | 🔴 |
| `Connecting`, `Authenticating` | `Connecting` | 🟡 |
| `Connected` | `Online` | 🟢 |
| `Reconnecting` | `Reconnecting` | 🟠 |

---

## 七、相关文档

- [错误处理策略](./error-handling.md) - 状态转换触发的错误
- [SessionModel](../modules/session-model.md) - 状态机的使用者
- [SSH 后端](../modules/ssh-backend.md) - 产生状态事件的来源