#实现路径与里程碑

> Zeterm 的分阶段实现计划与关键里程碑

---

## 一、实现原则

| 原则 | 说明 |
|------|------|
| 渐进式开发 | 从最小可行产品开始，逐步增强|
| Mock驱动 | 先用 Mock 验证架构，再接入真实实现 |
| 里程碑驱动 | 每个阶段有明确的可验证目标 |
| 测试先行 | 核心模块必须有单元测试覆盖 |

---

## 二、阶段总览

```
Phase 1        Phase 2        Phase 3        Phase 4        Phase 5
核心骨架   ──►终端渲染   ──►  SSH 接入   ──►  交互完善  ──►  产品化
 (2周)          (2周)          (2周)          (1周)          (2周)
   ││              │              │              │▼              ▼              ▼
MockConnectionTerminalView真实SSH连接    鼠标/复制粘贴   主机管理
Alacritty集成   (参考Zed)     密码/密钥认证  窗口ResizeSFTP/Tab
gpui-component  字符网格渲染                滚动缓冲区      (gpui-component)
```

**总工期: 约 9 周**

---

## 三、Phase 1: 核心骨架 (2周)

### 3.1 目标

搭建项目基础架构，验证核心抽象设计。

### 3.2 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 项目初始化 | Cargo workspace 结构 | P0 |
| GPUI 集成 | 创建基础窗口 | P0 |
| gpui-component 集成 | 初始化组件库 | P0 |
| 定义 `TerminalConnection` Trait | 核心抽象接口 | P0 |
| 实现 `MockConnection` | 测试用Mock 后端 | P0 |
| 集成 `alacritty_terminal` | 终端状态机 | P0 |
| 实现 `SessionCoordinator` | 连接 Mock 与 Alacritty | P0 |
| 基础日志系统 | tracing 集成 | P1 |

### 3.3 项目结构

```
zeterm/
├── Cargo.toml
├── crates/
│   ├── zeterm/# 主程序 (表现层 + 应用层)
│   │   └── src/
│   │       ├── main.rs
│   │       ├── ui/          # 表现层
│   │       └── app/         # 应用层
│   ├── zeterm-core/         # 领域层
│   │   └── src/
│   │       ├── traits/      # TerminalConnection 等
│   │       ├── entities/    # HostConfig 等
│   │       └── state/       # ConnectionState 等
│   ├── zeterm-terminal/     # 终端模型
│   └── zeterm-mock/         # Mock 实现
└── docs/
```

### 3.4 里程碑验证

- ✅ 运行程序，看到 GPUI 窗口
- ✅ gpui-component 基础组件正常渲染
- ✅ MockConnection 每秒输出 "Hello World\n"
- ✅ 控制台打印 Alacritty 解析后的内容
- ✅ 单元测试通过

---

## 四、Phase 2: 终端渲染 (2周)

### 4.1 目标

实现终端字符网格渲染，与 Alacritty 终端状态机对接。

### 4.2 组件分工

| 组件 | 来源 | 用途 |
|------|------|------|
| **TerminalView** | 参考 Zed `terminal_view` | 终端字符网格渲染 |
| **TerminalElement** | 参考 Zed `terminal_element` | 底层绘制实现 |
| **窗口 UI** | gpui-component | Tab、Dock、按钮等 |

>⚠️ **重要**: 终端渲染器需参考 Zed 实现，gpui-component 不提供终端渲染能力。

### 4.3 任务清单

| 任务 | 说明 | 优先级 |
|------|------|--------|
| 研究 Zed terminal_view | 理解渲染架构 | P0 |
| 实现 `TerminalElement` | 基于GPUI canvas 绘制 | P0 |
| 字体度量计算 | 等宽字体单元格尺寸 | P0 |
| 字符渲染 | 使用 GPUI text API | P0 |
| 颜色支持 | 256色/TrueColor | P0 |
| 光标渲染 | Block/Beam/Underline | P1 |
| 集成 gpui-component | Root、Theme等基础设施 | P0 |

### 4.4 Zed 参考文件

```
zed/crates/terminal_view/src/
├── terminal_view.rs      # 主视图组件 →参考
├── terminal_element.rs   # 渲染元素 → 核心参考
└── persistence.rs        # 可忽略

zed/crates/terminal/src/
├── terminal.rs           # 终端模型 → 参考
└── mappings/             # 按键映射 → 参考
```

### 4.5 实现策略

```
┌─────────────────────────────────────────────────┐
│  TerminalView (gpui::Render)                    │
│  ├──读取 SessionCoordinator 状态              │
│  ├── 创建 TerminalElement                │
│  └── 处理键盘/鼠标事件                          │
├─────────────────────────────────────────────────┤
│  TerminalElement (gpui::Element)                │
│  ├── prepaint(): 计算字体度量、布局             │
│  └── paint(): 绘制背景、字符、光标              │
├─────────────────────────────────────────────────┤
│  复用 gpui-component                │
│  ├── Root -窗口根组件                          │
│  └── Theme - 颜色主题                           │
└─────────────────────────────────────────────────┘
```

### 4.6 子阶段划分

| 子阶段 | 内容 | 时间 |
|--------|------|------|
| 2a | 基础字符网格渲染 (ASCII) | 3天 |
| 2b | Unicode/宽字符支持 | 3天 |
| 2c | 颜色和样式支持 | 2天 |
| 2d | 光标渲染 | 2天 |

### 4.7 里程碑验证

- ✅屏幕显示终端背景
- ✅ Mock 输出的"Hello World" 正确显示
- ✅ 光标可见且位置正确
- ✅ 支持基本ANSI 颜色
- ✅ 字体渲染清晰 (等宽字体)

---

## 五、Phase 3: SSH 接入 (2周)

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
用户输入 → 认证方式选择 → 执行认证│┌────────────────────┼────────────────────┐▼                    ▼                    ▼密码认证              公钥认证             Agent认证
         │                    │                    │
         └────────────────────┴────────────────────┘
                              │
                ▼
                      打开 PTY → 请求 Shell
```

### 5.4 里程碑验证

- ✅ 成功连接到真实 SSH 服务器
- ✅ 密码认证工作正常
- ✅ 公钥认证工作正常
- ✅运行 htop，画面流畅，颜色正确
- ✅ 运行 vim，编辑文件正常

---

## 六、Phase 4: 交互完善 (1周)

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

### 6.3 里程碑验证

- ✅ 拖拽窗口边缘，终端内容自适应
- ✅ Ctrl+Shift+C 复制选中内容
- ✅ Ctrl+Shift+V 粘贴剪贴板
- ✅ 鼠标点击定位光标 (支持的程序)
- ✅ 滚轮查看历史输出

---

## 七、Phase 5: 产品化 (2周)

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

### 7.3 UI 布局

```
┌─────────────────────────────────────────────────────────┐
│  [Tab1] [Tab2] [Tab3] [+][≡]            │  ← gpui_component::Tab
├──────────┬──────────────────────────────────────────────────┤
│          │                                                  │  ← gpui_component::Dock
│  主机列表 │              终端区域                            │
│ (Tree)   │           (TerminalView)                         │
│          │                                                  │
│▼生产环境│user@server:~$ ls -la                          │
│   Server1│  total 32│
│   Server2│  drwxr-xr-x5 user user4096 Jan1 00:00 .     │
│          │                                                  │
├──────────┴──────────────────────────────────────────────────┤
│  🟢 Connected | UTF-8 | 80x24                │  ← StatusBar
└─────────────────────────────────────────────────────────────┘
```

### 7.4 里程碑验证

- ✅ 可以添加、编辑、删除主机
- ✅ 双击主机快速连接
- ✅ 多标签页切换
- ✅ 分屏同时查看多个终端
- ✅ SFTP 上传下载文件
- ✅ 深色/浅色主题切换

---

## 八、技术风险与应对

| 风险 | 影响 | 应对策略 | 状态 |
|------|------|----------|------|
| Zed 渲染器理解困难 | Phase 2 延期 | 预留额外时间，逐步理解 |🟡 |
| russh 兼容性问题 | Phase 3阻塞 | 提前调研，准备 thrussh 备选 | ⚠️ |
| GPUI 学习曲线 | 整体延期 | gpui-component 示例丰富 | 🟢 |
| 性能问题 | 用户体验差 | 持续性能测试，及时优化 | ⚠️ |
| Unicode宽字符处理 | Phase 2 延期 | 参考 Zed 实现 | 🟡 |

---

## 九、性能目标

| 指标 | 目标值 | 测试方法 |
|------|--------|----------|
| 渲染延迟 | < 16ms (60fps) | 大量输出时测量帧率 |
| 输入延迟 | < 50ms | 按键到显示的时间 |
| 内存占用 | < 100MB (单会话) | 长时间运行监控 |
| 启动时间 | < 1s | 冷启动到可用 |

---

## 十、依赖清单

```toml
[dependencies]
# UI 框架
gpui = "0.2"
gpui-component = "0.4"

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

---

## 十一、相关文档

- [总体设计](./design.md) - 架构概述
- [五层架构](./architecture/layers.md) - 分层设计
- [TerminalConnection Trait](./core/connection-trait.md) - 核心接口
- [TerminalView](./modules/terminal-view.md) -渲染视图详情
- [SSH 后端](./modules/ssh-backend.md) - SSH 实现细节