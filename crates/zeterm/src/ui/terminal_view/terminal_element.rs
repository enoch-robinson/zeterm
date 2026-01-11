//! 终端渲染元素
//!
//! 基于 GPUI Element trait 的终端渲染实现。
//! 参考 Zed 编辑器的 terminal_element.rs 实现。

use std::sync::Arc;

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
/// 终端字体大小
pub const TERMINAL_FONT_SIZE: f32 = 14.0;
/// 终端行高倍数
pub const TERMINAL_LINE_HEIGHT: f32 = 1.2;

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

    /// 计算字体度量
    fn calculate_font_metrics(&self, window: &mut Window, _cx: &mut App) -> FontMetrics {
        let font_size = px(TERMINAL_FONT_SIZE);
        let text_system = window.text_system();

        // 获取等宽字体的字符宽度
        let font_id = text_system.resolve_font(&Font::default());
        let cell_width = text_system
            .advance(font_id, font_size, 'M')
            .map(|advance| advance.width)
            .unwrap_or(font_size * 0.6);

        let cell_height = font_size * TERMINAL_LINE_HEIGHT;

        FontMetrics {
            cell_width,
            cell_height,
            font_size,
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
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let theme = cx.theme();

        // 动态计算字体度量
        let font_metrics = self.calculate_font_metrics(window, cx);

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

            // 跳过宽字符占位符 (CJK字符的第二列)
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }

            // 判断是否为宽字符
            let is_wide = cell.flags.contains(Flags::WIDE_CHAR);

            // 处理反色显示 (INVERSE)
            let (fg, bg) = if cell.flags.contains(Flags::INVERSE) {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };

            // 绘制背景色(如果不是默认背景)
            if !matches!(bg, AnsiColor::Named(NamedColor::Background)) {
                let bg_color = self.convert_color(&bg, theme);
                //宽字符背景占用2列
                let bg_width = if is_wide {
                    prepaint.font_metrics.cell_width * 2.0
                } else {
                    prepaint.font_metrics.cell_width
                };
                let cell_bounds = Bounds::new(
                    Point::new(cell_x, cell_y),
                    Size {
                        width: bg_width,
                        height: prepaint.font_metrics.cell_height,
                    },
                );
                window.paint_quad(fill(cell_bounds, bg_color));
            }

            // 跳过隐藏字符
            if cell.flags.contains(Flags::HIDDEN) {
                continue;
            }

            // 绘制字符 (如果不是空格或空字符)
            if cell.c != ' ' && cell.c != '\0' {
                let mut fg_color = self.convert_color(&fg, theme);

                // 处理暗淡显示 (DIM)
                if cell.flags.contains(Flags::DIM) {
                    fg_color.a *= 0.66;
                }
                // 创建字符串
                let text: SharedString = cell.c.to_string().into();
                let font_size = px(14.0);

                // 创建文本样式
                let font = Font {
                    weight: if cell.flags.contains(Flags::BOLD) {
                        gpui::FontWeight::BOLD
                    } else {
                        gpui::FontWeight::NORMAL
                    },
                    style: if cell.flags.contains(Flags::ITALIC) {
                        gpui::FontStyle::Italic
                    } else {
                        gpui::FontStyle::Normal
                    },
                    ..Default::default()
                };

                // 下划线样式
                let underline = if cell.flags.intersects(
                    Flags::UNDERLINE
                        | Flags::DOUBLE_UNDERLINE
                        | Flags::UNDERCURL
                        | Flags::DOTTED_UNDERLINE
                        | Flags::DASHED_UNDERLINE,
                ) {
                    Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(fg_color),
                        wavy: cell.flags.contains(Flags::UNDERCURL),
                    })
                } else {
                    None
                };

                // 删除线样式
                let strikethrough = if cell.flags.contains(Flags::STRIKEOUT) {
                    Some(StrikethroughStyle {
                        thickness: px(1.0),
                        color: Some(fg_color),
                    })
                } else {
                    None
                };

                let text_run = TextRun {
                    len: text.len(),
                    font,
                    color: fg_color,
                    background_color: None,
                    underline,
                    strikethrough,
                };

                // 使用 text_system 绘制字符
                let text_system = window.text_system();
                let shaped = text_system.shape_line(text, font_size, &[text_run], None);
                let text_pos = Point::new(cell_x, cell_y);
                let _ = shaped.paint(
                    text_pos,
                    prepaint.font_metrics.cell_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        }

        // 4. 绘制光标
        if self.cursor_visible {
            let cursor = content.cursor;
            let cursor_x =
                origin.x + (cursor.point.column.0 as f32) * prepaint.font_metrics.cell_width;
            let cursor_y =
                origin.y + (cursor.point.line.0 as f32) * prepaint.font_metrics.cell_height;

            let cursor_color: Hsla = gpui::rgb(0x00ff00).into(); // 绿色光标

            match cursor.shape {
                CursorShape::Block => {
                    // 实心方块光标
                    let cursor_bounds = Bounds::new(
                        Point::new(cursor_x, cursor_y),
                        Size {
                            width: prepaint.font_metrics.cell_width,
                            height: prepaint.font_metrics.cell_height,
                        },
                    );
                    if self.focused {
                        window.paint_quad(fill(cursor_bounds, cursor_color));
                    } else {
                        // 失焦时显示空心方块
                        let border = px(1.5);
                        // 上边
                        window.paint_quad(fill(
                            Bounds::new(
                                Point::new(cursor_x, cursor_y),
                                Size {
                                    width: prepaint.font_metrics.cell_width,
                                    height: border,
                                },
                            ),
                            cursor_color,
                        ));
                        // 下边
                        window.paint_quad(fill(
                            Bounds::new(
                                Point::new(
                                    cursor_x,
                                    cursor_y + prepaint.font_metrics.cell_height - border,
                                ),
                                Size {
                                    width: prepaint.font_metrics.cell_width,
                                    height: border,
                                },
                            ),
                            cursor_color,
                        ));
                        // 左边
                        window.paint_quad(fill(
                            Bounds::new(
                                Point::new(cursor_x, cursor_y),
                                Size {
                                    width: border,
                                    height: prepaint.font_metrics.cell_height,
                                },
                            ),
                            cursor_color,
                        ));
                        // 右边
                        window.paint_quad(fill(
                            Bounds::new(
                                Point::new(
                                    cursor_x + prepaint.font_metrics.cell_width - border,
                                    cursor_y,
                                ),
                                Size {
                                    width: border,
                                    height: prepaint.font_metrics.cell_height,
                                },
                            ),
                            cursor_color,
                        ));
                    }
                },
                CursorShape::Beam => {
                    // 竖线光标
                    let cursor_bounds = Bounds::new(
                        Point::new(cursor_x, cursor_y),
                        Size {
                            width: px(2.0),
                            height: prepaint.font_metrics.cell_height,
                        },
                    );
                    window.paint_quad(fill(cursor_bounds, cursor_color));
                },
                CursorShape::Underline => {
                    // 下划线光标
                    let cursor_bounds = Bounds::new(
                        Point::new(
                            cursor_x,
                            cursor_y + prepaint.font_metrics.cell_height - px(2.0),
                        ),
                        Size {
                            width: prepaint.font_metrics.cell_width,
                            height: px(2.0),
                        },
                    );
                    window.paint_quad(fill(cursor_bounds, cursor_color));
                },
                CursorShape::HollowBlock => {
                    // 空心方块光标
                    let border = px(1.5);
                    // 上边
                    window.paint_quad(fill(
                        Bounds::new(
                            Point::new(cursor_x, cursor_y),
                            Size {
                                width: prepaint.font_metrics.cell_width,
                                height: border,
                            },
                        ),
                        cursor_color,
                    ));
                    // 下边
                    window.paint_quad(fill(
                        Bounds::new(
                            Point::new(
                                cursor_x,
                                cursor_y + prepaint.font_metrics.cell_height - border,
                            ),
                            Size {
                                width: prepaint.font_metrics.cell_width,
                                height: border,
                            },
                        ),
                        cursor_color,
                    ));
                    // 左边
                    window.paint_quad(fill(
                        Bounds::new(
                            Point::new(cursor_x, cursor_y),
                            Size {
                                width: border,
                                height: prepaint.font_metrics.cell_height,
                            },
                        ),
                        cursor_color,
                    ));
                    // 右边
                    window.paint_quad(fill(
                        Bounds::new(
                            Point::new(
                                cursor_x + prepaint.font_metrics.cell_width - border,
                                cursor_y,
                            ),
                            Size {
                                width: border,
                                height: prepaint.font_metrics.cell_height,
                            },
                        ),
                        cursor_color,
                    ));
                },
                CursorShape::Hidden => {
                    // 隐藏光标，不绘制
                },
            }
        }
    }
}
