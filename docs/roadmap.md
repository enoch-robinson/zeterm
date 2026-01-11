# 实现路径与里程碑

> Zeterm 的分阶段实现计划与关键里程碑

---

## 一、实现原则

1. **渐进式开发** - 从最小可行产品开始，逐步增强
2. **Mock 驱动** - 先用 Mock 验证架构，再接入真实实现
3. **里程碑驱动** - 每个阶段有明确的可验证目标
4. **测试先行** - 核心模块必须有单元测试覆盖

---

## 二、阶段总览

>🎉 **重大简化**: 引入 `gpui-component` 组件库后，Phase 2 和 Phase 5 工作量大幅减少

```
Phase 1        Phase 2        Phase 3        Phase 4        Phase 5
核心骨架   ──►  终端渲染   ──►  SSH 接入   ──►  交互完善  ──►  产品化
 (2周)          (1周)          (2周)          (1周)          (1周)
   │              │              │              │              │
   ▼              ▼              ▼
MockConnection  TerminalView  真实SSH连接    鼠标/复制粘贴   主机管理
Alacritty集成   字符网格渲染  密码/密钥认证  窗口Resize      SFTP/Tab
gpui-component  (自行实现)                (复用组件)
```

**总工期: 约 7 周** (原计划 9 周，节省 2 周)

---

## 三、Phase 1: 核心骨架 (The Skeleton)

### 3.1 目标

搭建项目基础架构，验证核心抽象设计，集成 gpui-component 组件库。

### 3.2 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 项目初始化 | Cargo workspace 结构 | P0 |
| GPUI 集成 | 创建基础窗口 | P0 |
| **gpui-component 集成** | 初始化组件库，验证基础组件 | P0 |
| 定义 `TerminalConnection` Trait | 核心抽象接口 | P0 |
| 实现 `MockConnection` | 测试用 Mock 后端 | P0 |
| 集成 `alacritty_terminal` | 终端状态机 | P0 |
| 实现 `SessionModel` | 连接 Mock 与 Alacritty | P0 |
| 基础日志系统 | tracing 集成 | P1 |

### 3.3 项目结构

```
zeterm/
├── Cargo.toml
├── crates/
│   ├── zeterm/              # 主程序
│   │   └── src/
│   │       ├── main.rs
│   │       └── ui/          # UI 组件 (基于 gpui-component)
│   │           ├── mod.rs
│   │           └── terminal_view.rs
│   ├── zeterm-core/         # 核心抽象
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs    # TerminalConnection Trait
│   │       ├── error.rs         # 错误类型
│   │       └── state.rs         # 状态机
│   ├── zeterm-terminal/     # 终端模型
│   │   └── src/
│   │       ├── lib.rs
│   │       └── session.rs       # SessionModel
│   └── zeterm-mock/         # Mock 实现
│       └── src/
│           └── lib.rs
└── docs/
```

### 3.4 里程碑验证

```
✅ 运行程序，看到 GPUI 窗口
✅ gpui-component 基础组件正常渲染 (Button, Input)
✅ MockConnection 每秒输出 "Hello World\n"
✅ 控制台打印 Alacritty 解析后的内容
✅ 单元测试通过
```

### 3.5 关键代码

```rust
// main.rs - gpui-component 初始化
use gpui::*;
use gpui_component::*;

fn main() {
    let app = Application::new();
    app.run(move |cx| {
        // 必须在使用任何 gpui-component 功能前调用
        gpui_component::init(cx);
        
        cx.open_window(WindowOptions::default(), |window, cx| {
            let view = cx.new(|_| AppView::new());
            cx.new(|cx| Root::new(view, window, cx))
        });
    });
}

// MockConnection 实现
pub struct MockConnection {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Option<mpsc::Receiver<Vec<u8>>>,
}

impl MockConnection {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(256);
        
        // 启动模拟输出任务
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let _ = tx_clone.send(b"Hello World\n".to_vec()).await;
            }
        });
        
        Self { tx, rx: Some(rx) }
    }
}
```

---

## 四、Phase 2: 终端渲染 (The Renderer)

>⚡ **简化说明**: 不再从Zed 移植，而是基于 gpui-component 自行实现轻量级终端渲染器

### 4.1 目标

实现终端字符网格渲染，与 Alacritty 终端状态机对接。

### 4.2 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 实现 `TerminalView` | 基于 GPUI canvas绘制字符网格 | P0 |
| 字体度量计算 | 等宽字体单元格尺寸 | P0 |
| 字符渲染 | 使用 GPUI text API | P0 |
| 颜色支持 | 256色/TrueColor | P0 |
| 光标渲染 | Block/Beam/Underline | P1 |
| 集成 gpui-component | 使用 Root、Theme 等基础设施 | P0 |

### 4.3 实现策略

```
自行实现 (参考 Zed 但不直接移植)
┌─────────────────────────────────────────┐
│  TerminalView (gpui::Render)            │
│  ├──读取 SessionModel 状态             │
│  ├── 计算字符网格布局                   │
│  └── 使用 canvas() 绘制                │
├─────────────────────────────────────────┤
│  复用 gpui-component                │
│  ├── Root -窗口根组件                  │
│  ├── Theme - 颜色主题                   │
│  └── 基础样式系统                       │
└─────────────────────────────────────────┘
```

### 4.4 核心渲染代码

```rust
impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.session.read(cx).renderable_content();
        
        canvas(
            move |bounds, cx| self.prepaint(bounds, cx),
            move |bounds, cx| {
                // 绘制背景
                //绘制字符网格
                // 绘制光标
            },
        )
        .size_full()
    }
}
```

### 4.5 里程碑验证

```
✅ 屏幕显示终端背景 (使用 gpui-component 主题色)
✅ Mock 输出的 "Hello World" 正确显示
✅ 光标可见且位置正确
✅ 支持基本 ANSI 颜色
✅ 字体渲染清晰 (等宽字体)
```

---

## 五、Phase 3: SSH 接入 (The Real Deal)

### 5.1 目标

实现真实的 SSH 连接，替换 MockConnection。

### 5.2 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 集成 russh | SSH 协议库 | P0 |
| 实现 `SshConnection` | TerminalConnection 实现 | P0 |
| 密码认证 | 基础认证方式 | P0 |
| 公钥认证 | 支持 RSA/Ed25519 | P0 |
| SSH Agent | 系统密钥代理 | P1 |
| 主机密钥验证 | known_hosts 支持 | P1 |
| 连接状态机 | 状态管理 | P0 |

### 5.3 认证流程

```
┌─────────┐     ┌─────────────┐     ┌─────────────┐
│用户输入 │────►│ 认证方式选择 │────►│ 执行认证    │
└─────────┘     └─────────────┘     └──────┬──────┘
                                          │
                     ┌────────────────────┼────────────────────┐▼                    ┌───────────┐       ┌───────────┐       ┌───────────┐
              │ 密码认证  │       │ 公钥认证  │       │ Agent认证 │
              └─────┬─────┘       └─────┬─────┘       └─────┬─────┘
                    │                   │                   │
                    └───────────────────┴───────────────────┘│
                                        ▼
                                 ┌─────────────┐
                                 │ 打开 PTY    │
                                 │ 请求 Shell│
                                 └─────────────┘
```

### 5.4 里程碑验证

```
✅ 成功连接到真实 SSH 服务器
✅ 密码认证工作正常
✅ 公钥认证工作正常
✅ 运行 htop，画面流畅，颜色正确
✅ 运行 vim，编辑文件正常
```

---

## 六、Phase 4: 交互完善 (The Polish)

### 6.1 目标

完善用户交互体验。

### 6.2 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 窗口 Resize | 同步终端尺寸 | P0 |
| 复制粘贴 | 系统剪贴板集成 | P0 |
| 鼠标支持 | 点击、选择、滚轮 | P1 |
| 滚动缓冲区 | 历史内容查看 | P1 |
| 搜索功能 | 终端内容搜索 | P2 |
| 快捷键 | 自定义快捷键 | P1 |

### 6.3Resize 流程

```rust
impl TerminalView {
    fn handle_layout_changed(&mut self, size: Size, cx: &mut ViewContext<Self>) {
        // 1. 计算新的行列数
        let (rows, cols) = self.calculate_dimensions(size);
        
        // 2. 更新 Alacritty 终端
        self.session.update(cx, |model, _| {
            model.term.lock().resize(rows, cols);
        });
        
        // 3. 通知远端
        self.session.update(cx, |model, cx| {
            model.resize(rows, cols, cx);
        });
    }
}
```

### 6.4 里程碑验证

```
✅ 拖拽窗口边缘，终端内容自适应
✅ Ctrl+Shift+C 复制选中内容
✅ Ctrl+Shift+V 粘贴剪贴板
✅ 鼠标点击定位光标 (支持的程序)
✅ 滚轮查看历史输出
```

---

## 七、Phase 5: 产品化 (The Product)

> ⚡ **大幅简化**: gpui-component 提供了Table、Tab、Dock、Tree 等组件，直接复用

### 7.1 目标

完成产品级功能，可供日常使用。

### 7.2 任务清单

| 任务 | 说明 | gpui-component | 优先级 |
|------|------|----------------|--------|
| 主机列表 | 侧边栏管理 | ✅ `Table` + `Tree` | P0 |
| 配置系统 | TOML 配置文件 | - | P0 |
| Tab 管理 | 多标签页 | ✅ `Tab` | P0 |
| 分屏布局 | 水平/垂直分割 | ✅ `Dock` | P0 |
| SFTP 面板 | 文件管理 | ✅ `Table` + `Tree` | P1 |
| 主题系统 | 颜色主题切换 | ✅ `Theme` | P0 |
| 数据持久化 | SQLite 存储 | - | P0 |
| 连接对话框 | 新建/编辑主机 | ✅ `Modal` + `Input` | P0 |
| 状态提示 | 连接状态通知 | ✅ `Notification` | P1 |
| 传输进度 | 文件上传下载 | ✅ `Progress` | P1 |

### 7.3 UI 布局 (基于 gpui-component Dock)

```
┌─────────────────────────────────────────────────────────────┐
│  [Tab1] [Tab2] [Tab3][+] [≡]        │  ← gpui_component::Tab
├──────────┬──────────────────────────────────────────────────┤
│          │                                                  │  ← gpui_component::Dock
│  主机列表 │              终端区域                            │
│ (Tree)   │           (TerminalView)                         │
│          │                                                  │
│▼ 生产环境│  user@server:~$ ls -la                          │
│   Server1│  total 32                                        │
│   Server2│  drwxr-xr-x  5 user user 4096 Jan  1 00:00 .     │
│          │  drwxr-xr-x  3 root root 4096 Jan  1 00:00 ..    │
│ ▼ 开发环境│  -rw-r--r--  1 user user  220 Jan  1 00:00 file  │
│   DevBox │  user@server:~$ █                │
│          │                                                  │
├──────────┴──────────────────────────────────────────────────┤
│  🟢 Connected | UTF-8 | 80x24                               │  ← StatusBar
└─────────────────────────────────────────────────────────────┘
```

### 7.4 组件映射

```rust
use gpui_component::*;

// 主窗口布局
fn render_app(&self, cx: &mut Context<Self>) -> impl IntoElement {
    Root::new(
        v_flex()
            .child(Tab::new(...))           // 标签栏
            .child(
                Dock::new()                  // 分屏布局
                    .left(HostTreeView {})   // 主机列表
                    .center(TerminalView {}) // 终端区域
            )
            .child(StatusBar::new(...)),    // 状态栏window, cx
    )
}
```

### 7.5 里程碑验证

```
✅ 可以添加、编辑、删除主机 (Modal + Input)
✅ 双击主机快速连接(Tree组件)
✅ 多标签页切换 (Tab 组件)
✅ 分屏同时查看多个终端 (Dock 组件)
✅ SFTP 上传下载文件 (Table + Progress)
✅ 配置文件修改后生效
✅ 深色/浅色主题切换 (Theme)
```

---

## 八、技术风险与应对

| 风险 | 影响 | 应对策略 | 状态 |
|------|------|----------|------|
| ~~Zed 渲染器移植困难~~ | ~~Phase 2 延期~~ | ~~预留额外时间~~ | ✅ 已规避 (自行实现) |
| russh 兼容性问题 | Phase 3 阻塞 | 提前调研，准备 thrussh 备选 | ⚠️ 待验证 |
| GPUI 学习曲线 | 整体延期 | gpui-component 示例丰富，降低学习成本 | 🟢 风险降低 |
| 性能问题 | 用户体验差 | 持续性能测试，及时优化 | ⚠️ 待验证 |
| gpui-component 版本兼容 | 升级困难 | 锁定版本，关注 changelog | 🟡 新增风险 |
| 终端渲染器自行实现 | Phase 2 延期 | 参考 Zed 源码，保持简单 | 🟡 新增风险 |

### 风险变化说明

**已消除的风险:**
- ~~从 Zed 移植 TerminalView~~ → 改为自行实现轻量级渲染器

**降低的风险:**
- GPUI 学习曲线→ gpui-component 提供丰富示例和文档

**新增的风险:**
- gpui-component 作为第三方库，需关注其稳定性和更新频率
- 自行实现终端渲染器需要一定工作量（但比移植 Zed 简单）

---

## 九、依赖清单

```toml
[dependencies]
# UI 框架
gpui = "0.2"
gpui-component = "0.4"      # 🆕 60+ UI组件

# 终端模拟
alacritty_terminal = "0.24"

# SSH
russh = "0.45"
russh-keys = "0.45"

# 异步运行时
tokio = { version = "1", features = ["full"] }
futures = "0.3"
async-trait = "0.1"

# 序列化
serde = { version = "1", features = ["derive"] }
toml = "0.8"

# 数据库
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }

# 错误处理
anyhow = "1"
thiserror = "2"

# 日志
tracing = "0.1"
tracing-subscriber = "0.3"
```

### gpui-component 提供的组件

| 类别 | 组件 |
|------|------|
| 基础 | Button, Input, Checkbox, Radio, Switch, Slider |
| 布局 | Dock, Tab, Modal, Drawer, Popover |
| 数据展示 | Table, Tree, List, Progress, Badge |
| 反馈 | Notification, Toast, Tooltip |
| 导航 | Dropdown, ContextMenu, Breadcrumb |

---

## 十、相关文档

- [总体设计](./design.md) - 架构概述
- [四层架构](./architecture/layers.md) - 分层设计
- [TerminalConnection Trait](./core/connection-trait.md) - 核心接口
- [SSH 后端](./modules/ssh-backend.md) - SSH 实现细节