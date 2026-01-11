# TerminalView 渲染视图

> 基于 GPUI 的终端渲染组件，移植自 Zed 编辑器

---

## 一、设计目标

1. **高性能渲染** - GPU 加速的字符网格绘制
2. **完整终端支持** - 256色、TrueColor、Unicode
3. **交互响应** - 键盘、鼠标、选择、滚动
4. **可定制** - 字体、颜色、光标样式

---

## 二、组件结构

```rust
use gpui::{View, ViewContext, Render};

/// 终端渲染视图
pub struct TerminalView {
    /// 关联的会话模型
    session: Model<SessionModel>,
    
    /// 渲染配置
    config: TerminalViewConfig,
    
    /// 选择状态
    selection: Option<Selection>,
    
    /// 滚动偏移
    scroll_offset: f32,
    
    /// 是否获得焦点
    focused: bool,
    
    /// 字体度量信息
    font_metrics: FontMetrics,
}

/// 视图配置
#[derive(Debug, Clone)]
pub struct TerminalViewConfig {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub cursor_style: CursorStyle,
    pub cursor_blink: bool,
    pub theme: TerminalTheme,
}
```

---

## 三、渲染流程

### 3.1 Render 实现

```rust
impl Render for TerminalView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let content = self.session.read(cx).renderable_content();
        let size = self.session.read(cx).size();
        
        div()
            .size_full()
            .bg(self.config.theme.background)
            .child(
                canvas(
                    move |bounds, cx| self.prepaint(bounds, cx),
                    move |bounds, cx| self.paint(bounds, &content, cx),
                ))
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))}
}
```

### 3.2 绘制方法

```rust
impl TerminalView {
    fn paint(
        &self,
        bounds: Bounds<Pixels>,
        content: &RenderableContent,
        cx: &mut PaintContext,
    ) {
        // 1. 绘制背景
        self.paint_background(bounds, cx);
        
        // 2. 绘制选择区域
        if let Some(selection) = &self.selection {
            self.paint_selection(bounds, selection, cx);
        }
        
        // 3. 绘制字符网格
        self.paint_cells(bounds, content, cx);
        
        // 4. 绘制光标
        self.paint_cursor(bounds, content.cursor, cx);
    }
    fn paint_cells(
        &self,
        bounds: Bounds<Pixels>,
        content: &RenderableContent,
        cx: &mut PaintContext,
    ) {
        for indexed_cell in content.display_iter {
            let point = indexed_cell.point;
            let cell = &indexed_cell.cell;
            
            // 计算单元格位置
            let x = bounds.origin.x + point.column as f32 * self.font_metrics.cell_width;
            let y = bounds.origin.y + point.line as f32 * self.font_metrics.cell_height;
            
            // 绘制背景色
            if cell.bg != self.config.theme.background {
                self.paint_cell_background(x, y, cell.bg, cx);
            }
            
            // 绘制字符
            if cell.c != ' ' {
                self.paint_character(x, y, cell, cx);
            }
        }
    }
}
```

---

## 四、事件处理

### 4.1 键盘输入

```rust
impl TerminalView {
    fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut ViewContext<Self>) {
        // 转换为 ANSI 序列
        let bytes = match &event.keystroke.key {
            Key::Enter => b"\r".to_vec(),
            Key::Backspace => b"\x7f".to_vec(),
            Key::Tab => b"\t".to_vec(),
            Key::Escape => b"\x1b".to_vec(),
            Key::ArrowUp => b"\x1b[A".to_vec(),
            Key::ArrowDown => b"\x1b[B".to_vec(),
            Key::ArrowRight => b"\x1b[C".to_vec(),
            Key::ArrowLeft => b"\x1b[D".to_vec(),
            Key::Char(c) => {
                if event.keystroke.modifiers.control {
                    // Ctrl+字母-> 控制字符
                    vec![(*c as u8) & 0x1f]
                } else {
                    c.to_string().into_bytes()
                }
            }
            _ => return,
        };
        
        // 发送到会话
        self.session.update(cx, |model, cx| {
            model.send_input(&bytes, cx);
        });
    }
}
```

### 4.2 鼠标处理

```rust
impl TerminalView {
    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        cx: &mut ViewContext<Self>,
    ) {
        // 计算点击的单元格位置
        let cell_pos = self.pixel_to_cell(event.position);
        
        // 开始选择
        self.selection = Some(Selection::new(cell_pos));
        
        cx.notify();
    }
    
    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut ViewContext<Self>,
    ) {
        if let Some(selection) = &mut self.selection {
            let cell_pos = self.pixel_to_cell(event.position);
            selection.update_end(cell_pos);
            cx.notify();
        }
    }
}
```

---

## 五、布局计算

```rust
impl TerminalView {
    /// 计算终端尺寸
    fn calculate_dimensions(&self, bounds: Bounds<Pixels>) ->TerminalSize {
        let cols = (bounds.size.width / self.font_metrics.cell_width).floor() as u16;
        let rows = (bounds.size.height / self.font_metrics.cell_height).floor() as u16;
        
        TerminalSize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: bounds.size.width as u16,
            pixel_height: bounds.size.height as u16,
        }
    }
    
    /// 处理布局变化
    fn handle_layout_changed(&mut self, bounds: Bounds<Pixels>, cx: &mut ViewContext<Self>) {
        let new_size = self.calculate_dimensions(bounds);
        let current_size = self.session.read(cx).size();
        
        if new_size != current_size {
            self.session.update(cx, |model, cx| {
                model.resize(new_size, cx);
            });
        }
    }
}
```

---

## 六、移植指南

### 6.1 从 Zed 提取的文件

```
zed/crates/terminal_view/
├── src/
│   ├── terminal_view.rs    → 核心渲染逻辑
│   ├── terminal_element.rs → 绘制实现
│   └── ...

需要移除的依赖:
- workspace
- project
- language
- settings (替换为自己的配置)
- theme (简化为终端主题)
```

### 6.2 修改要点

1. 替换 `Model<Terminal>` 为 `Model<SessionModel>`
2. 移除 Zed 特有的上下文菜单
3. 简化主题系统
4. 调整快捷键绑定

---

## 七、相关文档

- [SessionModel](./session-model.md) - 会话模型
- [数据流设计](../architecture/data-flow.md) - 渲染数据流
- [实现路径](../roadmap.md) - Phase 2 详情