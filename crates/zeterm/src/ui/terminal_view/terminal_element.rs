//! 终端渲染元素
//!
//! 基于 GPUI Element trait 的终端渲染实现。
//! 参考 Zed 编辑器的 terminal_element.rs 实现。
//!
//! ## 性能优化
//!
//! 本模块实现了以下性能优化：
//! - **BatchedTextRun**: 将相邻的、样式相同的字符合并成批次，减少 shape_line 调用
//! - **LayoutRect**: 合并相邻的背景区域，减少 paint_quad 调用
//! - **prepaint/paint 分离**: 预处理与绘制分离，提高代码可维护性

use std::sync::Arc;

use super::fonts;

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, CursorShape, NamedColor};
use gpui::{
    App, Bounds, Element, ElementId, Font, GlobalElementId, Hsla, IntoElement, LayoutId, Pixels,
    Point, SharedString, Size, StrikethroughStyle, Style, TextRun, UnderlineStyle, Window, fill,
    px,
};
use gpui_component::ActiveTheme;
use tracing::debug;

use crate::app::session::SessionCoordinator;

//============================================================================
// 常量定义
// ============================================================================

/// 终端字体大小
pub const TERMINAL_FONT_SIZE: f32 = 14.0;
/// 终端行高倍数
pub const TERMINAL_LINE_HEIGHT: f32 = 1.2;

// ============================================================================
// 核心数据结构
// ============================================================================

/// 终端渲染元素
pub struct TerminalElement {
    /// 会话协调器
    coordinator: Arc<SessionCoordinator>,
    /// 是否聚焦
    focused: bool,
    /// 光标是否可见
    cursor_visible: bool,
    /// 字体大小
    font_size: f32,
    /// 行高倍数
    line_height: f32,
    /// 光标颜色（可选，None时使用默认绿色）
    cursor_color: Option<Hsla>,
}

/// 字体度量信息
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// 单元格宽度
    pub cell_width: Pixels,
    /// 单元格高度 (行高)
    pub cell_height: Pixels,
    /// 字体大小
    pub font_size: Pixels,
}

impl Default for FontMetrics {
    fn default() -> Self {
        let font_size = px(TERMINAL_FONT_SIZE);
        Self {
            cell_width: font_size * 0.6,
            cell_height: font_size * TERMINAL_LINE_HEIGHT,
            font_size,
        }
    }
}

// ============================================================================
// 批处理优化数据结构
// ============================================================================

/// 批量文本运行
///
/// 将相邻的、样式相同的字符合并成一个批次，大幅减少绘制调用。
/// 例如 "hello" 这5个字符如果样式相同，会合并成一个 BatchedTextRun。
#[derive(Debug, Clone)]
pub struct BatchedTextRun {
    /// 起始位置 (行, 列)
    pub start_line: i32,
    pub start_col: i32,
    /// 合并后的文本
    pub text: String,
    /// 占用的单元格数（考虑宽字符）
    pub cell_count: usize,
    /// 文本样式
    pub style: TextRunStyle,
}

/// 文本运行样式
///
/// 用于比较两个字符是否可以合并到同一批次
#[derive(Debug, Clone, PartialEq)]
pub struct TextRunStyle {
    /// 字体
    pub font: Font,
    /// 前景色
    pub color: Hsla,
    /// 下划线样式
    pub underline: Option<UnderlineStyle>,
    /// 删除线样式
    pub strikethrough: Option<StrikethroughStyle>,
}

impl BatchedTextRun {
    /// 创建新的批量文本运行
    pub fn new(line: i32, col: i32, c: char, cell_width: usize, style: TextRunStyle) -> Self {
        Self {
            start_line: line,
            start_col: col,
            text: c.to_string(),
            cell_count: cell_width,
            style,
        }
    }

    /// 检查是否可以追加新字符（样式必须相同且位置相邻）
    pub fn can_append(&self, line: i32, col: i32, style: &TextRunStyle) -> bool {
        // 必须在同一行
        if self.start_line != line {
            return false;
        }
        // 必须紧邻当前批次的末尾
        let expected_col = self.start_col + self.cell_count as i32;
        if col != expected_col {
            return false;
        }
        // 样式必须相同
        self.style == *style
    }

    /// 追加字符到批次
    pub fn append_char(&mut self, c: char, cell_width: usize) {
        self.text.push(c);
        self.cell_count += cell_width;
    }

    /// 绘制批量文本
    pub fn paint(
        &self,
        origin: Point<Pixels>,
        font_metrics: &FontMetrics,
        window: &mut Window,
        cx: &mut App,
    ) {
        let pos = Point::new(
            origin.x + self.start_col as f32 * font_metrics.cell_width,
            origin.y + self.start_line as f32 * font_metrics.cell_height,
        );

        let text: SharedString = self.text.clone().into();
        let text_run = TextRun {
            len: text.len(),
            font: self.style.font.clone(),
            color: self.style.color,
            background_color: None,
            underline: self.style.underline.clone(),
            strikethrough: self.style.strikethrough.clone(),
        };

        let text_system = window.text_system();
        let shaped = text_system.shape_line(text, font_metrics.font_size, &[text_run], None);
        let _ = shaped.paint(
            pos,
            font_metrics.cell_height,
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

/// 背景矩形
///
/// 表示一个需要绘制背景色的矩形区域
#[derive(Debug, Clone)]
pub struct LayoutRect {
    /// 起始位置 (行, 列)
    pub line: i32,
    pub col: i32,
    /// 占用的单元格数（水平方向）
    pub num_of_cells: usize,
    /// 背景颜色
    pub color: Hsla,
}

impl LayoutRect {
    /// 创建新的背景矩形
    pub fn new(line: i32, col: i32, num_of_cells: usize, color: Hsla) -> Self {
        Self {
            line,
            col,
            num_of_cells,
            color,
        }
    }

    /// 检查是否可以与另一个矩形合并（同一行、相邻、同色）
    pub fn can_merge_with(&self, other: &Self) -> bool {
        // 必须在同一行
        if self.line != other.line {
            return false;
        }
        // 必须颜色相同
        if self.color != other.color {
            return false;
        }
        // 必须相邻
        let self_end = self.col + self.num_of_cells as i32;
        self_end == other.col
    }

    /// 合并另一个矩形
    pub fn merge_with(&mut self, other: &Self) {
        self.num_of_cells += other.num_of_cells;
    }

    /// 绘制背景矩形
    pub fn paint(&self, origin: Point<Pixels>, font_metrics: &FontMetrics, window: &mut Window) {
        let position = Point::new(
            (origin.x + self.col as f32 * font_metrics.cell_width).floor(),
            origin.y + self.line as f32 * font_metrics.cell_height,
        );
        let size = Size {
            width: (font_metrics.cell_width * self.num_of_cells as f32).ceil(),
            height: font_metrics.cell_height,
        };

        window.paint_quad(fill(Bounds::new(position, size), self.color));
    }
}

/// 光标布局信息
#[derive(Debug, Clone)]
pub struct CursorLayout {
    /// 光标位置 (行, 列)
    pub line: i32,
    pub col: i32,
    /// 光标形状
    pub shape: CursorShape,
    /// 光标颜色
    pub color: Hsla,
}

// ============================================================================
// 布局状态
// ============================================================================

/// 视口信息
///
/// 用于视口裁剪优化，只渲染可见区域
#[derive(Debug, Clone, Copy)]
pub struct ViewportInfo {
    /// 可见的起始行（相对于终端内容）
    pub first_visible_line: i32,
    /// 可见的结束行（相对于终端内容）
    pub last_visible_line: i32,
    /// 可见的起始列
    pub first_visible_col: i32,
    /// 可见的结束列
    pub last_visible_col: i32,
}

impl ViewportInfo {
    /// 检查指定行是否在视口内
    #[inline]
    pub fn is_line_visible(&self, line: i32) -> bool {
        line >= self.first_visible_line && line <= self.last_visible_line
    }

    /// 检查指定单元格是否在视口内
    #[inline]
    pub fn is_cell_visible(&self, line: i32, col: i32) -> bool {
        self.is_line_visible(line) && col >= self.first_visible_col && col <= self.last_visible_col
    }
}

//============================================================================
// 高亮范围数据结构
// ============================================================================

/// 高亮类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightType {
    /// 文本选择
    Selection,
    /// 搜索匹配
    SearchMatch,
    /// 当前搜索匹配（焦点）
    CurrentSearchMatch,
}

/// 高亮范围（单行）
///
/// 表示一行中需要高亮的区域
#[derive(Debug, Clone)]
pub struct HighlightedRangeLine {
    /// 行号
    pub line: i32,
    /// 起始列
    pub start_col: i32,
    /// 结束列（包含）
    pub end_col: i32,
    /// 高亮类型
    pub highlight_type: HighlightType,
    /// 高亮颜色
    pub color: Hsla,
}

impl HighlightedRangeLine {
    /// 创建新的高亮范围行
    pub fn new(
        line: i32,
        start_col: i32,
        end_col: i32,
        highlight_type: HighlightType,
        color: Hsla,
    ) -> Self {
        Self {
            line,
            start_col,
            end_col,
            highlight_type,
            color,
        }
    }

    /// 绘制高亮背景
    pub fn paint(&self, origin: Point<Pixels>, font_metrics: &FontMetrics, window: &mut Window) {
        let x = origin.x + self.start_col as f32 * font_metrics.cell_width;
        let y = origin.y + self.line as f32 * font_metrics.cell_height;
        let width = (self.end_col - self.start_col + 1) as f32 * font_metrics.cell_width;

        let bounds = Bounds::new(
            Point::new(x, y),
            Size {
                width: px(width.into()),
                height: font_metrics.cell_height,
            },
        );
        window.paint_quad(fill(bounds, self.color));
    }
}

/// 布局状态
///
/// 在prepaint 阶段计算，在 paint 阶段使用
pub struct LayoutState {
    /// 背景色
    pub background_color: Hsla,
    /// 字体度量
    pub font_metrics: FontMetrics,
    /// 终端列数
    pub cols: usize,
    /// 终端行数
    pub rows: usize,
    /// 视口信息（用于裁剪优化）
    pub viewport: ViewportInfo,
    /// 批量文本运行列表（预处理后）
    pub batched_text_runs: Vec<BatchedTextRun>,
    /// 背景矩形列表（预处理后）
    pub background_rects: Vec<LayoutRect>,
    /// 高亮范围列表（选择、搜索匹配等）
    pub highlighted_ranges: Vec<HighlightedRangeLine>,
    /// 光标布局（预处理后）
    pub cursor_layout: Option<CursorLayout>,
}

// ============================================================================
// TerminalElement 实现
// ============================================================================

impl TerminalElement {
    /// 创建新的终端元素
    pub fn new(coordinator: Arc<SessionCoordinator>) -> Self {
        Self {
            coordinator,
            focused: true,
            cursor_visible: true,
            font_size: TERMINAL_FONT_SIZE,
            line_height: TERMINAL_LINE_HEIGHT,
            cursor_color: None,
        }
    }

    /// 使用配置创建终端元素
    pub fn with_config(
        coordinator: Arc<SessionCoordinator>,
        focused: bool,
        cursor_visible: bool,
        font_size: f32,
        line_height: f32,
        cursor_color: Option<Hsla>,
    ) -> Self {
        Self {
            coordinator,
            focused,
            cursor_visible,
            font_size,
            line_height,
            cursor_color,
        }
    }

    /// 设置字体大小
    pub fn with_font_size(mut self, font_size: f32) -> Self {
        self.font_size = font_size;
        self
    }

    /// 设置行高倍数
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    /// 设置光标颜色
    pub fn with_cursor_color(mut self, color: Hsla) -> Self {
        self.cursor_color = Some(color);
        self
    }

    /// 计算字体度量
    fn calculate_font_metrics(&self, window: &mut Window, _cx: &mut App) -> FontMetrics {
        let font_size = px(self.font_size);
        let font = fonts::terminal_font();

        // 使用 'M' 字符测量等宽字体宽度
        let text: SharedString = "M".into();
        let text_run = TextRun {
            len: 1,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let text_system = window.text_system();
        let shaped = text_system.shape_line(text, font_size, &[text_run], None);
        let cell_width = shaped.width;
        let cell_height = font_size * self.line_height;

        FontMetrics {
            cell_width,
            cell_height,
            font_size,
        }
    }

    /// 转换 Alacritty 颜色到 GPUI 颜色
    fn convert_color(&self, color: &AnsiColor, theme: &gpui_component::Theme) -> Hsla {
        match color {
            AnsiColor::Named(named) => self.convert_named_color(named, theme),
            AnsiColor::Spec(rgb) => {
                gpui::rgb(((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | (rgb.b as u32)).into()
            },
            AnsiColor::Indexed(index) => self.convert_indexed_color(*index, theme),
        }
    }

    /// 转换命名颜色
    fn convert_named_color(&self, named: &NamedColor, theme: &gpui_component::Theme) -> Hsla {
        match named {
            NamedColor::Black => gpui::rgb(0x000000).into(),
            NamedColor::Red => gpui::rgb(0xcc0000).into(),
            NamedColor::Green => gpui::rgb(0x00cc00).into(),
            NamedColor::Yellow => gpui::rgb(0xcccc00).into(),
            NamedColor::Blue => gpui::rgb(0x0000cc).into(),
            NamedColor::Magenta => gpui::rgb(0xcc00cc).into(),
            NamedColor::Cyan => gpui::rgb(0x00cccc).into(),
            NamedColor::White => gpui::rgb(0xcccccc).into(),
            NamedColor::BrightBlack => gpui::rgb(0x666666).into(),
            NamedColor::BrightRed => gpui::rgb(0xff0000).into(),
            NamedColor::BrightGreen => gpui::rgb(0x00ff00).into(),
            NamedColor::BrightYellow => gpui::rgb(0xffff00).into(),
            NamedColor::BrightBlue => gpui::rgb(0x0000ff).into(),
            NamedColor::BrightMagenta => gpui::rgb(0xff00ff).into(),
            NamedColor::BrightCyan => gpui::rgb(0x00ffff).into(),
            NamedColor::BrightWhite => gpui::rgb(0xffffff).into(),
            NamedColor::Foreground => theme.foreground,
            NamedColor::Background => theme.background,
            _ => theme.foreground,
        }
    }

    /// 转换索引颜色 (256色)
    fn convert_indexed_color(&self, index: u8, _theme: &gpui_component::Theme) -> Hsla {
        match index {
            0 => gpui::rgb(0x000000).into(),
            1 => gpui::rgb(0xcc0000).into(),
            2 => gpui::rgb(0x00cc00).into(),
            3 => gpui::rgb(0xcccc00).into(),
            4 => gpui::rgb(0x0000cc).into(),
            5 => gpui::rgb(0xcc00cc).into(),
            6 => gpui::rgb(0x00cccc).into(),
            7 => gpui::rgb(0xcccccc).into(),
            8 => gpui::rgb(0x666666).into(),
            9 => gpui::rgb(0xff0000).into(),
            10 => gpui::rgb(0x00ff00).into(),
            11 => gpui::rgb(0xffff00).into(),
            12 => gpui::rgb(0x0000ff).into(),
            13 => gpui::rgb(0xff00ff).into(),
            14 => gpui::rgb(0x00ffff).into(),
            15 => gpui::rgb(0xffffff).into(),
            // 216色立方体 (16-231)
            16..=231 => {
                let idx = index - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let r = if r > 0 { r * 40 + 55 } else { 0 };
                let g = if g > 0 { g * 40 + 55 } else { 0 };
                let b = if b > 0 { b * 40 + 55 } else { 0 };
                gpui::rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)).into()
            },
            // 灰度 (232-255)
            232..=255 => {
                let gray = (index - 232) * 10 + 8;
                gpui::rgb(((gray as u32) << 16) | ((gray as u32) << 8) | (gray as u32)).into()
            },
        }
    }

    /// 构建单元格的文本样式
    fn build_text_style(&self, flags: Flags, fg_color: Hsla) -> TextRunStyle {
        let is_bold = flags.contains(Flags::BOLD);
        let is_italic = flags.contains(Flags::ITALIC);
        let font = fonts::terminal_font_with_style(is_bold, is_italic);

        // 下划线样式
        let underline = if flags.intersects(
            Flags::UNDERLINE
                | Flags::DOUBLE_UNDERLINE
                | Flags::UNDERCURL
                | Flags::DOTTED_UNDERLINE
                | Flags::DASHED_UNDERLINE,
        ) {
            Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(fg_color),
                wavy: flags.contains(Flags::UNDERCURL),
            })
        } else {
            None
        };

        // 删除线样式
        let strikethrough = if flags.contains(Flags::STRIKEOUT) {
            Some(StrikethroughStyle {
                thickness: px(1.0),
                color: Some(fg_color),
            })
        } else {
            None
        };

        TextRunStyle {
            font,
            color: fg_color,
            underline,
            strikethrough,
        }
    }
}

// ============================================================================
// IntoElement 实现
// ============================================================================

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

// ============================================================================
// Element 实现
// ============================================================================

impl Element for TerminalElement {
    type RequestLayoutState = ();
    type PrepaintState = LayoutState;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("terminal-element".into()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // 动态计算字体度量
        let font_metrics = self.calculate_font_metrics(window, cx);
        let theme = cx.theme();

        // 计算终端尺寸
        let cols = (bounds.size.width / font_metrics.cell_width).floor() as usize;
        let rows = (bounds.size.height / font_metrics.cell_height).floor() as usize;

        // 创建视口信息（用于裁剪优化）
        let viewport = ViewportInfo {
            first_visible_line: 0,
            last_visible_line: rows as i32 - 1,
            first_visible_col: 0,
            last_visible_col: cols as i32 - 1,
        };

        debug!(
            "Terminal prepaint: {}x{} cells, viewport: lines {}-{}, cols {}-{}",
            cols,
            rows,
            viewport.first_visible_line,
            viewport.last_visible_line,
            viewport.first_visible_col,
            viewport.last_visible_col
        );

        // 获取终端内容
        let term = self.coordinator.terminal().term();
        let term_guard = term.lock();
        let content = term_guard.renderable_content();

        // 预处理：构建批量文本运行和背景矩形
        let mut batched_text_runs: Vec<BatchedTextRun> = Vec::new();
        let mut background_rects: Vec<LayoutRect> = Vec::new();

        // 统计跳过的单元格数（用于调试）
        let mut skipped_cells = 0usize;

        for cell in content.display_iter {
            let point = cell.point;
            let line = point.line.0;
            let col = point.column.0 as i32;

            // 跳过宽字符占位符
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            // 视口裁剪：跳过不可见的单元格
            if !viewport.is_line_visible(line) {
                skipped_cells += 1;
                continue;
            }

            // 判断是否为宽字符
            let is_wide = cell.flags.contains(Flags::WIDE_CHAR);
            let cell_width = if is_wide { 2 } else { 1 };

            // 处理反色显示
            let (fg, bg) = if cell.flags.contains(Flags::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            // 收集背景矩形（如果不是默认背景）
            if !matches!(bg, AnsiColor::Named(NamedColor::Background)) {
                let bg_color = self.convert_color(&bg, &theme);
                let new_rect = LayoutRect::new(line, col, cell_width, bg_color);

                //尝试与上一个矩形合并
                if let Some(last_rect) = background_rects.last_mut() {
                    if last_rect.can_merge_with(&new_rect) {
                        last_rect.merge_with(&new_rect);
                    } else {
                        background_rects.push(new_rect);
                    }
                } else {
                    background_rects.push(new_rect);
                }
            }

            // 跳过隐藏字符
            if cell.flags.contains(Flags::HIDDEN) {
                continue;
            }

            // 跳过空格和空字符
            if cell.c == ' ' || cell.c == '\0' {
                continue;
            }

            // 构建文本样式
            let mut fg_color = self.convert_color(&fg, &theme);
            if cell.flags.contains(Flags::DIM) {
                fg_color.a *= 0.66;
            }
            let style = self.build_text_style(cell.flags, fg_color);

            // 尝试追加到现有批次或创建新批次
            if let Some(last_run) = batched_text_runs.last_mut() {
                if last_run.can_append(line, col, &style) {
                    last_run.append_char(cell.c, cell_width);
                } else {
                    batched_text_runs
                        .push(BatchedTextRun::new(line, col, cell.c, cell_width, style));
                }
            } else {
                batched_text_runs.push(BatchedTextRun::new(line, col, cell.c, cell_width, style));
            }
        }

        // 预处理光标
        let cursor_layout = if self.cursor_visible {
            let cursor = content.cursor;
            let cursor_color = self
                .cursor_color
                .unwrap_or_else(|| gpui::rgb(0x00ff00).into());

            Some(CursorLayout {
                line: cursor.point.line.0,
                col: cursor.point.column.0 as i32,
                shape: cursor.shape,
                color: cursor_color,
            })
        } else {
            None
        };

        // 处理选择高亮
        let mut highlighted_ranges: Vec<HighlightedRangeLine> = Vec::new();
        let selection_color = Hsla {
            h: 210.0 / 360.0,
            s: 0.5,
            l: 0.5,
            a: 0.3,
        };

        // 从终端内容获取选择范围
        if let Some(selection) = &content.selection {
            let start = selection.start;
            let end = selection.end;

            // 确保 start <= end
            let (start_line, start_col, end_line, end_col) = if start.line <= end.line {
                (
                    start.line.0,
                    start.column.0 as i32,
                    end.line.0,
                    end.column.0 as i32,
                )
            } else {
                (
                    end.line.0,
                    end.column.0 as i32,
                    start.line.0,
                    start.column.0 as i32,
                )
            };

            // 为每一行创建高亮范围
            for line in start_line..=end_line {
                // 视口裁剪：跳过不可见的行
                if !viewport.is_line_visible(line) {
                    continue;
                }

                let line_start_col = if line == start_line { start_col } else { 0 };
                let line_end_col = if line == end_line {
                    end_col
                } else {
                    cols as i32 - 1
                };

                if line_start_col <= line_end_col {
                    highlighted_ranges.push(HighlightedRangeLine::new(
                        line,
                        line_start_col,
                        line_end_col,
                        HighlightType::Selection,
                        selection_color,
                    ));
                }
            }
        }

        debug!(
            "Prepaint complete: {} text runs, {} background rects, {} highlights, {} cells skipped",
            batched_text_runs.len(),
            background_rects.len(),
            highlighted_ranges.len(),
            skipped_cells
        );

        LayoutState {
            background_color: theme.background,
            font_metrics,
            cols,
            rows,
            viewport,
            batched_text_runs,
            background_rects,
            highlighted_ranges,
            cursor_layout,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin;

        // 1. 绘制整体背景
        window.paint_quad(fill(bounds, prepaint.background_color));

        // 2. 绘制单元格背景
        for rect in &prepaint.background_rects {
            rect.paint(origin, &prepaint.font_metrics, window);
        }

        // 3. 绘制高亮（选择、搜索匹配等）
        for highlight in &prepaint.highlighted_ranges {
            highlight.paint(origin, &prepaint.font_metrics, window);
        }

        // 4. 绘制批量文本
        for text_run in &prepaint.batched_text_runs {
            text_run.paint(origin, &prepaint.font_metrics, window, cx);
        }

        // 5. 绘制光标
        if let Some(cursor) = &prepaint.cursor_layout {
            self.paint_cursor(cursor, origin, &prepaint.font_metrics, window);
        }
    }
}

// ============================================================================
//光标绘制
// ============================================================================

impl TerminalElement {
    /// 绘制光标
    fn paint_cursor(
        &self,
        cursor: &CursorLayout,
        origin: Point<Pixels>,
        font_metrics: &FontMetrics,
        window: &mut Window,
    ) {
        let cursor_x = origin.x + (cursor.col as f32) * font_metrics.cell_width;
        let cursor_y = origin.y + (cursor.line as f32) * font_metrics.cell_height;

        match cursor.shape {
            CursorShape::Block => {
                let cursor_bounds = Bounds::new(
                    Point::new(cursor_x, cursor_y),
                    Size {
                        width: font_metrics.cell_width,
                        height: font_metrics.cell_height,
                    },
                );
                if self.focused {
                    window.paint_quad(fill(cursor_bounds, cursor.color));
                } else {
                    // 失焦时显示空心方块
                    self.paint_hollow_block(cursor_x, cursor_y, font_metrics, cursor.color, window);
                }
            },
            CursorShape::Beam => {
                let cursor_bounds = Bounds::new(
                    Point::new(cursor_x, cursor_y),
                    Size {
                        width: px(2.0),
                        height: font_metrics.cell_height,
                    },
                );
                window.paint_quad(fill(cursor_bounds, cursor.color));
            },
            CursorShape::Underline => {
                let cursor_bounds = Bounds::new(
                    Point::new(cursor_x, cursor_y + font_metrics.cell_height - px(2.0)),
                    Size {
                        width: font_metrics.cell_width,
                        height: px(2.0),
                    },
                );
                window.paint_quad(fill(cursor_bounds, cursor.color));
            },
            CursorShape::HollowBlock => {
                self.paint_hollow_block(cursor_x, cursor_y, font_metrics, cursor.color, window);
            },
            CursorShape::Hidden => {
                // 隐藏光标，不绘制
            },
        }
    }

    /// 绘制空心方块光标
    fn paint_hollow_block(
        &self,
        x: Pixels,
        y: Pixels,
        font_metrics: &FontMetrics,
        color: Hsla,
        window: &mut Window,
    ) {
        let border = px(1.5);

        // 上边
        window.paint_quad(fill(
            Bounds::new(
                Point::new(x, y),
                Size {
                    width: font_metrics.cell_width,
                    height: border,
                },
            ),
            color,
        ));
        // 下边
        window.paint_quad(fill(
            Bounds::new(
                Point::new(x, y + font_metrics.cell_height - border),
                Size {
                    width: font_metrics.cell_width,
                    height: border,
                },
            ),
            color,
        ));
        // 左边
        window.paint_quad(fill(
            Bounds::new(
                Point::new(x, y),
                Size {
                    width: border,
                    height: font_metrics.cell_height,
                },
            ),
            color,
        ));
        // 右边
        window.paint_quad(fill(
            Bounds::new(
                Point::new(x + font_metrics.cell_width - border, y),
                Size {
                    width: border,
                    height: font_metrics.cell_height,
                },
            ),
            color,
        ));
    }
}
