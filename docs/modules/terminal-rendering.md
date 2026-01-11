# 终端渲染实现细节

> 基于 GPUI 的终端字符网格渲染完整实现指南

---

## 文档说明

本文档定义 **Zeterm 的终端渲染实现**，部分结构参考了 Zed 编辑器的终端实现。

| 文档 | 用途 |
|------|------|
| **本文档** | Zeterm 实际实现规范 |
| [Zed 终端分析](./zed-terminal-analysis.md) | Zed 源码分析参考 |

> 两个文档中的 `FontMetrics`、`BatchedTextRun` 等结构可能存在差异，以本文档为准。

---

## 一、渲染架构概述

### 1.1 组件层次

```
┌─────────────────────────────────────────────────────────────┐
│  TerminalView (gpui::Render)                                │
│  ├──实现 Render trait│
│  ├── 管理焦点状态                                           │
│  └── 分发键盘/鼠标事件                                │
├─────────────────────────────────────────────────────────────┤
│  TerminalElement (gpui::Element)                │
│  ├── request_layout(): 计算所需尺寸                         │
│  ├── prepaint(): 准备绘制数据                               │
│  └── paint(): 执行实际绘制                                  │
├─────────────────────────────────────────────────────────────┤
│  RenderableContent (来自 alacritty_terminal)                │
│  ├── display_iter(): 可见单元格迭代器│
│  ├── cursor: 光标位置和样式                                 │
│  └── selection: 选择范围                                    │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 渲染流程

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ request_layout│───►│   prepaint   │───►│    paint     │
│  计算尺寸    │    │  准备数据    │    │  绘制像素    │
└──────────────┘    └──────────────┘
       ││                   │
       ▼                   ▼                   ▼
  返回所需尺寸        计算字体度量         绘制背景
                构建文本批次         绘制字符
                     计算光标位置         绘制光标
                                          绘制选择
```

---

## 二、核心数据结构

### 2.1 字体度量 (FontMetrics)

```rust
/// 字体度量信息，用于计算单元格尺寸
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    /// 单元格宽度 (等宽字体的字符宽度)
    pub cell_width: Pixels,
    /// 单元格高度 (行高)
    pub cell_height: Pixels,
    /// 基线位置 (从单元格顶部到基线的距离)
    pub baseline: Pixels,
    /// 下降部分 (基线到字符最低点的距离)
    pub descent: Pixels,
    /// 上升部分 (基线到字符最高点的距离)
    pub ascent: Pixels,
}

impl FontMetrics {
    /// 从字体和字号计算度量信息
    pub fn calculate(
        font_family: &str,
        font_size: Pixels,
        line_height_ratio: f32,
        cx: &WindowContext,
    ) -> Self {
        // 1. 获取字体 ID
        let font_id = cx
            .text_system()
            .resolve_font(&TextStyle {
                font_family: font_family.into(),
                font_size,
                ..Default::default()
            });

        // 2. 获取字体度量
        let font_metrics = cx.text_system().font_metrics(font_id);
        
        // 3. 计算单元格宽度 (使用 'M' 作为参考字符)
        let cell_width = cx
            .text_system()
            .advance(font_id, 'M')
            .unwrap_or(font_size * 0.6);

        // 4. 计算行高
        let natural_line_height = font_metrics.ascent + font_metrics.descent;
        let cell_height = natural_line_height * line_height_ratio;

        // 5. 计算基线位置 (垂直居中)
        let extra_height = cell_height - natural_line_height;
        let baseline = font_metrics.ascent + (extra_height / 2.0);

        Self {
            cell_width,
            cell_height,
            baseline,
            descent: font_metrics.descent,
            ascent: font_metrics.ascent,
        }
    }

    /// 计算终端的行列数
    pub fn grid_dimensions(&self, size: Size<Pixels>) -> (u16, u16) {
        let cols = (size.width / self.cell_width).floor() as u16;
        let rows = (size.height / self.cell_height).floor() as u16;
        (rows.max(1), cols.max(1))
    }

    /// 计算单元格左上角的像素坐标
    pub fn cell_origin(&self, point: Point<i32>) -> Point<Pixels> {
        Point {
            x: px(point.x as f32) * self.cell_width,
            y: px(point.y as f32) * self.cell_height,
        }
    }

    /// 从像素坐标计算单元格位置
    pub fn point_for_position(&self, position: Point<Pixels>) -> Point<i32> {
        Point {
            x: (position.x / self.cell_width).floor() as i32,
            y: (position.y / self.cell_height).floor() as i32,
        }
    }
}
```

### 2.2 布局状态 (LayoutState)

```rust
/// prepaint 阶段计算的布局状态
pub struct LayoutState {
    /// 字体度量
    pub font_metrics: FontMetrics,
    /// 终端尺寸 (行x列)
    pub dimensions: TerminalSize,
    /// 背景矩形列表 (合并后)
    pub background_rects: Vec<BackgroundRect>,
    /// 批量文本运行(性能优化)
    pub text_runs: Vec<BatchedTextRun>,
    /// 光标信息
    pub cursor: Option<CursorLayout>,
    /// 选择区域
    pub selection_rects: Vec<SelectionRect>,
    /// 基础文本样式
    pub base_style: TextStyle,
}

/// 背景矩形(合并相邻同色单元格)
pub struct BackgroundRect {
    pub origin: Point<Pixels>,
    pub size: Size<Pixels>,
    pub color: Hsla,
}

/// 批量文本运行 (相同样式的连续字符)
pub struct BatchedTextRun {
    /// 起始单元格位置
    pub start_point: Point<i32>,
    /// 文本内容
    pub text: String,
    /// 单元格数量(考虑宽字符)
    pub cell_count: usize,
    /// 文本样式
    pub style: TextStyle,
}

impl BatchedTextRun {
    /// 判断是否可以追加字符到当前批次
    pub fn can_append(&self, cell: &RenderableCell, style: &TextStyle) -> bool {
        // 样式必须完全相同
        self.style == *style
            // 必须是相邻单元格
            && cell.point.column.0 == self.start_point.x + self.cell_count as i32
            && cell.point.line.0 == self.start_point.y
    }
}

/// 光标布局信息
pub struct CursorLayout {
    pub origin: Point<Pixels>,
    pub size: Size<Pixels>,
    pub style: CursorStyle,
    pub color: Hsla,/// 光标下的字符 (用于 Block 样式反色显示)
    pub text: Option<String>,
    pub text_color: Hsla,
}

/// 光标样式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CursorStyle {
    Block,      // 实心方块
    Beam,       // 竖线
    Underline,  // 下划线
    HollowBlock, // 空心方块 (失焦时)
}
```

---

## 三、Element 实现

### 3.1 完整的 TerminalElement 实现

```rust
use gpui::*;

pub struct TerminalElement {
    /// 会话协调器
    session: Model<SessionCoordinator>,
    /// 是否获得焦点
    focused: bool,
    /// 光标是否可见 (闪烁状态)
    cursor_visible: bool,
    /// 渲染配置
    config: TerminalRenderConfig,
}

pub struct TerminalRenderConfig {
    pub font_family: SharedString,
    pub font_size: Pixels,
    pub line_height: f32,
    pub cursor_style: CursorStyle,
    pub theme: TerminalTheme,
}

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> {
        Some("terminal-element".into())
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        cx: &mut WindowContext,
    ) -> (LayoutId, Self::RequestLayoutState) {
        // 请求填充所有可用空间
        let layout_id = cx.request_layout(Style {
            size: Size {
                width: relative(1.0).into(),
                height: relative(1.0).into(),
            },
            ..Default::default()
        }, []);
        
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        cx: &mut WindowContext,
    ) -> Self::PrepaintState {
        // 1. 计算字体度量
        let font_metrics = FontMetrics::calculate(
            &self.config.font_family,
            self.config.font_size,
            self.config.line_height,
            cx,
        );

        // 2. 计算终端尺寸
        let (rows, cols) = font_metrics.grid_dimensions(bounds.size);
        let dimensions = TerminalSize { rows, cols };

        // 3. 获取可渲染内容
        let content = self.session.read(cx).renderable_content(cx);

        // 4. 构建背景矩形 (合并优化)
        let background_rects = self.build_background_rects(
            &content,
            &font_metrics,
            &self.config.theme,
        );

        // 5. 构建文本批次 (批量优化)
        let text_runs = self.build_text_runs(
            &content,
            &font_metrics,
            &self.config.theme,
            cx,
        );

        // 6. 计算光标布局
        let cursor = self.build_cursor_layout(
            &content,
            &font_metrics,
            &self.config.theme,);

        // 7. 计算选择区域
        let selection_rects = self.build_selection_rects(
            &content,
            &font_metrics,
            &self.config.theme,
        );

        // 8. 基础文本样式
        let base_style = TextStyle {
            font_family: self.config.font_family.clone(),
            font_size: self.config.font_size,
            color: self.config.theme.foreground,
            ..Default::default()
        };

        LayoutState {
            font_metrics,
            dimensions,
            background_rects,
            text_runs,
            cursor,
            selection_rects,
            base_style,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        cx: &mut WindowContext,
    ) {
        // 1. 绘制终端背景
        cx.paint_quad(fill(bounds, self.config.theme.background));

        // 2. 绘制单元格背景 (非默认背景色的单元格)
        for rect in &prepaint.background_rects {
            let bounds = Bounds {
                origin: bounds.origin + rect.origin,
                size: rect.size,
            };
            cx.paint_quad(fill(bounds, rect.color));
        }

        // 3. 绘制选择区域
        for rect in &prepaint.selection_rects {
            let bounds = Bounds {
                origin: bounds.origin + rect.origin,
                size: rect.size,
            };
            cx.paint_quad(fill(bounds, self.config.theme.selection));
        }

        // 4. 绘制文本
        for run in &prepaint.text_runs {
            let origin = prepaint.font_metrics.cell_origin(run.start_point);
            let text_origin = Point {
                x: bounds.origin.x + origin.x,
                y: bounds.origin.y + origin.y + prepaint.font_metrics.baseline,
            };
            
            cx.paint_text(
                text_origin,
                &run.text,
                run.style.clone(),
            );
        }

        // 5. 绘制光标
        if let Some(cursor) = &prepaint.cursor {
            if self.cursor_visible {
                self.paint_cursor(bounds.origin, cursor, cx);
            }
        }
    }
}
```

### 3.2 背景矩形构建 (合并优化)

```rust
impl TerminalElement {
    fn build_background_rects(
        &self,
        content: &RenderableContent,
        metrics: &FontMetrics,
        theme: &TerminalTheme,
    ) -> Vec<BackgroundRect> {
        let mut rects = Vec::new();
        let mut current_rect: Option<(Point<i32>, i32, Hsla)> = None;

        for cell in content.display_iter() {
            let bg_color = convert_color(cell.bg, theme);
            // 跳过默认背景色
            if bg_color == theme.background {
                // 结束当前矩形
                if let Some((start, width, color)) = current_rect.take() {
                    rects.push(self.create_background_rect(start, width, color, metrics));
                }
                continue;
            }

            match &mut current_rect {
                Some((start, width, color)) => {
                    // 检查是否可以合并 (同一行、相邻、同色)
                    if start.y == cell.point.line.0
                        && start.x + *width == cell.point.column.0
                        && *color == bg_color
                    {
                        // 扩展当前矩形
                        *width += cell.width() as i32;
                    } else {
                        // 保存当前矩形，开始新矩形
                        rects.push(self.create_background_rect(*start, *width, *color, metrics));
                        *start = Point::new(cell.point.column.0, cell.point.line.0);
                        *width = cell.width() as i32;
                        *color = bg_color;
                    }
                }
                None => {
                    // 开始新矩形
                    current_rect = Some((
                        Point::new(cell.point.column.0, cell.point.line.0),
                        cell.width() as i32,
                        bg_color,
                    ));
                }
            }
        }

        // 处理最后一个矩形
        if let Some((start, width, color)) = current_rect {
            rects.push(self.create_background_rect(start, width, color, metrics));
        }

        rects
    }

    fn create_background_rect(
        &self,
        start: Point<i32>,
        width: i32,
        color: Hsla,
        metrics: &FontMetrics,
    ) -> BackgroundRect {
        BackgroundRect {
            origin: metrics.cell_origin(start),
            size: Size {
                width: metrics.cell_width * width as f32,
                height: metrics.cell_height,
            },
            color,
        }
    }
}
```

### 3.3 文本批次构建 (批量优化)

```rust
impl TerminalElement {
    fn build_text_runs(
        &self,
        content: &RenderableContent,
        metrics: &FontMetrics,
        theme: &TerminalTheme,
        cx: &WindowContext,
    ) -> Vec<BatchedTextRun> {
        let mut runs = Vec::new();
        let mut current_run: Option<BatchedTextRun> = None;

        for cell in content.display_iter() {
            // 跳过空格和占位符
            if cell.c == ' ' || cell.c == '\0' {
                if let Some(run) = current_run.take() {
                    runs.push(run);
                }
                continue;
            }

            //跳过宽字符的第二个单元格
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            // 构建文本样式
            let style = self.build_text_style(&cell, theme);

            match &mut current_run {
                Some(run) if run.can_append(&cell, &style) => {
                    // 追加到当前批次
                    run.text.push(cell.c);
                    run.cell_count += cell.width();
                }
                _ => {
                    // 保存当前批次，开始新批次
                    if let Some(run) = current_run.take() {
                        runs.push(run);
                    }
                    current_run = Some(BatchedTextRun {
                        start_point: Point::new(cell.point.column.0, cell.point.line.0),
                        text: cell.c.to_string(),
                        cell_count: cell.width(),
                        style,
                    });
                }
            }
        }

        // 处理最后一个批次
        if let Some(run) = current_run {
            runs.push(run);
        }

        runs
    }

    fn build_text_style(&self, cell: &RenderableCell, theme: &TerminalTheme) -> TextStyle {
        let mut style = TextStyle {
            font_family: self.config.font_family.clone(),
            font_size: self.config.font_size,
            color: convert_color(cell.fg, theme),
            ..Default::default()
        };

        // 处理文本属性
        if cell.flags.contains(Flags::BOLD) {
            style.font_weight = FontWeight::BOLD;
        }
        if cell.flags.contains(Flags::ITALIC) {
            style.font_style = FontStyle::Italic;
        }
        if cell.flags.contains(Flags::UNDERLINE) {
            style.underline = Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(style.color),
                wavy: false,
            });
        }
        if cell.flags.contains(Flags::STRIKETHROUGH) {
            style.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(style.color),
            });
        }
        if cell.flags.contains(Flags::INVERSE) {
            // 反色：交换前景和背景
            std::mem::swap(&mut style.color, &mut style.background_color.unwrap_or(theme.background));
        }

        style
    }
}
```

---

## 四、宽字符处理

### 4.1 宽字符检测

```rust
/// 判断字符是否为宽字符(占用2个单元格)
pub fn is_wide_char(c: char) -> bool {
    use unicode_width::UnicodeWidthChar;
    c.width().unwrap_or(0) > 1
}

/// 获取字符宽度 (单元格数)
pub fn char_width(c: char) -> usize {
    use unicode_width::UnicodeWidthChar;
    c.width().unwrap_or(1).max(1)
}
```

### 4.2 宽字符渲染

```rust
impl TerminalElement {
    fn layout_cell(
        &self,
        cell: &RenderableCell,
        metrics: &FontMetrics,) -> CellLayout {
        let width = cell.width();
        let is_wide = width > 1;

        //宽字符占用多个单元格宽度
        let cell_width = metrics.cell_width * width as f32;

        CellLayout {
            origin: metrics.cell_origin(Point::new(
                cell.point.column.0,
                cell.point.line.0,
            )),
            size: Size {
                width: cell_width,
                height: metrics.cell_height,
            },
            is_wide,char: cell.c,
        }
    }
}

/// 单元格布局信息
struct CellLayout {
    origin: Point<Pixels>,
    size: Size<Pixels>,
    is_wide: bool,
    char: char,
}
```

### 4.3 宽字符边界处理

```rust
/// 处理宽字符在行尾的截断
fn handle_wide_char_at_line_end(
    cell: &RenderableCell,
    cols: u16,
) -> Option<char> {
    let col = cell.point.column.0 as u16;
    let width = cell.width() as u16;

    // 如果宽字符会超出行尾，显示占位符
    if col + width > cols {
        Some('') // 或使用特殊占位符 '�'
    } else {
        Some(cell.c)
    }
}
```

---

## 五、颜色处理

### 5.1 颜色转换

```rust
use alacritty_terminal::vte::ansi::Color as AnsiColor;

/// 终端主题颜色
pub struct TerminalTheme {
    pub background: Hsla,
    pub foreground: Hsla,
    pub selection: Hsla,
    pub cursor: Hsla,
    pub cursor_text: Hsla,
    // 标准 16 色
    pub black: Hsla,
    pub red: Hsla,
    pub green: Hsla,
    pub yellow: Hsla,
    pub blue: Hsla,
    pub magenta: Hsla,
    pub cyan: Hsla,
    pub white: Hsla,
    pub bright_black: Hsla,
    pub bright_red: Hsla,
    pub bright_green: Hsla,
    pub bright_yellow: Hsla,
    pub bright_blue: Hsla,
    pub bright_magenta: Hsla,
    pub bright_cyan: Hsla,
    pub bright_white: Hsla,
}

/// 将ANSI 颜色转换为 HSLA
pub fn convert_color(color: AnsiColor, theme: &TerminalTheme) -> Hsla {
    match color {
        AnsiColor::Named(named) => convert_named_color(named, theme),
        AnsiColor::Spec(rgb) => rgb_to_hsla(rgb.r, rgb.g, rgb.b),AnsiColor::Indexed(index) => convert_indexed_color(index, theme),
    }
}

fn convert_named_color(named:NamedColor, theme: &TerminalTheme) -> Hsla {
    match named {
        NamedColor::Black => theme.black,
        NamedColor::Red => theme.red,
        NamedColor::Green => theme.green,
        NamedColor::Yellow => theme.yellow,
        NamedColor::Blue => theme.blue,
        NamedColor::Magenta => theme.magenta,
        NamedColor::Cyan => theme.cyan,
        NamedColor::White => theme.white,
        NamedColor::BrightBlack => theme.bright_black,
        NamedColor::BrightRed => theme.bright_red,
        NamedColor::BrightGreen => theme.bright_green,
        NamedColor::BrightYellow => theme.bright_yellow,
        NamedColor::BrightBlue => theme.bright_blue,
        NamedColor::BrightMagenta => theme.bright_magenta,
        NamedColor::BrightCyan => theme.bright_cyan,
        NamedColor::BrightWhite => theme.bright_white,NamedColor::Foreground => theme.foreground,
        NamedColor::Background => theme.background,
        NamedColor::Cursor => theme.cursor,
        _ => theme.foreground,
    }
}

/// 256 色索引转换
fn convert_indexed_color(index: u8, theme: &TerminalTheme) -> Hsla {
    match index {
        // 标准 16 色 (0-15)
        0 => theme.black,
        1 => theme.red,
        2 => theme.green,
        3 => theme.yellow,
        4 => theme.blue,
        5 => theme.magenta,
        6 => theme.cyan,
        7 => theme.white,
        8 => theme.bright_black,
        9 => theme.bright_red,
        10 => theme.bright_green,
        11 => theme.bright_yellow,
        12 => theme.bright_blue,
        13 => theme.bright_magenta,
        14 => theme.bright_cyan,
        15 => theme.bright_white,
        // 216 色立方体 (16-231)
        16..=231 => {
            let index = index - 16;
            let r = (index / 36) % 6;
            let g = (index / 6) % 6;
            let b = index % 6;
            
            let r = if r > 0 { r * 40 + 55 } else { 0 };
            let g = if g > 0 { g * 40 + 55 } else { 0 };
            let b = if b > 0 { b * 40 + 55 } else { 0 };
            
            rgb_to_hsla(r, g, b)
        }
        
        // 24 级灰度 (232-255)
        232..=255 => {
            let gray = (index - 232) * 10 + 8;
            rgb_to_hsla(gray, gray, gray)
        }}
}

/// RGB 转 HSLA
fn rgb_to_hsla(r: u8, g: u8, b: u8) -> Hsla {Hsla::from(Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    })
}
```

---

## 六、光标渲染

### 6.1 光标布局计算

```rust
impl TerminalElement {
    fn build_cursor_layout(
        &self,
        content: &RenderableContent,
        metrics: &FontMetrics,
        theme: &TerminalTheme,
    ) -> Option<CursorLayout> {
        let cursor = content.cursor?;
        let point = cursor.point;
        
        let origin = metrics.cell_origin(Point::new(
            point.column.0,
            point.line.0,
        ));

        // 获取光标下的字符
        let cell = content.display_iter()
            .find(|c| c.point == point);
        
        let (text, cell_width) = if let Some(cell) = cell {
            (Some(cell.c.to_string()), cell.width())
        } else {
            (None, 1)
        };

        let style = if self.focused {
            self.config.cursor_style
        } else {
            CursorStyle::HollowBlock // 失焦时显示空心方块
        };

        let size = match style {
            CursorStyle::Block | CursorStyle::HollowBlock => Size {
                width: metrics.cell_width * cell_width as f32,
                height: metrics.cell_height,
            },
            CursorStyle::Beam => Size {
                width: px(2.0),
                height: metrics.cell_height,
            },
            CursorStyle::Underline => Size {
                width: metrics.cell_width * cell_width as f32,
                height: px(2.0),
            },
        };

        Some(CursorLayout {
            origin,
            size,
            style,
            color: theme.cursor,text,
            text_color: theme.cursor_text,
        })
    }
}
```

### 6.2 光标绘制

```rust
impl TerminalElement {
    fn paint_cursor(
        &self,
        bounds_origin: Point<Pixels>,
        cursor: &CursorLayout,
        cx: &mut WindowContext,
    ) {
        let origin = bounds_origin + cursor.origin;

        match cursor.style {
            CursorStyle::Block => {
                // 实心方块
                let bounds = Bounds {
                    origin,
                    size: cursor.size,
                };
                cx.paint_quad(fill(bounds, cursor.color));

                // 绘制反色文字
                if let Some(text) = &cursor.text {
                    let text_origin = Point {
                        x: origin.x,
                        y: origin.y + prepaint.font_metrics.baseline,
                    };
                    cx.paint_text(text_origin, text, TextStyle {
                        color: cursor.text_color,
                        ..prepaint.base_style.clone()
                    });
                }
            CursorStyle::HollowBlock => {
                // 空心方块 (失焦时)
                let bounds = Bounds {
                    origin,
                    size: cursor.size,
                };
                cx.paint_quad(outline(bounds, cursor.color, px(1.0)));
            }
            CursorStyle::Beam => {
                // 竖线光标
                let bounds = Bounds {
                    origin,
                    size: cursor.size,
                };
                cx.paint_quad(fill(bounds, cursor.color));
            }
            CursorStyle::Underline => {
                // 下划线光标
                let bounds = Bounds {
                    origin: Point {
                        x: origin.x,
                        y: origin.y + cursor.size.height - px(2.0),
                    },
                    size: cursor.size,
                };
                cx.paint_quad(fill(bounds, cursor.color));
            }
        }
    }
}
```

---

## 七、性能优化策略

### 7.1 批量渲染优化

```rust
/// 性能优化：批量文本渲染
///
/// 原理：将相同样式的连续字符合并为单次绘制调用
/// 效果：减少 GPU draw call 数量，提升渲染性能
/// 
/// 优化前：每个字符一次 draw call
/// 优化后：相同样式的连续字符一次 draw call
impl BatchedTextRun {
    /// 合并相邻的文本运行
    pub fn merge_runs(runs: Vec<BatchedTextRun>) -> Vec<BatchedTextRun> {
        let mut merged = Vec::with_capacity(runs.len());
        for run in runs {
            if let Some(last) = merged.last_mut() {
                if last.can_merge(&run) {
                    last.text.push_str(&run.text);
                    last.cell_count += run.cell_count;
                    continue;
                }
            }
            merged.push(run);
        }
        
        merged
    }
    
    fn can_merge(&self, other: &BatchedTextRun) -> bool {
        self.style == other.style
            && self.start_point.y == other.start_point.y
            && self.start_point.x + self.cell_count as i32 == other.start_point.x
    }
}
```

### 7.2 脏区域更新

```rust
/// 脏区域追踪，只重绘变化的部分
pub struct DirtyRegion {
    /// 脏行范围
    dirty_lines: RangeSet<i32>,
    /// 是否需要全量重绘
    full_repaint: bool,
}

impl DirtyRegion {
    pub fn new() -> Self {
        Self {
            dirty_lines: RangeSet::new(),
            full_repaint: true, // 首次需要全量绘制
        }
    }
    
    /// 标记行为脏
    pub fn mark_dirty(&mut self, line: i32) {
        self.dirty_lines.insert(line);}
    
    /// 标记行范围为脏
    pub fn mark_range_dirty(&mut self, start: i32, end: i32) {
        for line in start..=end {
            self.dirty_lines.insert(line);
        }
    }
    
    /// 标记全量重绘
    pub fn mark_full_repaint(&mut self) {
        self.full_repaint = true;
    }
    
    /// 检查是否需要重绘
    pub fn needs_repaint(&self, line: i32) -> bool {
        self.full_repaint || self.dirty_lines.contains(&line)
    }
    
    /// 清除脏标记
    pub fn clear(&mut self) {
        self.dirty_lines.clear();
        self.full_repaint = false;
    }
}
```

### 7.3 视口裁剪

```rust
/// 只渲染可见区域内的内容
fn visible_cells<'a>(
    content: &'a RenderableContent,
    viewport: Bounds<Pixels>,
    metrics: &FontMetrics,
) -> impl Iterator<Item = &'a RenderableCell> {
    let start_row = (viewport.origin.y / metrics.cell_height).floor() as i32;
    let end_row = ((viewport.origin.y + viewport.size.height) / metrics.cell_height).ceil() as i32;
    
    content.display_iter().filter(move |cell| {
        let row = cell.point.line.0;
        row >= start_row && row <= end_row
    })
}
```

### 7.4 帧率控制

```rust
/// 帧率限制器，避免过度渲染
pub struct FrameRateLimiter {
    last_frame: Instant,
    min_frame_interval: Duration,
    pending_notify: bool,
}

impl FrameRateLimiter {
    pub fn new(target_fps: u32) -> Self {
        Self {
            last_frame: Instant::now(),
            min_frame_interval: Duration::from_secs_f64(1.0 / target_fps as f64),
            pending_notify: false,
        }
    }
    
    /// 请求重绘，返回是否应该立即重绘
    pub fn request_frame(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now - self.last_frame;
        
        if elapsed >= self.min_frame_interval {
            self.last_frame = now;
            self.pending_notify = false;
            true
        } else {
            self.pending_notify = true;
            false
        }
    }
    /// 检查是否有待处理的重绘请求
    pub fn has_pending(&self) -> bool {
        self.pending_notify
    }
}
```

---

## 八、选择区域渲染

### 8.1 选择区域计算

```rust
impl TerminalElement {
    fn build_selection_rects(
        &self,
        content: &RenderableContent,
        metrics: &FontMetrics,
        theme: &TerminalTheme,
    ) -> Vec<SelectionRect> {
        let selection = match &content.selection {
            Some(sel) => sel,
            None => return Vec::new(),
        };
        
        let mut rects = Vec::new();
        let (start, end) = selection.to_range();
        
        // 处理每一行的选择
        for line in start.line.0..=end.line.0 {
            let (start_col, end_col) = if line == start.line.0 && line == end.line.0 {
                // 单行选择
                (start.column.0, end.column.0)
            } else if line == start.line.0 {
                // 选择起始行
                (start.column.0, content.grid.columns() as i32)
            } else if line == end.line.0 {
                // 选择结束行
                (0, end.column.0)
            } else {
                // 中间行，全选
                (0, content.grid.columns() as i32)
            };
            
            let origin = metrics.cell_origin(Point::new(start_col, line));
            let width = (end_col - start_col) as f32 * metrics.cell_width;
            
            rects.push(SelectionRect {
                origin,
                size: Size {
                    width,
                    height: metrics.cell_height,
                },});
        }
        
        rects
    }
}

pub struct SelectionRect {
    pub origin: Point<Pixels>,
    pub size: Size<Pixels>,
}
```

---

## 九、相关文档

- [TerminalView](./terminal-view.md) - 视图组件设计
- [Zed 终端分析](./zed-terminal-analysis.md) - Zed 实现参考
- [数据流设计](../architecture/data-flow.md) -渲染数据流
- [按键映射](./key-mappings.md) - 键盘输入处理