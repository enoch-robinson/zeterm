# SessionModel 会话模型

> 连接 UI 层与后端的核心桥梁，管理单个终端会话的完整生命周期

---

## 一、设计目标

1. **状态集中** - 统一管理终端状态、连接状态、配置
2. **线程安全** - 支持 GPUI 主线程与 Tokio 异步任务交互
3. **生命周期管理** - 处理连接建立、数据传输、断开重连
4. **事件驱动** - 响应用户输入和后端数据

---

## 二、核心结构

```rust
use std::sync::Arc;
use parking_lot::Mutex;
use alacritty_terminal::Term;
use tokio_util::sync::CancellationToken;

/// 终端会话模型
///
/// 作为 GPUI Model 使用，是View 与 Backend 之间的桥梁
pub struct SessionModel {
    /// 终端状态机 (Alacritty)
    term: Arc<Mutex<Term<EventProxy>>>,
    
    /// 连接后端 (多态)
    backend: Arc<Mutex<Box<dyn TerminalConnection>>>,
    
    /// 连接状态机
    state_machine: ConnectionStateMachine,
    
    /// 会话配置
    config: SessionConfig,
    
    /// 会话 ID
    id: SessionId,
    
    /// 关联的主机ID
    host_id: Option<String>,
    
    /// 任务取消令牌
    cancel_token: CancellationToken,
    
    /// 终端尺寸
    size: TerminalSize,
}

/// 会话配置
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 滚动缓冲区行数
    pub scrollback_lines: u32,
    /// 终端类型
    pub term_type: String,
    /// 初始尺寸
    pub initial_size: TerminalSize,
}

/// 终端尺寸
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}
```

---

## 三、生命周期管理

### 3.1 创建流程

```rust
impl SessionModel {
    /// 创建新会话
    pub fn new(
        backend: Box<dyn TerminalConnection>,
        config: SessionConfig,
        cx: &mut ModelContext<Self>,
    ) -> Self {
        // 1. 创建终端状态机
        let term = Self::create_terminal(&config);
        
        // 2. 包装后端
        let backend = Arc::new(Mutex::new(backend));
        
        // 3. 创建状态机
        let state_machine = ConnectionStateMachine::new();
        
        // 4. 创建取消令牌
        let cancel_token = CancellationToken::new();
        
        let mut model = Self {
            term,
            backend,
            state_machine,
            config,
            id: SessionId::new(),
            host_id: None,
            cancel_token,
            size: config.initial_size,
        };
        
        // 5. 启动数据泵
        model.start_data_pump(cx);
        
        // 6. 更新状态
        model.state_machine.handle_event(ConnectionEvent::Connect);
        
        model
    }
    
    fn create_terminal(config: &SessionConfig) -> Arc<Mutex<Term<EventProxy>>> {
        let term_config = alacritty_terminal::Config::default();
        let size = SizeInfo::new(
            config.initial_size.cols as f32,
            config.initial_size.rows as f32,
            1.0,1.0, 0.0, 0.0, false,
        );
        
        let term = Term::new(term_config, &size, EventProxy);
        Arc::new(Mutex::new(term))
    }
}
```

### 3.2 数据泵 (Data Pump)

```rust
impl SessionModel {
    /// 启动后台数据接收任务
    fn start_data_pump(&mut self, cx: &mut ModelContext<Self>) {
        let backend = self.backend.clone();
        let term = self.term.clone();
        let cancel_token = self.cancel_token.clone();
        
        // 获取接收流
        let stream = backend.lock().receive_stream();
        
        cx.spawn(|this, mut cx| async move {
            let mut stream = stream;
            
            loop {
                tokio::select! {
                    // 监听取消信号
                    _ = cancel_token.cancelled() => {
                        tracing::info!("数据泵收到取消信号");
                        break;
                    }
                    
                    // 接收数据
                    result = stream.next() => {
                        match result {
                            Some(Ok(data)) => {
                                // 更新终端状态
                                term.lock().advance_bytes(&data);
                                
                                // 通知 UI 重绘
                                this.update(&mut cx, |_, cx| {
                                    cx.notify();
                                }).ok();
                            }
                            Some(Err(e)) => {
                                tracing::error!("接收数据错误: {}", e);
                                this.update(&mut cx, |model, cx| {
                                    model.handle_connection_error(e, cx);
                                }).ok();
                                break;
                            }
                            None => {
                                // 流结束
                                tracing::info!("数据流结束");
                                this.update(&mut cx, |model, cx| {
                                    model.handle_disconnect(cx);
                                }).ok();
                                break;
                            }
                        }
                    }
                }
            }
        }).detach();
    }
}
```

### 3.3 关闭流程

```rust
impl SessionModel {
    /// 关闭会话
    pub async fn close(&mut self) {
        // 1. 取消所有后台任务
        self.cancel_token.cancel();
        
        // 2. 关闭后端连接
        if let Err(e) = self.backend.lock().close().await {
            tracing::warn!("关闭连接失败: {}", e);
        }
        
        // 3. 更新状态
        self.state_machine.handle_event(ConnectionEvent::DisconnectComplete);
    }
}

impl Drop for SessionModel {
    fn drop(&mut self) {
        // 确保取消令牌被触发
        self.cancel_token.cancel();
    }
}
```

---

## 四、核心方法

### 4.1 输入处理

```rust
impl SessionModel {
    /// 发送用户输入到远端
    pub fn send_input(&self, bytes: &[u8], cx: &mut ModelContext<Self>) {
        let backend = self.backend.clone();
        let bytes = bytes.to_vec();
        
        cx.spawn(|_, _| async move {
            if let Err(e) = backend.lock().write(&bytes).await {
                tracing::error!("发送数据失败: {}", e);
            }
        }).detach();
    }
    
    /// 发送特殊按键
    pub fn send_key(&self, key: TerminalKey, cx: &mut ModelContext<Self>) {
        let bytes = key.to_ansi_bytes();
        self.send_input(&bytes, cx);
    }
}
```

### 4.2窗口调整

```rust
impl SessionModel {
    /// 调整终端大小
    pub fn resize(&mut self, size: TerminalSize, cx: &mut ModelContext<Self>) {
        // 1. 更新本地尺寸
        self.size = size;
        
        // 2. 更新 Alacritty 终端
        let size_info = SizeInfo::new(
            size.cols as f32,
            size.rows as f32,
            1.0, 1.0, 0.0, 0.0, false,
        );
        self.term.lock().resize(size_info);
        
        // 3. 通知远端
        let backend = self.backend.clone();
        cx.spawn(|_, _| async move {
            if let Err(e) = backend.lock().resize(size.rows, size.cols).await {
                tracing::error!("调整远端窗口大小失败: {}", e);
            }
        }).detach();
        
        // 4. 触发重绘
        cx.notify();
    }
}
```

### 4.3 状态查询

```rust
impl SessionModel {
    /// 获取可渲染内容
    pub fn renderable_content(&self) -> RenderableContent {
        self.term.lock().renderable_content()
    }
    
    /// 获取连接状态
    pub fn connection_status(&self) -> ConnectionStatus {
        self.state_machine.current_state().into()
    }
    
    /// 是否已连接
    pub fn is_connected(&self) -> bool {
        matches!(
            self.state_machine.current_state(),
            ConnectionState::Connected { .. }
        )
    }
    /// 获取终端尺寸
    pub fn size(&self) -> TerminalSize {
        self.size
    }
}
```

---

## 五、错误处理

```rust
impl SessionModel {
    fn handle_connection_error(&mut self, error: anyhow::Error, cx: &mut ModelContext<Self>) {
        tracing::error!("连接错误: {}", error);
        
        // 更新状态机
        self.state_machine.handle_event(ConnectionEvent::ConnectionLost);
        
        // 如果配置了自动重连，启动重连
        if self.config.auto_reconnect {
            self.start_reconnect(cx);
        }
        cx.notify();
    }
    
    fn handle_disconnect(&mut self, cx: &mut ModelContext<Self>) {
        self.state_machine.handle_event(ConnectionEvent::DisconnectComplete);
        cx.notify();
    }
    
    fn start_reconnect(&mut self, cx: &mut ModelContext<Self>) {
        // 重连逻辑...
    }
}
```

---

## 六、与 View 的交互

```rust
// 在 TerminalView 中使用 SessionModel
impl TerminalView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        // 读取会话状态
        let content = self.session.read(cx).renderable_content();
        let status = self.session.read(cx).connection_status();
        
        // 渲染终端内容...
    }
    
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut ViewContext<Self>) {
        let bytes = self.key_to_ansi(event);
        // 更新会话模型
        self.session.update(cx, |model, cx| {
            model.send_input(&bytes, cx);
        });
    }
}
```

---

## 七、相关文档

- [TerminalConnection Trait](../core/connection-trait.md) - 后端接口
- [连接状态机](../core/state-machine.md) - 状态管理
- [数据流设计](../architecture/data-flow.md) - 数据流转
- [TerminalView](./terminal-view.md) - 渲染视图