#TerminalView 渲染视图

> 基于 GPUI 的终端渲染组件，参考 Zed 编辑器终端实现

---

## 一、设计概述

### 1.1 组件分工

| 组件 | 来源 | 用途 |
|------|------|------|
| **终端渲染器** | 参考 Zed `terminal_view` | 字符网格绘制、光标、选择 |
| **窗口 UI 组件** | gpui-component | Tab、Dock、Modal、按钮等 |

>⚠️ **重要**: 终端渲染器需要自行实现（参考 Zed），gpui-component 不提供终端渲染能力。

### 1.2 设计目标

| 目标 | 说明 |
|------|------|
| 高性能渲染 | GPU 加速的字符网格绘制 |
| 完整终端支持 | 256色、TrueColor、Unicode、宽字符 |
| 交互响应 | 键盘、鼠标、选择、滚动 |
| 可定制 | 字体、颜色、光标样式 |

---

## 二、Zed 终端实现参考

### 2.1 关键源文件

```
zed/crates/terminal_view/src/
├── terminal_view.rs      # 主视图组件
├── terminal_element.rs   # 渲染元素 (核心)
└── persistence.rs        # 状态持久化

zed/crates/terminal/src/
├── terminal.rs           # 终端模型
└── mappings/             # 按键映射
```

### 2.2 核心渲染流程 (参考 Zed)

```
┌─────────────────────────────────────────────────────────┐
│  TerminalView (gpui::Render)                            │
│  └──实现 render() 方法                                 │
├─────────────────────────────────────────────────────────┤
│  TerminalElement (gpui::Element)                        │
│  ├── prepaint(): 计算布局、字体度量                │
│  └── paint(): 绘制背景、字符、光标、选择                │
├─────────────────────────────────────────────────────────┤
│  alacritty_terminal::Term│
│  └── 提供 renderable_content()│
└─────────────────────────────────────────────────────────┘
```

---

## 三、组件结构

### 3.1 TerminalView

```rust
pub struct TerminalView {
    session: Model<SessionCoordinator>,  // 会话协调器
    element: TerminalElement,            // 渲染元素
    focus_handle: FocusHandle,           // 焦点管理
}
```

### 3.2 TerminalElement

```rust
pub struct TerminalElement {
    font_metrics: FontMetrics,    // 字体度量
    selection: Option<Selection>, // 选择状态
    scroll_offset: f32,           // 滚动偏移
    config: TerminalViewConfig,   // 渲染配置
}

pub struct FontMetrics {
    cell_width: Pixels,   // 单元格宽度
    cell_height: Pixels,  // 单元格高度
    baseline: Pixels,     // 基线位置
}
```

---

## 四、渲染实现要点

### 4.1 Render Trait 实现

```rust
impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.session.read(cx).renderable_content(cx);
        div()
            .size_full()
            .child(self.element.clone().with_content(content))
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))}
}
```

### 4.2 Element 绘制流程

|阶段 | 方法 | 职责 |
|------|------|------|
| 1.预绘制 | `prepaint()` | 计算字体度量、布局尺寸 |
| 2. 绘制背景 | `paint_background()` | 填充终端背景色 |
| 3. 绘制选择 | `paint_selection()` | 高亮选中区域 |
| 4. 绘制字符 | `paint_cells()` | 逐单元格绘制字符 |
| 5. 绘制光标 | `paint_cursor()` | 绘制光标 (Block/Beam/Underline) |

### 4.3 字符绘制核心逻辑

```
for cell in content.display_iter:
    position = calculate_cell_position(cell.point)
    
    if cell.bg != default_bg:
        paint_cell_background(position, cell.bg)
    
    if cell.c != ' ':
        paint_character(position, cell.c, cell.fg, cell.flags)
```

---

## 五、事件处理

### 5.1 键盘输入

| 按键 | ANSI 序列 |
|------|-----------|
| Enter | `\r` |
| Backspace | `\x7f` |
| Tab | `\t` |
| Escape | `\x1b` |
| Arrow Up | `\x1b[A` |
| Arrow Down | `\x1b[B` |
| Ctrl+C | `\x03` |

### 5.2 鼠标处理

| 事件 | 处理 |
|------|------|
| 左键按下 | 开始选择 |
| 拖动 | 更新选择范围 |
| 左键释放 | 结束选择 |
| 双击 | 选择单词 |
| 三击 | 选择整行 |
| 滚轮 | 滚动历史 |

---

## 六、布局计算

### 6.1 尺寸计算

```
cols = floor(view_width / cell_width)
rows = floor(view_height / cell_height)
```

### 6.2 Resize 流程

1. 检测视图尺寸变化
2. 计算新的行列数
3. 更新`TerminalState` (Alacritty)
4. 通知远端 (`ConnectionManager.resize()`)
5. 触发重绘

---

## 七、配置项

| 配置 | 默认值 | 说明 |
|------|--------|------|
| `font_family` | "JetBrains Mono" | 字体族 (等宽) |
| `font_size` | 14.0 | 字体大小 |
| `line_height` | 1.2 | 行高倍数 |
| `cursor_style` | Block | 光标样式 |
| `cursor_blink` | true | 光标闪烁 |
| `scrollback_lines` | 10000 | 滚动缓冲区 |

---

## 八、与 gpui-component 的集成

```rust
//窗口布局使用 gpui-component
fn render_workspace(&self, cx: &mut Context<Self>) -> impl IntoElement {
    Root::new(
        v_flex()
            .child(Tab::new(...))           // gpui-component:标签栏
            .child(Dock::new()                  // gpui-component: 分屏布局
                    .left(HostTreeView {})// gpui-component: 主机列表
                    .center(TerminalView {}) //自实现: 终端渲染
            )
            .child(StatusBar::new(...)),    // gpui-component: 状态栏
        window, cx
    )
}
```

---

## 九、实现优先级

| 优先级 | 功能 | Phase |
|--------|------|-------|
| P0 | 基础字符渲染 | 2 |
| P0 | 光标显示 | 2 |
| P0 | 键盘输入 | 2 |
| P1 | 256色/TrueColor | 2 |
| P1 |鼠标选择 | 4 |
| P1 | 滚动缓冲区 | 4 |
| P2 | 光标闪烁 | 4 |
| P2 | 搜索高亮 | 4 |

---

## 十、相关文档

- [SessionModel](./session-model.md) - 会话模型
- [数据流设计](../architecture/data-flow.md) -渲染数据流
- [实现路径](../roadmap.md) - Phase 2 详情
- [Zed Terminal 源码](https://github.com/zed-industries/zed/tree/main/crates/terminal_view)