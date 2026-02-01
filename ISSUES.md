# Zeterm 代码问题跟踪文档

> 生成时间: 2026-01-29 11:32:28 CST
> 基于代码版本: zeterm/main

---

## 问题概览

| 优先级 | 数量 | 状态 |
|--------|------|------|
| 🔴 高 | 3 | 待修复 |
| 🟡 中 | 7 | 待修复 |
| 🟢 低 | 14 | 优化项 |
| **总计** | **24** | - |

---

## 🔴 高优先级问题

### #1: Render 方法中修改状态（严重）
**位置**: `zeterm/crates/zeterm/src/ui/main_window.rs` L1102-L1107

**问题描述**:
`Render` trait 的 `render` 方法中调用了 `create_local_tab` 和 `sync_status_bar_from_coordinator`，这两个方法都会修改状态。这违反了 GPUI 的设计原则，`render` 方法应该是纯函数，只负责渲染而不修改状态。

```rust
impl Render for MainWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ❌ 错误：在 render 中修改状态
        if self.new_tab_requested.swap(false, Ordering::SeqCst) {
            if let Some(tab_id) = self.create_local_tab("新终端", cx) { ... }
        }
        self.sync_status_bar_from_coordinator(cx);  // 修改状态！
    }
}
```

**影响**: 可能导致无限重渲染循环、UI 状态不一致或其他未定义行为。

**修复建议**:
- 将状态修改逻辑移到事件处理器中
- 使用 `cx.observe` 或订阅模式来响应状态变化
- 在 render 之前的状态变更点（如用户交互回调）处理这些逻辑

---

### #2: 连接成功后状态栏未更新
**位置**: `zeterm/crates/zeterm/src/ui/connection_manager.rs` L43-L87

**问题描述**:
异步连接任务中，连接成功或失败后没有更新状态栏状态。用户点击连接后，状态栏一直显示 "Connecting..."，即使连接已经建立或失败。

```rust
runtime::spawn(async move {
    match Self::do_ssh_connect(&host_clone, coordinator_clone.clone()).await {
        Ok(stream) => {
            info!("SSH 连接成功");
            coordinator_clone.start_data_pump(stream, ...).await;
            // ❌ 缺少：更新状态栏为 Connected
        }
        Err(e) => {
            error!("SSH 连接失败: {}", e);
            // ❌ 缺少：更新状态栏为 Error
        }
    }
});
```

**影响**: 用户无法从 UI 得知连接的真实状态。

**修复建议**:
- 在异步任务中通过 Entity 的 `update` 方法更新状态栏
- 连接成功后更新为 `ConnectionStatus::Connected`
- 连接失败后更新为 `ConnectionStatus::Error` 并显示错误信息

---

### #3: Tab 关闭时 SSH 连接未释放
**位置**: `zeterm/crates/zeterm/src/ui/main_window.rs` L277-L311

**问题描述**:
`cleanup_tab` 方法移除了 UI 相关的 SplitView 和 TerminalView，但没有关闭底层的 SSH 连接。

```rust
fn cleanup_tab(&mut self, tab_id: TabId, cx: &mut Context<Self>) {
    // 移除 SplitView
    if let Some(_split_view) = self.split_views.remove(&tab_id) {
        ...
    }
    // ❌ 缺少：关闭 SSH 连接
}
```

**影响**: 连接泄漏，可能导致服务器端的僵尸连接或本地资源耗尽。

**修复建议**:
- 在 `TerminalPaneData` 中保存 `SessionCoordinator` 的引用
- 关闭 Tab 时调用 `coordinator.close().await`
- 确保数据泵任务正确终止

---

## 🟡 中优先级问题

### #4: 全局数据库 Panic 风险
**位置**: `zeterm/crates/zeterm/src/app/mod.rs` L50-L58

**问题描述**:
`global_database()` 在未初始化时直接 panic，而不是返回错误。

```rust
pub fn global_database() -> Arc<Database> {
    GLOBAL_DATABASE
        .get()
        .expect("Database not initialized...")  // ❌ 直接 panic
        .clone()
}
```

**影响**: 如果数据库初始化失败，程序崩溃而不是优雅降级。

**修复建议**:
- 返回 `Option<Arc<Database>>` 或 `Result<Arc<Database>, DbError>`
- 调用者负责处理未初始化的情况

---

### #5: 连接错误没有 UI 反馈
**位置**: `zeterm/crates/zeterm/src/ui/connection_manager.rs` L83-L86

**问题描述**:
SSH 连接失败只在日志中记录错误，用户界面上没有任何反馈。

**影响**: 用户不知道连接是否成功，无法得知失败原因。

**修复建议**:
- 显示错误提示对话框
- 或在终端面板中显示错误信息
- 状态栏显示错误状态并提供详细信息

---

### #6: TextInput 组件缺少光标显示
**位置**: `zeterm/crates/zeterm/src/ui/components/text_input.rs` L217-L237

**问题描述**:
虽然实现了光标位置管理，但渲染时没有显示光标。

```rust
fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    ...
    div()
        .child(display_text)  // ❌ 只显示文本，没有光标
}
```

**影响**: 用户无法看到输入焦点位置。

**修复建议**:
- 在文本中插入光标指示器
- 或使用绝对定位叠加光标元素
- 支持光标闪烁动画

---

### #7: 数据泵通知的线程安全问题
**位置**: `zeterm/crates/zeterm/src/app/session/session_coordinator.rs` L276-L280

**问题描述**:
`notify_callback` 在异步线程中被调用，如果回调直接操作 GPUI UI，可能导致线程安全问题。

```rust
notify_callback: F,
) where F: Fn() + Send + Sync + 'static

notify_callback();  // 在异步线程中调用
```

**影响**: 可能导致数据竞争或崩溃。

**修复建议**:
- 使用 GPUI 的 `cx.emit` 发送事件
- 或通过 GPUI 的线程安全机制调度到 UI 线程

---

### #8: 数据冗余导致不一致风险
**位置**: `zeterm/crates/zeterm/src/ui/main_window.rs` L49-L55

**问题描述**:
`terminal_panes` 同时在 `MainWindow` 和 `TerminalPaneManager` 中存储。

```rust
// MainWindow
terminal_panes: HashMap<PaneId, TerminalPaneData>,

// TerminalPaneManager 中也有
terminal_panes: HashMap<PaneId, TerminalPaneData>,
```

**影响**: 可能导致数据不一致，一处更新另一处未同步。

**修复建议**:
- 只在一处维护数据
- 或者使用 Rc/Arc 共享同一份数据

---

### #9: Arc<Mutex<>> 在 GPUI 中的不当使用
**位置**: `zeterm/crates/zeterm/src/ui/dialogs/host_connection_dialog.rs` L275-L292

**问题描述**:
使用 `Arc<Mutex<>>` 存储表单数据，这与 GPUI 的响应式系统不兼容。

```rust
pub struct HostConnectionDialog {
    form_data: Arc<Mutex<FormData>>,
    validation_error: Arc<Mutex<Option<String>>>,
}
```

**影响**: 可能导致死锁、状态变更不触发重渲染。

**修复建议**:
- 使用 GPUI 的 `Model` 或 `Context` 状态管理
- 直接存储在 struct 字段中，通过 `&mut self` 修改

---

### #10: 终端光标闪烁未实现
**位置**: `zeterm/crates/zeterm/src/ui/terminal_view/mod.rs` L215-L220

**问题描述**:
虽然有 `cursor_visible` 和 `cursor_blink_enabled` 字段，但没有定时器实现闪烁。

**影响**: 光标不闪烁，用户可能难以定位光标。

**修复建议**:
- 使用 `cx.spawn` 创建定时任务
- 定时切换 `cursor_visible` 状态并调用 `cx.notify()`

---

## 🟢 低优先级问题（优化项）

### #11: Runtime 双重初始化配置不一致
**位置**: `zeterm/crates/zeterm/src/app/runtime.rs` L130-L140

**问题**: 自动初始化使用 `worker_threads(2)`，与默认配置不同。

---

### #12: SSH 配置终端大小硬编码
**位置**: `zeterm/crates/zeterm/src/ui/connection_manager.rs` L148

**问题**: 终端大小硬编码为 80x24，未根据实际视图调整。

---

### #13: 重连逻辑未集成
**位置**: 多个文件

**问题**: `ReconnectController` 实现完整但未在主流程中集成，没有自动重连功能。

---

### #14: 分屏拖动灵敏度固定值
**位置**: `zeterm/crates/zeterm/src/ui/split_pane/split_view.rs` L18

**问题**: `DRAG_SENSITIVITY = 400.0` 是固定像素值，在高 DPI 屏幕上表现不一致。

---

### #15: 对话框缺少 Z-Index 控制
**位置**: `zeterm/crates/zeterm/src/ui/main_window.rs` L1189-L1194

**问题**: 对话框只是简单作为子元素添加，可能被其他元素遮挡。

---

### #16: 硬编码的尺寸值
**位置**: 多个文件

**问题**: 
- 侧边栏宽度: `px(280.0)`
- Tab 栏高度: `px(40.0)`
- 状态栏高度: `px(24.0)`

这些值应该考虑系统 DPI 缩放或允许用户自定义。

---

### #17: SplitView 递归渲染无深度限制
**位置**: `zeterm/crates/zeterm/src/ui/split_pane/split_view.rs` L116-L180

**问题**: 无限分屏可能导致栈溢出。

---

### #18: 搜索功能性能问题
**位置**: `zeterm/crates/zeterm/src/ui/terminal_view/mod.rs` L332-L367

**问题**: 每次按键都遍历整个终端内容，大缓冲区时很慢。应该使用 debounce 或增量搜索。

---

### #19: 终端字体回退处理缺失
**位置**: `zeterm/crates/zeterm/src/ui/terminal_view/terminal_element.rs`

**问题**: 没有处理字体回退，如果主字体缺少字符将显示为空白或方框。

---

### #20: 潜在的死锁风险
**位置**: `zeterm/crates/zeterm/src/ui/tab_view.rs` L88-L99

**问题**: 如果在持有 `terminal_views` 写锁时调用 GPUI 方法，可能导致死锁。

---

### #21: 主机密钥确认对话框未集成
**位置**: `zeterm/crates/zeterm/src/ui/dialogs/host_key_dialog.rs`

**问题**: 实现了但未在主连接流程中使用。

---

### #22: SFTP 模块未启用
**位置**: `zeterm/crates/zeterm/src/ui/sftp/`

**问题**: SFTP 相关代码存在，但主界面没有启用入口。

---

### #23: 缺少快捷键处理的焦点检查
**位置**: `zeterm/crates/zeterm/src/ui/terminal_view/mod.rs` L433-L464

**问题**: 键盘事件处理没有检查焦点状态，可能在没有焦点的终端中也响应快捷键。

---

### #24: 日志配置硬编码为开发模式
**位置**: `zeterm/crates/zeterm/src/main.rs` L93

**问题**: `LogConfig::development()` 硬编码，生产环境应使用不同配置。

---

## 修复计划建议

### 第一阶段（紧急）
1. 修复 #1: Render 方法状态修改问题
2. 修复 #2: 连接状态更新问题
3. 修复 #3: 连接资源释放问题

### 第二阶段（重要）
4. 修复 #4: 全局数据库错误处理
5. 修复 #5: 连接错误 UI 反馈
6. 修复 #6: TextInput 光标显示
7. 修复 #9: GPUI 状态管理规范化

### 第三阶段（优化）
8. 修复 #7, #10, #11-24: 其他优化项

---

## 备注

- 部分问题涉及 GPUI 框架的使用方式，需要参考 GPUI 最新文档
- 建议添加自动化测试覆盖连接流程和 UI 交互
- 考虑使用 `tracing` 的 span 功能跟踪异步连接流程