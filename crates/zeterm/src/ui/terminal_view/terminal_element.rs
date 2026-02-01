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
/// 最小字体大小
pub const MIN_FONT_SIZE: f32 = 8.0;
/// 最大字体大小
pub const MAX_FONT_SIZE: f32 = 72.0;
/// 字体缩放步长
pub const FONT_SCALE_STEP: f32 = 2.0;
/// 光标闪烁间隔（毫秒）
pub const CURSOR_BLINK_INTERVAL_MS: u64 = 500;

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
    /// 是否启用光标闪烁
    cursor_blink_enabled: bool,
    /// 字体大小
    font_size: f32,
    /// 行高倍数
    line_height: f32,
    /// 光标颜色（可选，None时使用默认绿色）
    cursor_color: Option<Hsla>,
    /// 搜索匹配列表
    search_matches: Vec<super::search::SearchMatch>,
    /// 当前搜索匹配索引
    current_search_index: Option<usize>,
    /// 搜索匹配高亮颜色
    search_match_color: Hsla,
    /// 当前搜索匹配高亮颜色
    current_search_match_color: Hsla,
    /// 选择范围（Phase 4）
    selection_range: Option<super::selection::SelectionRange>,
    /// 选择高亮颜色
    selection_color: Hsla,
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
            cursor_blink_enabled: true,
            font_size: TERMINAL_FONT_SIZE,
            line_height: TERMINAL_LINE_HEIGHT,
            cursor_color: None,
            search_matches: Vec::new(),
            current_search_index: None,
            search_match_color: Hsla {
                h: 60.0 / 360.0,
                s: 0.8,
                l: 0.5,
                a: 0.3,
            },
            current_search_match_color: Hsla {
                h: 30.0 / 360.0,
                s: 0.9,
                l: 0.5,
                a: 0.5,
            },
            selection_range: None,
            selection_color: Hsla {
                h: 210.0 / 360.0,
                s: 0.6,
                l: 0.5,
                a: 0.3,
            },
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
            cursor_blink_enabled: true,
            font_size,
            line_height,
            cursor_color,
            search_matches: Vec::new(),
            current_search_index: None,
            search_match_color: Hsla {
                h: 60.0 / 360.0,
                s: 0.8,
                l: 0.5,
                a: 0.3,
            },
            current_search_match_color: Hsla {
                h: 30.0 / 360.0,
                s: 0.9,
                l: 0.5,
                a: 0.5,
            },
            selection_range: None,
            selection_color: Hsla {
                h: 210.0 / 360.0,
                s: 0.6,
                l: 0.5,
                a: 0.3,
            },
        }
    }

    /// 从 RenderConfig 创建终端元素///
    /// 完全使用 RenderConfig 中的配置，包括：
    /// - 字体大小和行高
    /// - 光标颜色（从主题获取）
    /// - 光标闪烁设置
    /// - 选择颜色（从主题获取）
    /// - 搜索匹配颜色（从主题获取）
    pub fn from_render_config(
        coordinator: Arc<SessionCoordinator>,
        config: &super::RenderConfig,
        focused: bool,
        cursor_visible: bool,
    ) -> Self {
        use super::theme::rgb_to_hsla;

        let theme = &config.theme;

        // 从主题获取光标颜色
        let cursor_color = rgb_to_hsla(theme.cursor.color);

        // 从主题获取选择颜色
        let selection_rgb = theme.selection.background;
        let selection_color = Hsla {
            h: rgb_to_hsla(selection_rgb).h,
            s: rgb_to_hsla(selection_rgb).s,
            l: rgb_to_hsla(selection_rgb).l,
            a: 0.4, // 选择高亮需要透明度
        };

        // 从主题获取搜索匹配颜色
        let search_match_rgb = theme.ui.search_match;
        let search_match_color = Hsla {
            h: rgb_to_hsla(search_match_rgb).h,
            s: rgb_to_hsla(search_match_rgb).s,
            l: rgb_to_hsla(search_match_rgb).l,
            a: 0.4,
        };

        let search_match_active_rgb = theme.ui.search_match_active;
        let current_search_match_color = Hsla {
            h: rgb_to_hsla(search_match_active_rgb).h,
            s: rgb_to_hsla(search_match_active_rgb).s,
            l: rgb_to_hsla(search_match_active_rgb).l,
            a: 0.6,
        };

        Self {
            coordinator,
            focused,
            cursor_visible,
            cursor_blink_enabled: config.cursor_blink,
            font_size: config.font_size,
            line_height: config.line_height,
            cursor_color: Some(cursor_color),
            search_matches: Vec::new(),
            current_search_index: None,
            search_match_color,
            current_search_match_color,
            selection_range: None,
            selection_color,
        }
    }

    /// 设置选择范围
    pub fn with_selection(mut self, range: super::selection::SelectionRange) -> Self {
        self.selection_range = Some(range);
        self
    }

    /// 设置选择高亮颜色
    pub fn with_selection_color(mut self, color: Hsla) -> Self {
        self.selection_color = color;
        self
    }

    /// 清除选择
    pub fn clear_selection(mut self) -> Self {
        self.selection_range = None;
        self
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

    /// 设置焦点状态
    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// 设置光标可见性
    pub fn with_cursor_visible(mut self, visible: bool) -> Self {
        self.cursor_visible = visible;
        self
    }

    /// 设置光标闪烁
    pub fn with_cursor_blink(mut self, enabled: bool) -> Self {
        self.cursor_blink_enabled = enabled;
        self
    }

    /// 启用光标闪烁
    pub fn enable_cursor_blink(&mut self) {
        self.cursor_blink_enabled = true;
    }

    /// 禁用光标闪烁
    pub fn disable_cursor_blink(&mut self) {
        self.cursor_blink_enabled = false;
    }

    /// 检查光标闪烁是否启用
    pub fn is_cursor_blink_enabled(&self) -> bool {
        self.cursor_blink_enabled
    }

    /// 设置搜索匹配
    pub fn with_search_matches(
        mut self,
        matches: Vec<super::search::SearchMatch>,
        current_index: Option<usize>,
    ) -> Self {
        self.search_matches = matches;
        self.current_search_index = current_index;
        self
    }

    /// 设置搜索匹配颜色
    pub fn with_search_colors(mut self, match_color: Hsla, current_match_color: Hsla) -> Self {
        self.search_match_color = match_color;
        self.current_search_match_color = current_match_color;
        self
    }

    /// 清除搜索匹配
    pub fn clear_search_matches(&mut self) {
        self.search_matches.clear();
        self.current_search_index = None;
    }

    /// 放大字体
    pub fn zoom_in(&mut self) {
        self.font_size = (self.font_size + FONT_SCALE_STEP).min(MAX_FONT_SIZE);
    }

    /// 缩小字体
    pub fn zoom_out(&mut self) {
        self.font_size = (self.font_size - FONT_SCALE_STEP).max(MIN_FONT_SIZE);
    }

    /// 重置字体大小
    pub fn reset_zoom(&mut self) {
        self.font_size = TERMINAL_FONT_SIZE;
    }

    /// 获取当前字体大小
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// 设置字体大小（带范围限制）
    pub fn set_font_size(&mut self, size: f32) {
        self.font_size = size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
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

    /// 检测是否为装饰字符///
    /// 装饰字符（如 Powerline 符号、Box Drawing 等）需要保持原色，
    /// 不应用对比度调整，以保持视觉效果。
    #[inline]
    fn is_decorative_character(ch: char) -> bool {
        matches!(
            ch as u32,
            // Box Drawing (└ ┐ ─ │等)
            0x2500..=0x257F
            // Block Elements (▀ ▄ █ ░ 等)
            | 0x2580..=0x259F
            // Geometric Shapes (■▶ ● 等)
            | 0x25A0..=0x25FF
            // Powerline symbols
            | 0xE0B0..=0xE0D7
            // Powerline Extra symbols
            | 0xE0A0..=0xE0A3
        )
    }

    /// 确保前景色和背景色有足够的对比度///
    /// 使用 WCAG 2.0 的相对亮度公式计算对比度，
    /// 如果对比度不足，则调整前景色的亮度。
    fn ensure_minimum_contrast(fg: Hsla, bg: Hsla, minimum_contrast: f32) -> Hsla {
        let fg_luminance = Self::relative_luminance(fg);
        let bg_luminance = Self::relative_luminance(bg);

        let contrast = Self::contrast_ratio(fg_luminance, bg_luminance);

        if contrast >= minimum_contrast {
            return fg;
        }

        // 需要调整前景色亮度
        let mut adjusted = fg;

        // 根据背景亮度决定调整方向
        if bg_luminance > 0.5 {
            // 深色背景，降低前景色亮度
            adjusted.l = (adjusted.l - 0.1).max(0.0);
        } else {
            // 浅色背景，提高前景色亮度
            adjusted.l = (adjusted.l + 0.1).min(1.0);
        }

        // 递归调整直到满足对比度要求（最多调整10次）
        let new_contrast = Self::contrast_ratio(Self::relative_luminance(adjusted), bg_luminance);
        if new_contrast < minimum_contrast && (adjusted.l > 0.05 && adjusted.l < 0.95) {
            Self::ensure_minimum_contrast(adjusted, bg, minimum_contrast)
        } else {
            adjusted
        }
    }

    /// 计算相对亮度(WCAG 2.0)
    #[inline]
    fn relative_luminance(color: Hsla) -> f32 {
        // 将 HSL 转换为 RGB 来计算亮度
        // 简化计算：使用亮度分量作为近似值
        color.l
    }

    /// 计算对比度比率 (WCAG 2.0)
    #[inline]
    fn contrast_ratio(l1: f32, l2: f32) -> f32 {
        let lighter = l1.max(l2);
        let darker = l1.min(l2);
        (lighter + 0.05) / (darker + 0.05)
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
            let bg_color = self.convert_color(&bg, &theme);

            // 对比度调整：确保文本可读（跳过装饰字符）
            if !Self::is_decorative_character(cell.c) {
                const MINIMUM_CONTRAST: f32 = 4.5; // WCAG AA 标准
                fg_color = Self::ensure_minimum_contrast(fg_color, bg_color, MINIMUM_CONTRAST);
            }

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
            // 使用配置的光标颜色，如果未配置则使用默认灰白色（而非硬编码的绿色）
            // 这个默认值应该很少使用，因为 from_render_config 会从主题获取颜色
            let cursor_color = self
                .cursor_color
                .unwrap_or_else(|| gpui::rgb(0xcccccc).into());

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
        let selection_color = self.selection_color;

        // 优先使用 self.selection_range（来自 TerminalView 的选择）
        if let Some(range) = &self.selection_range {
            let start_line = range.start.line;
            let start_col = range.start.col;
            let end_line = range.end.line;
            let end_col = range.end.col;

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
        // 否则从终端内容获取选择范围
        else if let Some(selection) = &content.selection {
            let start = selection.start;
            let end = selection.end;

            // 确保 start <= end（同时处理同一行内的反向选择）
            let (start_line, start_col, end_line, end_col) = if start.line < end.line
                || (start.line == end.line && start.column <= end.column)
            {
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

        // 处理搜索匹配高亮
        for (match_idx, search_match) in self.search_matches.iter().enumerate() {
            let is_current = self.current_search_index == Some(match_idx);
            let color = if is_current {
                self.current_search_match_color
            } else {
                self.search_match_color
            };

            // 为每一行创建高亮范围
            for line in search_match.start_line..=search_match.end_line {
                // 视口裁剪：跳过不可见的行
                if !viewport.is_line_visible(line) {
                    continue;
                }

                let line_start_col = if line == search_match.start_line {
                    search_match.start_col
                } else {
                    0
                };
                let line_end_col = if line == search_match.end_line {
                    search_match.end_col
                } else {
                    cols as i32 - 1
                };

                if line_start_col <= line_end_col {
                    let highlight_type = if is_current {
                        HighlightType::CurrentSearchMatch
                    } else {
                        HighlightType::SearchMatch
                    };

                    highlighted_ranges.push(HighlightedRangeLine::new(
                        line,
                        line_start_col,
                        line_end_col,
                        highlight_type,
                        color,
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
