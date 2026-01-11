# 数据流与线程模型

>定义 Zeterm 的数据流转路径与多线程协作机制

---

## 一、数据流概览

```
┌─────────────────────────────────────────────────────────────────────┐
│                          用户交互                                    │
│                (键盘/鼠标/窗口事件)                               │
└──────────────────────────┬──────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────┐
│                GPUI 主线程                                     │
│┌─────────────┐    ┌─────────────┐    ┌─────────────┐              │
│  │TerminalView │───►│SessionModel │───►│   渲染      │              │
│  │ (事件处理)  │    │ (状态更新)  │    │  (paint)    │              │
│  └─────────────┘    └──────┬──────┘    └─────────────┘              │
└────────────────────────────┼────────────────────────────────────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             ▼                             ▼
┌─────────────────────────┐   ┌─────────────────────────┐
│      写入通道           │   │      接收通道           │
│  (用户输入 → 远端)│   │  (远端 → 终端解析)      │
└───────────┬─────────────┘   └─────────────┬───────────┘
            │                               │
            ▼                               │
┌─────────────────────────────────────────────────────────────────────┐
│                       Tokio 异步运行时                               │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                Backend Task│    │
│  │  ┌──────────┐    ┌──────────┐    ┌──────────┐               │    │
│  │  │ SSH/PTY  │◄──►│ Channel  │◄──►│远端服务 │               │    │
│  │  │ Backend  │    │ (russh)  │    │          │               │    │
│  │  └──────────┘    └──────────┘    └──────────┘               │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────┘
```

---

## 二、输入数据流 (用户 → 远端)

### 2.1 流程步骤

```
1. 用户按键│
       ▼
2. GPUI 捕获 KeyDown 事件
       │
       ▼
3. TerminalView.handle_key_event()
       │
       ▼
4. 转换为 ANSI 转义序列
       │
       ▼
5. SessionModel.send_input(bytes)
       │
       ▼
6. Backend.write(bytes)
       │
       ▼
7. 网络发送到远端服务器
```

### 2.2 代码示例

```rust
impl TerminalView {
    fn handle_key_event(&mut self, event: &KeyDownEvent, cx: &mut ViewContext<Self>) {
        // 1. 将按键转换为 ANSI 序列
        let bytes = self.key_to_ansi(event);
        
        // 2. 发送到 SessionModel
        self.session.update(cx, |model, _| {
            model.send_input(&bytes);
        });
    }
}

impl SessionModel {
    pub fn send_input(&mut self, bytes: &[u8]) {
        // 3. 通过后端发送
        let backend = self.backend.clone();
        let bytes = bytes.to_vec();
        
        // 4. 异步发送，不阻塞主线程
        cx.spawn(|_, _| async move {
            backend.lock().await.write(&bytes).await
        }).detach();
    }
}
```

---

## 三、输出数据流 (远端 → 显示)

### 3.1 流程步骤

```
1. 远端服务器发送数据
       │
       ▼
2. Backend.receive_stream() 产生数据
       │
       ▼
3. Data Pump 任务接收
       │
       ▼
4. Term.advance_bytes() 解析 ANSI
       │
       ▼
5. 更新终端状态 (Grid)
       │
       ▼
6. cx.notify() 触发重绘
       │
       ▼
7. TerminalView.render() 渲染新内容
```

### 3.2 Data Pump 实现

```rust
impl SessionModel {
    /// 启动数据泵任务
    fn start_data_pump(
        &self,
        mut stream: BoxStream<'static, Result<Vec<u8>>>,
        cx: &mut ModelContext<Self>,
    ) {
        cx.spawn(|this, mut cx| async move {
            while let Some(result) = stream.next().await {
                match result {
                    Ok(data) => {
                        // 在主线程更新终端状态
                        this.update(&mut cx, |model, cx| {
                            model.term.lock().advance_bytes(&data);
                            cx.notify(); // 触发重绘
                        }).ok();
                    }
                    Err(e) => {
                        // 处理错误，可能触发重连
                        this.update(&mut cx, |model, cx| {
                            model.handle_connection_error(e, cx);}).ok();
                        break;
                    }
                }
            }
        }).detach();
    }
}
```

---

## 四、线程模型

### 4.1 线程分布

| 线程 | 职责 | 特点 |
|------|------|------|
| GPUI 主线程 | UI 渲染、事件处理 | 必须保持响应 |
| Tokio 工作线程池 | 网络 IO、异步任务 | 多线程并发 |
|后台任务线程 | 长时间运算| 避免阻塞主线程 |

### 4.2 线程交互

```
┌─────────────────┐         ┌─────────────────┐
│   GPUI 主线程   │◄───────►│  Tokio Runtime  │
│                 │ channel │                 │
│- UI 渲染      │         │  - 网络 IO      │
│  - 事件处理     │         │  - SSH 协议     │
│  - 状态更新     │         │  - 文件传输     │
└─────────────────┘         └─────────────────┘
        │                           │
        │cx.spawn()           │
        └───────────────────────────┘
```

### 4.3 同步机制

```rust
// 1. Arc<Mutex<T>> - 共享可变状态
let term: Arc<Mutex<Term>> = Arc::new(Mutex::new(term));

// 2. mpsc::channel - 单向数据流
let (tx, rx) = tokio::sync::mpsc::channel(1024);

// 3. watch::channel - 状态广播
let (state_tx, state_rx) = tokio::sync::watch::channel(state);

// 4. oneshot::channel - 一次性响应
let (result_tx, result_rx) = tokio::sync::oneshot::channel();
```

---

## 五、背压控制

### 5.1 问题场景

当远端数据产生速度超过终端解析速度时，需要背压控制。

### 5.2 解决方案

```rust
// 使用有界 channel 实现背压
let (tx, rx) = mpsc::channel::<Vec<u8>>(1024);

// 当channel 满时，send 会等待
async fn forward_data(tx: Sender<Vec<u8>>, data: Vec<u8>) {
    // 如果 channel 满，这里会等待消费者处理
    tx.send(data).await.ok();
}
```

### 5.3 流量控制参数

```rust
pub struct FlowControlConfig {
    /// 接收缓冲区大小
    pub recv_buffer_size: usize,
    /// 批量处理阈值
    pub batch_threshold: usize,
    
    /// 最大批量延迟 (ms)
    pub max_batch_delay_ms: u64,
}

impl Default for FlowControlConfig {
    fn default() -> Self {
        Self {
            recv_buffer_size: 1024,
            batch_threshold: 4096,
            max_batch_delay_ms: 16, // ~60fps
        }
    }
}
```

---

## 六、任务生命周期

### 6.1 任务类型

| 任务 | 生命周期 | 取消方式 |
|------|----------|----------|
| Data Pump | 与Session 相同 | Session 关闭时取消 |
| 心跳任务 | 与连接相同 | 断开连接时取消 |
| 文件传输 | 独立 | 用户取消或完成 |

### 6.2 优雅取消

```rust
use tokio_util::sync::CancellationToken;

pub struct SessionModel {
    /// 取消令牌
    cancel_token: CancellationToken,
}

impl SessionModel {
    fn start_data_pump(&self, stream: BoxStream<...>, cx: &mut ModelContext<Self>) {
        let token = self.cancel_token.clone();
        
        cx.spawn(|this, mut cx| async move {
            tokio::select! {
                _ = token.cancelled() => {
                    // 收到取消信号，优雅退出
                }
                _ = async {
                    while let Some(data) = stream.next().await {
                        // 处理数据...
                    }
                } => {}
            }
        }).detach();
    }
    
    fn close(&mut self) {
        // 取消所有关联任务
        self.cancel_token.cancel();
    }
}
```

---

## 七、相关文档

- [四层架构](./layers.md) - 架构分层设计
- [SessionModel](../modules/session-model.md) - 会话模型详情
- [SSH 后端](../modules/ssh-backend.md) - 后端实现细节