//! 终端渲染元素
//!
//! 基于 GPUI Element trait 的终端渲染实现。
//! 参考 Zed 编辑器的 terminal_element.rs 实现。

use std::sync::Arc;

use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hsla, IntoElement, LayoutId, Pixels, Point,
    Size, Style, Window, fill, px,
};
use gpui_component::ActiveTheme;
use tracing::debug;

use crate::app::session::SessionCoordinator;

/// 终端渲染元素
pub struct TerminalElement {
    /// 会话协调器
    coordinator: Arc<SessionCoordinator>,
    /// 是否聚焦
    focused: bool,
    /// 光标是否可见
    cursor_visible: bool,
}

/// 字体度量信息
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// 单元格宽度
    pub cell_width: Pixels,
    /// 单元格高度 (行高)
    pub cell_height: Pixels,
    /// 基线位置
    pub baseline: Pixels,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            cell_width: px(8.4),
            cell_height: px(17.0),
            baseline: px(13.0),
        }
    }
}

/// 布局状态
pub struct LayoutState {
    /// 背景色
    pub background_color: Hsla,
    /// 字体度量
    pub font_metrics: FontMetrics,
    /// 终端列数
    pub cols: usize,
    /// 终端行数
    pub rows: usize,
}

impl TerminalElement {
    /// 创建新的终端元素
    pub fn new(coordinator: Arc<SessionCoordinator>, focused: bool, cursor_visible: bool) -> Self {
        Self {
            coordinator,
            focused,
            cursor_visible,
        }
    }

    /// 将Alacritty 颜色转换为 GPUI 颜色
    fn convert_color(&self, color: &AnsiColor, theme: &gpui_component::theme::Theme) -> Hsla {
        match color {
            AnsiColor::Named(named) => self.convert_named_color(named, theme),
            AnsiColor::Spec(rgb) => gpui::rgba(
                ((rgb.r as u32) << 24) | ((rgb.g as u32) << 16) | ((rgb.b as u32) << 8) | 0xff,
            )
            .into(),
            AnsiColor::Indexed(idx) => self.convert_indexed_color(*idx, theme),
        }
    }

    /// 转换命名颜色
    fn convert_named_color(
        &self,
        named: &NamedColor,
        theme: &gpui_component::theme::Theme,
    ) -> Hsla {
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
    fn convert_indexed_color(&self, idx: u8, theme: &gpui_component::theme::Theme) -> Hsla {
        if idx < 16 {
            let named = match idx {
                0 => NamedColor::Black,
                1 => NamedColor::Red,
                2 => NamedColor::Green,
                3 => NamedColor::Yellow,
                4 => NamedColor::Blue,
                5 => NamedColor::Magenta,
                6 => NamedColor::Cyan,
                7 => NamedColor::White,
                8 => NamedColor::BrightBlack,
                9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen,
                11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue,
                13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan,
                15 => NamedColor::BrightWhite,
                _ => NamedColor::Foreground,
            };
            self.convert_named_color(&named, theme)
        } else if idx < 232 {
            // 216色立方体 (6x6x6)
            let idx = idx - 16;
            let r = (idx / 36) * 51;
            let g = ((idx / 6) % 6) * 51;
            let b = (idx % 6) * 51;
            gpui::rgb((r as u32) << 16 | (g as u32) << 8 | b as u32).into()
        } else {
            // 24级灰度
            let gray = (idx - 232) * 10 + 8;
            gpui::rgb((gray as u32) << 16 | (gray as u32) << 8 | gray as u32).into()
        }
    }
}

impl IntoElement for TerminalElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

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
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = cx.theme();

        // 计算字体度量
        let font_metrics = FontMetrics::default();

        // 计算终端尺寸
        let cols = (bounds.size.width / font_metrics.cell_width).floor() as usize;
        let rows = (bounds.size.height / font_metrics.cell_height).floor() as usize;

        debug!("Terminal prepaint: {}x{} cells", cols, rows);

        LayoutState {
            background_color: theme.background,
            font_metrics,
            cols,
            rows,
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
        let theme = cx.theme();

        // 1. 绘制背景
        window.paint_quad(fill(bounds, prepaint.background_color));

        // 2. 获取终端内容并绘制
        let term = self.coordinator.terminal().term();
        let term_guard = term.lock();
        let content = term_guard.renderable_content();
        let origin = bounds.origin;

        // 3. 绘制单元格
        for cell in content.display_iter {
            let point = cell.point;
            let cell_x = origin.x + (point.column.0 as f32) * prepaint.font_metrics.cell_width;
            let cell_y = origin.y + (point.line.0 as f32) * prepaint.font_metrics.cell_height;

            // 绘制背景色(如果不是默认背景)
            if !matches!(cell.bg, AnsiColor::Named(NamedColor::Background)) {
                let bg_color = self.convert_color(&cell.bg, theme);
                let cell_bounds = Bounds::new(
                    Point::new(cell_x, cell_y),
                    Size {
                        width: prepaint.font_metrics.cell_width,
                        height: prepaint.font_metrics.cell_height,
                    },
                );
                window.paint_quad(fill(cell_bounds, bg_color));
            }

            // TODO: 绘制字符 (后续实现)
        }

        // 4. 绘制光标
        if self.cursor_visible && self.focused {
            let cursor = content.cursor;
            let cursor_x =
                origin.x + (cursor.point.column.0 as f32) * prepaint.font_metrics.cell_width;
            let cursor_y =
                origin.y + (cursor.point.line.0 as f32) * prepaint.font_metrics.cell_height;

            let cursor_bounds = Bounds::new(
                Point::new(cursor_x, cursor_y),
                Size {
                    width: prepaint.font_metrics.cell_width,
                    height: prepaint.font_metrics.cell_height,
                },
            );

            // 绘制光标 (实心方块)
            let cursor_color: Hsla = gpui::rgb(0xffffff).into();
            window.paint_quad(fill(cursor_bounds, cursor_color));
        }
    }
}
