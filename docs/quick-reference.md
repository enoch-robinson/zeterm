# Zeterm 快速参考卡

> 开发过程中的常用参考信息速查

---

## 一、ANSI 转义序列速查

### 1.1 CSI 序列格式

```
ESC [ <参数> <中间字节> <最终字节>
\x1b[  .........
```

### 1.2 光标控制

| 序列 | 说明 | 示例 |
|------|------|------|
| `\x1b[H` | 光标移到左上角 | - |
| `\x1b[<r>;<c>H` | 光标移到 (r,c) | `\x1b[5;10H` |
| `\x1b[<n>A` | 光标上移 n 行 | `\x1b[3A` |
| `\x1b[<n>B` | 光标下移 n 行 | `\x1b[3B` |
| `\x1b[<n>C` | 光标右移 n 列 | `\x1b[3C` |
| `\x1b[<n>D` | 光标左移 n 列 | `\x1b[3D` |
| `\x1b[s` | 保存光标位置 | - |
| `\x1b[u` | 恢复光标位置 | - |
| `\x1b[?25h` | 显示光标 | - |
| `\x1b[?25l` | 隐藏光标 | - |

### 1.3 清屏操作

| 序列 | 说明 |
|------|------|
| `\x1b[J` | 清除光标到屏幕末尾 |
| `\x1b[0J` | 同上 |
| `\x1b[1J` | 清除屏幕开头到光标 |
| `\x1b[2J` | 清除整个屏幕 |
| `\x1b[3J` | 清除整个屏幕和滚动缓冲区 |
| `\x1b[K` | 清除光标到行尾 |
| `\x1b[0K` | 同上 |
| `\x1b[1K` | 清除行首到光标 |
| `\x1b[2K` | 清除整行 |

### 1.4 文本样式 (SGR)

格式: `\x1b[<n>m`

| 代码 | 说明 | 代码 | 说明 |
|------|------|------|------|
| 0 | 重置所有属性 | 1 | 粗体 |
| 2 | 暗淡 | 3 | 斜体 |
| 4 | 下划线 | 5 | 闪烁 |
| 7 | 反色 | 8 | 隐藏 |
| 9 | 删除线 | 22 | 取消粗体/暗淡 |
| 23 | 取消斜体| 24 | 取消下划线 |
| 27 | 取消反色 | 29 | 取消删除线 |

### 1.5 前景色 (文字颜色)

| 代码 | 颜色 | 亮色代码 | 亮色 |
|------|------|----------|------|
| 30 | 黑色 | 90 | 亮黑 (灰) |
| 31 | 红色 | 91 | 亮红 |
| 32 | 绿色 | 92 | 亮绿 |
| 33 | 黄色 | 93 | 亮黄 |
| 34 | 蓝色 | 94 | 亮蓝 |
| 35 | 品红 | 95 | 亮品红 |
| 36 | 青色 | 96 | 亮青 |
| 37 | 白色 | 97 | 亮白 |
| 39 | 默认色 | - | - |

### 1.6 背景色

| 代码 | 颜色 | 亮色代码 | 亮色 |
|------|------|----------|------|
| 40 | 黑色 | 100 | 亮黑 |
| 41 | 红色 | 101 | 亮红 |
| 42 | 绿色 | 102 | 亮绿 |
| 43 | 黄色 | 103 | 亮黄 |
| 44 | 蓝色 | 104 | 亮蓝 |
| 45 | 品红 | 105 | 亮品红 |
| 46 | 青色 | 106 | 亮青 |
| 47 | 白色 | 107 | 亮白 |
| 49 | 默认色 | - | - |

### 1.7 256 色模式

```
前景色: \x1b[38;5;<n>m    (n = 0-255)
背景色: \x1b[48;5;<n>m    (n = 0-255)

颜色分布:
  0-7:标准色
  8-15:   高亮色
  16-231: 216 色立方体 (6x6x6)
  232-255: 24 级灰度
```

### 1.8 TrueColor (24位)

```
前景色: \x1b[38;2;<r>;<g>;<b>m
背景色: \x1b[48;2;<r>;<g>;<b>m

示例: \x1b[38;2;255;128;0m  (橙色文字)
```

---

## 二、终端模式控制

### 2.1 DEC 私有模式

| 序列 | 说明 |
|------|------|
| `\x1b[?1h` | 启用应用光标键模式 (DECCKM) |
| `\x1b[?1l` | 禁用应用光标键模式 |
| `\x1b[?25h` | 显示光标 (DECTCEM) |
| `\x1b[?25l` | 隐藏光标 |
| `\x1b[?47h` | 切换到备用屏幕缓冲区 |
| `\x1b[?47l` | 切换回主屏幕缓冲区 |
| `\x1b[?1049h` | 保存光标并切换到备用屏幕 |
| `\x1b[?1049l` | 恢复光标并切换回主屏幕 |
| `\x1b[?2004h` | 启用括号粘贴模式 |
| `\x1b[?2004l` | 禁用括号粘贴模式 |

### 2.2 鼠标模式

| 序列 | 说明 |
|------|------|
| `\x1b[?1000h` | 启用鼠标点击报告 |
| `\x1b[?1002h` | 启用鼠标按钮事件报告 |
| `\x1b[?1003h` | 启用所有鼠标事件报告 |
| `\x1b[?1006h` | 启用 SGR 鼠标模式 |

---

## 三、SSH 协议速查

### 3.1 消息类型

| 类型 | 值 | 说明 |
|------|-----|------|
| SSH_MSG_DISCONNECT | 1 | 断开连接 |
| SSH_MSG_IGNORE | 2 | 忽略消息 |
| SSH_MSG_USERAUTH_REQUEST | 50 | 认证请求 |
| SSH_MSG_USERAUTH_SUCCESS | 52 | 认证成功 |
| SSH_MSG_USERAUTH_FAILURE | 51 | 认证失败 |
| SSH_MSG_CHANNEL_OPEN | 90 | 打开通道 |
| SSH_MSG_CHANNEL_DATA | 94 | 通道数据 |
| SSH_MSG_CHANNEL_EOF | 96 | 通道 EOF |
| SSH_MSG_CHANNEL_CLOSE | 97 | 关闭通道 |

### 3.2 认证方法

| 方法 | 说明 |
|------|------|
| `none` | 无认证 |
| `password` | 密码认证 |
| `publickey` | 公钥认证 |
| `keyboard-interactive` | 键盘交互认证 |
| `hostbased` | 基于主机认证 |

### 3.3 PTY 请求参数

```rust
struct PtyRequest {
    term: String,      // 终端类型 (如 "xterm-256color")
    cols: u32,         // 列数
    rows: u32,         // 行数
    width_px: u32,     // 像素宽度 (可为 0)
    height_px: u32,    // 像素高度 (可为 0)
    modes: Vec<u8>,    // 终端模式
}
```

---

## 四、russh API 速查

### 4.1 连接建立

```rust
use russh::*;
use russh_keys::*;

// 创建配置
let config = client::Config::default();

// 连接服务器
let mut session = client::connect(
    Arc::new(config),
    (host, port),
    handler,
).await?;

// 密码认证
let auth_result = session
    .authenticate_password(username, password)
    .await?;

// 公钥认证
let key = load_secret_key(key_path, passphrase)?;
let auth_result = session
    .authenticate_publickey(username, Arc::new(key))
    .await?;
```

### 4.2 通道操作

```rust
// 打开会话通道
let channel = session.channel_open_session().await?;

// 请求 PTY
channel.request_pty(
    false,           // want_reply
    "xterm-256color", // term
    cols, rows,      // 尺寸
    0, 0,            // 像素尺寸
    &[],             // 终端模式
).await?;

// 请求 Shell
channel.request_shell(false).await?;

// 发送数据
channel.data(&data[..]).await?;

// 调整窗口大小
channel.window_change(cols, rows, 0, 0).await?;

// 关闭通道
channel.eof().await?;
channel.close().await?;
```

---

## 五、alacritty_terminal API 速查

### 5.1 创建终端

```rust
use alacritty_terminal::term::{Term, Config};
use alacritty_terminal::event::EventListener;

// 事件监听器
struct EventProxy;
impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        // 处理事件
    }
}

// 创建终端
let config = Config::default();
let size = SizeInfo::new(cols, rows, cell_width, cell_height,0.0, 0.0, false);
let term = Term::new(config, &size, EventProxy);
```

### 5.2 数据处理

```rust
// 输入数据
term.advance_bytes(&data);

// 获取可渲染内容
let content = term.renderable_content();

// 遍历单元格
for cell in content.display_iter() {
    let point = cell.point;// 位置
    let c = cell.c;              // 字符
    let fg = cell.fg;            // 前景色
    let bg = cell.bg;            // 背景色
    let flags = cell.flags;      // 属性标志
}

// 获取光标
let cursor = content.cursor;
```

### 5.3 终端操作

```rust
// 调整大小
term.resize(SizeInfo::new(...));

// 滚动
term.scroll_display(Scroll::Lines(delta));

// 选择
term.selection = Some(Selection::new(...));

// 获取选中文本
let text = term.selection_to_string();
```

---

## 六、GPUI 速查

### 6.1 Element 实现

```rust
impl Element for MyElement {
    type RequestLayoutState = ();
    type PrepaintState = MyPrepaintState;

    fn id(&self) -> Option<ElementId> {
        Some("my-element".into())
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        cx: &mut WindowContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = cx.request_layout(Style::default(), []);
        (layout_id,())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) -> Self::PrepaintState {
        // 准备绘制数据
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut WindowContext,
    ) {
        // 执行绘制
    }
}
```

### 6.2 常用绘制操作

```rust
// 填充矩形
cx.paint_quad(fill(bounds, color));

// 描边矩形
cx.paint_quad(outline(bounds, color, thickness));

// 绘制文本
cx.paint_text(origin, &text, style);
```

---

## 七、常用命令

### 7.1 开发命令

```bash
# 构建
cargo build

# 运行
cargo run

# 测试
cargo test

# 检查
cargo check

# 格式化
cargo fmt

# Lint
cargo clippy
```

### 7.2 调试命令

```bash
# 启用日志
RUST_LOG=debug cargo run

# 特定模块日志
RUST_LOG=zeterm_ssh=trace cargo run

# 性能分析
cargo build --release
perf record ./target/release/zeterm
perf report
```

---

## 八、相关文档

- [终端渲染实现](./modules/terminal-rendering.md)
- [按键映射表](./modules/key-mappings.md)
- [跨平台差异](./infrastructure/platform.md)
- [API 文档](./api.md)
- [实现路径](./roadmap.md)