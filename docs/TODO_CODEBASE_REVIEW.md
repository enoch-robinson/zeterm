# Zeterm Crates 代码审查 - 问题清单与修复计划

> 审查日期：2025-01-22
> 审查范围：`zeterm/crates/` 目录下所有模块

---

## 目录

- [一、架构概述](#一架构概述)
- [二、已修复的问题](#二已修复的问题)
- [三、待修复的 Bug](#三待修复的-bug)
- [四、功能缺失](#四功能缺失)
- [五、设计改进建议](#五设计改进建议)
- [六、低优先级问题](#六低优先级问题)
- [七、修复计划](#七修复计划)

---

## 一、架构概述

### 模块依赖关系

```
┌─────────────────────────────────────────────────────────────┐
│                        zeterm (应用层)                       │
│  - app/session: 会话协调、连接管理                            │
│  - app/terminal: 终端状态机                                  │
│  - ui: GPUI 界面组件                                         │
└─────────────────────┬───────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  zeterm-ssh  │ │zeterm-storage│ │ zeterm-mock  │
│  SSH 连接实现 │ │ 数据持久化   │ │ 测试用 Mock  │
└──────┬───────┘ └──────┬───────┘ └──────┬───────┘
       │                │                │
       └────────────────┴────────────────┘
                        │
                        ▼
              ┌──────────────────┐
              │   zeterm-core    │
              │ 核心抽象、实体、  │
              │ 错误类型、状态机  │
              └──────────────────┘
```

---

## 二、已修复的问题

### ✅ 2.1 MockConnection 取消标志无法正确取消任务

- **文件**: `zeterm-mock/src/lib.rs`
- **问题**: `start_auto_output()` 创建了新的 `Arc<AtomicBool>`，与结构体字段 `cancel_auto_output` 无关联，导致 `close()` 无法取消自动输出任务
- **修复**: 将 `cancel_auto_output` 改为 `Arc<AtomicBool>` 类型，在任务中克隆使用
- **修复日期**: 2025-01-22

### ✅ 2.2 公钥编码使用不稳定的 Debug 格式

- **文件**: `zeterm-ssh/src/handler.rs`
- **问题**: 使用 `format!("{:?}", public_key)` 作为公钥标识，库升级时可能改变
- **修复**: 使用 `PublicKeyBase64` trait 的 `public_key_base64()` 方法和 `fingerprint()` 方法
- **修复日期**: 2025-01-22

---

## 三、待修复的 Bug

### 🔴 3.1 ConfigWatcher 变量混淆

- **优先级**: P0 - 阻塞
- **文件**: `zeterm-core/src/config/watcher.rs`
- **行号**: L185, L283-285
- **问题描述**:
  - 结构体定义了 `last_events` 字段用于防抖
  - 但 `start()` 方法创建了新的局部 `last_events` 变量
  - 导致结构体字段从未使用，每次重启 watcher 丢失防抖状态
- **影响**: 配置文件变更监听的防抖功能失效
- **修复方案**:
  ```rust
  // 修改前 (L283-285)
  let last_events = Arc::new(RwLock::new(HashMap::<PathBuf, Instant>::new()));
  
  // 修改后：使用结构体字段
  let last_events = Arc::new(self.last_events.clone()); // 或重新设计为 Arc
  ```

---

## 四、功能缺失

### 🟡 4.1 重连逻辑未集成

- **优先级**: P2 - 中
- **相关文件**:
  - `zeterm-ssh/src/reconnect.rs` (策略已实现)
  - `zeterm/src/app/session/session_coordinator.rs`
  - `zeterm-core/src/state/state_machine.rs`
- **问题描述**:
  - `ReconnectPolicy` 和 `ExponentialBackoff` 已实现
  - 但在 `SshConnection` 和 `SessionCoordinator` 中未使用
  - 状态机有 `Reconnecting` 状态但无触发逻辑
- **实现思路**:
  1. 创建 `ReconnectController` 组件
  2. 引入连接工厂模式（因为 `SshConnection` 是一次性使用的）
  3. 监听 `ConnectionLost` 事件触发重连
  4. 使用退避算法计算延迟
  5. 重连成功后重启数据泵

### 🟡 4.2 KeyboardInteractive 认证未实现

- **优先级**: P3 - 低
- **文件**: `zeterm-ssh/src/connection.rs` (L387-389)
- **问题描述**: 返回 "not yet implemented" 错误
- **影响**: 无法连接需要 2FA 的服务器
- **实现思路**:
  1. 在认证流程中调用 `session.authenticate_keyboard_interactive()`
  2. 定义 `KeyboardInteractiveCallback` 接口获取用户输入
  3. 处理多轮 prompt/response 交互

### 🟡 4.3 SFTP 传输控制功能未实现

- **优先级**: P2 - 中
- **文件**: `zeterm/src/ui/sftp/sftp_view.rs` (L387-402)
- **问题描述**: 暂停、恢复、取消、重试传输功能均为 TODO
- **实现思路**:
  1. 在 `TransferTask` 中实现暂停/恢复状态管理
  2. 利用已有的 `cancel_flag` 实现取消
  3. 重试需要记录原始参数并重新创建任务

### 🔴 4.4 SFTP 异步加载无法更新 UI

- **优先级**: P1 - 高
- **文件**: `zeterm/src/ui/sftp/sftp_view.rs` (L272-299)
- **问题描述**:
  - 异步加载目录后结果被丢弃
  - TODO 注释表明需要通过事件系统更新 UI
- **实现思路**:
  1. 使用 GPUI 的 `cx.spawn()` 或事件通道
  2. 将加载结果通过 `cx.emit()` 发送
  3. 在事件处理器中更新 `file_list`

### 🟡 4.5 系统主题检测未实现

- **优先级**: P3 - 低
- **文件**: `zeterm/src/ui/app_theme.rs` (L387-389)
- **问题描述**: `ThemeMode::System` 选项无效
- **实现思路**: 使用平台 API 检测系统深色/浅色模式

### 🟡 4.6 启动时清理僵尸连接记录

- **优先级**: P1 - 高
- **相关文件**:
  - `zeterm-storage/src/repository/connection_history.rs` (方法已实现)
  - `zeterm/src/main.rs` (需要调用)
- **问题描述**:
  - 应用崩溃后 `Connecting` 状态记录不会更新
  - `mark_all_active_as_disconnected()` 方法已存在但未在启动时调用
- **修复方案**: 在 `main.rs` 初始化数据库后调用该方法

### 🟢 4.7 状态机补充转换

- **优先级**: P3 - 低
- **文件**: `zeterm-core/src/state/state_machine.rs`
- **问题描述**: 缺少 `(Connecting, ConnectionLost) => Disconnected` 转换
- **影响**: 防御性编程，实际场景中不太可能触发
- **修复**: 在 `transition()` 方法中添加一行

---

## 五、设计改进建议

### 🟡 5.1 多处创建临时 Tokio Runtime

- **优先级**: P1 - 高
- **涉及文件**:
  - `zeterm/src/app/session/session_coordinator.rs` - `send_input_sync()`
  - `zeterm/src/ui/host_list/host_list_view.rs` - 多处
  - `zeterm/src/ui/sftp/sftp_view.rs` - `load_directory()`
- **问题描述**:
  - 每次异步操作都创建新的 Runtime
  - 资源开销大，可能导致线程爆炸
- **改进方案**:
  1. 创建应用级别的共享 Runtime
  2. 或使用 `tokio::runtime::Handle::current()` 复用
  3. 考虑使用 GPUI 的 `AsyncAppContext`

### 🟡 5.2 数据库未在启动时初始化

- **优先级**: P1 - 高
- **文件**: `zeterm/src/main.rs`
- **问题描述**:
  - 数据库延迟初始化
  - 无法在启动时执行清理操作
  - 首次数据库操作有延迟
- **改进方案**:
  ```rust
  // main.rs 中添加
  let db = Database::with_default_path().await?;
  db.init().await?;
  let history_repo = SqliteConnectionHistoryRepository::new(db.pool().clone());
  history_repo.mark_all_active_as_disconnected().await?;
  ```

### 🟢 5.3 删除主机缺少确认警告

- **优先级**: P3 - 低
- **文件**: `zeterm/src/ui/host_list/host_list_view.rs`
- **问题描述**: 删除主机会级联删除所有历史记录，但无警告
- **改进方案**: 添加确认对话框说明将删除关联历史

---

## 六、低优先级问题

### 6.1 终端单词选择功能简化

- **文件**: `zeterm/src/ui/terminal_view/mod.rs` (L589-595)
- **问题**: 双击选词不准确，需要从终端内容提取单词边界

### 6.2 SFTP 右键菜单未实现

- **文件**: `zeterm/src/ui/sftp/sftp_view.rs` (L362-365)

### 6.3 密码引用解析缺少错误处理

- **文件**: `zeterm-core/src/config/hosts_config.rs` (L152-167)
- **问题**: 无效格式静默转为 Keychain 类型

### 6.4 终端尺寸边界验证不完整

- **文件**: `zeterm-core/src/entities/mod.rs`
- **问题**: 缺少最大尺寸限制

### 6.5 未使用的 imports

- **文件**: `zeterm/src/ui/dialogs/mod.rs`
- **问题**: `CloseConfirmDialog` 等未使用

---

## 七、修复计划

### Phase 1: 紧急修复 (1-2 天)

| # | 任务 | 优先级 | 预估时间 | 状态 |
|---|------|--------|----------|------|
| 1.1 | 修复 ConfigWatcher 变量混淆 | P0 | 0.5h | ⬜ 待开始 |
| 1.2 | 数据库启动时初始化 | P1 | 1h | ⬜ 待开始 |
| 1.3 | 启动时清理僵尸连接记录 | P1 | 0.5h | ⬜ 待开始 |
| 1.4 | 共享 Tokio Runtime 设计 | P1 | 2h | ⬜ 待开始 |

### Phase 2: 核心功能完善 (3-5 天)

| # | 任务 | 优先级 | 预估时间 | 状态 |
|---|------|--------|----------|------|
| 2.1 | SFTP 异步加载更新 UI | P1 | 3h | ⬜ 待开始 |
| 2.2 | SFTP 传输控制实现 | P2 | 4h | ⬜ 待开始 |
| 2.3 | 重连逻辑集成 | P2 | 6h | ⬜ 待开始 |
| 2.4 | 状态机补充转换 | P3 | 0.5h | ⬜ 待开始 |

### Phase 3: 功能增强 (按需)

| # | 任务 | 优先级 | 预估时间 | 状态 |
|---|------|--------|----------|------|
| 3.1 | KeyboardInteractive 认证 | P3 | 4h | ⬜ 待开始 |
| 3.2 | 系统主题检测 | P3 | 2h | ⬜ 待开始 |
| 3.3 | 删除主机确认对话框 | P3 | 1h | ⬜ 待开始 |
| 3.4 | 终端单词选择优化 | P3 | 2h | ⬜ 待开始 |
| 3.5 | SFTP 右键菜单 | P3 | 3h | ⬜ 待开始 |

### Phase 4: 代码清理 (持续)

| # | 任务 | 优先级 | 预估时间 | 状态 |
|---|------|--------|----------|------|
| 4.1 | 清理未使用的 imports | P4 | 0.5h | ⬜ 待开始 |
| 4.2 | 密码引用解析错误处理 | P4 | 1h | ⬜ 待开始 |
| 4.3 | 终端尺寸边界验证 | P4 | 0.5h | ⬜ 待开始 |

---

## 附录

### A. 已验证为非问题的原始分析

以下原始分析经审查后确认为非问题或合理设计：

1. **KeepaliveManager RTT 计算重复** - 设计合理，两处计算目的不同
2. **HostConfig 与 HostEntry 重复** - 职责分离的 DTO 模式，合理设计
3. **锁混合使用死锁风险** - 代码已妥善处理，作用域不嵌套
4. **SFTP 进度回调问题** - 行为正常，每次循环都触发
5. **receive_stream 一次性消费** - 设计可接受，有文档说明

### B. 相关文件快速索引

| 模块 | 主要文件 |
|------|----------|
| 核心抽象 | `zeterm-core/src/traits/connection.rs` |
| 状态机 | `zeterm-core/src/state/state_machine.rs` |
| SSH 连接 | `zeterm-ssh/src/connection.rs` |
| SSH Handler | `zeterm-ssh/src/handler.rs` |
| 重连策略 | `zeterm-ssh/src/reconnect.rs` |
| 心跳保活 | `zeterm-ssh/src/keepalive.rs` |
| SFTP | `zeterm-ssh/src/sftp.rs` |
| 数据库 | `zeterm-storage/src/database.rs` |
| 连接历史 | `zeterm-storage/src/repository/connection_history.rs` |
| 配置监听 | `zeterm-core/src/config/watcher.rs` |
| 会话协调 | `zeterm/src/app/session/session_coordinator.rs` |
| SFTP UI | `zeterm/src/ui/sftp/sftp_view.rs` |

---

*文档最后更新: 2025-01-22*