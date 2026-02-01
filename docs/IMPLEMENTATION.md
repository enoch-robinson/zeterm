# Zeterm 实现指南

> 开发阶段规划与技术要点

---

## 一、阶段总览

```
Phase 1        Phase 2        Phase 3        Phase 4        Phase 5
核心骨架   ──►终端渲染   ──►  SSH 接入   ──►  交互完善  ──►  产品化
 (2周)          (2周)          (2周)          (1周)          (2周)
```

---

## 二、各阶段详情

### Phase 1: 核心骨架

**目标**: 搭建项目基础架构，验证核心抽象设计

**关键任务**:
- Cargo workspace 结构搭建
- GPUI 基础窗口集成
- `TerminalConnection` Trait 定义
- `MockConnection` 实现
- `alacritty_terminal` 集成
- `SessionCoordinator` 实现

**里程碑**:
- ✅ GPUI 窗口正常显示
- ✅ MockConnection 每秒输出 "Hello World"
- ✅ Alacritty 解析内容正确打印

---

### Phase 2: 终端渲染

**目标**: 实现终端字符网格渲染

**关键任务**:
- 研究 Zed `terminal_view` / `terminal_element`
- 实现 `TerminalElement` (GPUI Element)
- 字体度量计算 (等宽字体单元格尺寸)
- ASCII/Unicode/宽字符支持
- 256色/TrueColor 支持
- 光标渲染 (Block/Beam/Underline)

**实现要点**:
```rust
// TerminalView 渲染流程
TerminalView (Render)
  └── TerminalElement (Element)
       ├── prepaint(): 计算字体度量
       └── paint(): 绘制背景、字符、光标
```

**里程碑**:
- ✅ "Hello World" 正确显示
- ✅ 光标位置正确
- ✅ 基本 ANSI 颜色支持

---

### Phase 3: SSH 接入

**目标**: 实现真实 SSH 连接

**关键任务**:
- 集成 `russh` SSH 协议库
- 实现 `SshConnection` (TerminalConnection)
- 密码/公钥/Agent 认证
- 主机密钥验证 (`known_hosts`)
- 连接状态机

**认证流程**:
```
用户输入 → 认证方式选择 → 执行认证
                              │
         ┌────────────────────┼────────────────────┐
         ▼                    ▼                    ▼
    密码认证              公钥认证             Agent认证
         │                    │                    │
         └────────────────────┴────────────────────┘
                              │
                              ▼
                        打开 PTY → 请求 Shell
```

**里程碑**:
- ✅ 成功连接真实 SSH 服务器
- ✅ htop/vim 运行正常

---

### Phase 4: 交互完善

**目标**: 完善用户交互体验

**关键任务**:
- 窗口 Resize (同步终端尺寸)
- 复制粘贴 (系统剪贴板集成)
- 鼠标支持 (点击、选择、滚轮)
- 滚动缓冲区
- 快捷键系统

**快捷键**:
| 功能 | 快捷键 |
|------|--------|
| 新建 Tab | `Ctrl+Shift+T` |
| 关闭 Tab | `Ctrl+Shift+W` |
| 水平分屏 | `Ctrl+\` |
| 垂直分屏 | `Ctrl+Shift+-` |
| 复制 | `Ctrl+Shift+C` |
| 粘贴 | `Ctrl+Shift+V` |

---

### Phase 5: 产品化

**目标**: 完成产品级功能

**关键任务**:
- 主机列表管理 (侧边栏)
- Tab 管理 (自实现)
- 分屏布局 (自实现)
- SFTP 文件管理
- 主题系统
- 数据持久化 (SQLite)

**UI 布局**:
```
┌─────────────────────────────────────────────────────────┐
│  [生产环境] [开发环境] [测试环境] [+]                   │  ← TabView (自实现)
├──────────┬──────────────────────────────────────────────┤
│          │ ┌───────────────┬───────────────┐            │
│  主机列表 │ │   Server1     │   Server2     │            │  ← SplitView
│ (Tree)   │ │   (Terminal)  │   (Terminal)  │            │
│          │ ├───────────────┴───────────────┤            │
│ ▼生产环境│ │         Server3               │            │
│   Server1│ │         (Terminal)            │            │
├──────────┴──────────────────────────────────────────────┤
│  🟢 Connected | root@Server1 | UTF-8 | 80x24            │  ← StatusBar
└─────────────────────────────────────────────────────────┘
```

**设计决策**:
- Tab/分屏采用**自实现方案**（非 gpui-component）
- 原因: 终端场景特殊需求（字符网格对齐、PTY resize、焦点管理）

---

## 三、技术要点

### 3.1 线程模型

| 线程 | 职责 | 特点 |
|------|------|------|
| GPUI 主线程 | UI 渲染、事件处理 | <16ms 响应 |
| Tokio Runtime | 网络 IO、SSH 协议 | 多线程并发 |

### 3.2 数据流

```
输入: 用户按键 → TerminalView → ANSI序列 → SessionCoordinator → SSH → 远端
输出: 远端数据 → SSH → Data Pump → TerminalState → cx.notify() → 重绘
```

### 3.3 组件实现方式

| 组件 | 实现方式 | 说明 |
|------|----------|------|
| TerminalView | 自实现 | 参考 Zed terminal_view |
| TabView | 自实现 | 多标签页管理 |
| SplitView | 自实现 | 分屏布局 |
| Modal/Input | gpui-component | 对话框组件 |
| Theme | gpui-component | 颜色主题 |

---

## 四、性能目标

| 指标 | 目标值 |
|------|--------|
| 渲染延迟 | < 16ms (60fps) |
| 输入延迟 | < 50ms |
| 内存占用 | < 100MB (单会话) |
| 启动时间 | < 1s |

---

## 五、相关文档

- [ARCHITECTURE.md](./ARCHITECTURE.md) - 架构设计
- [API.md](./API.md) - 接口定义
- [USER_GUIDE.md](./USER_GUIDE.md) - 用户指南