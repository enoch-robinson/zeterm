//! 终端视图模块
//!
//! 提供终端渲染相关的组件，包括：
//! - `colors` - 颜色转换
//! - `font_metrics` - 字体度量
//! - `key_mapping` - 按键映射
//! - `wide_char` - 宽字符处理
//! - `theme` - 终端主题
//! - `terminal_element` - 终端渲染元素
//!
//! 基于GPUI 的终端渲染视图，参考 Zed 编辑器的 terminal_view 实现。
//! 负责：
//! - 管理终端渲染元素 (TerminalElement)
//! - 处理键盘输入事件
//! - 处理鼠标事件
//! - 管理焦点状态

use std::sync::Arc;

use alacritty_terminal::grid::Dimensions;
use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Render,
    ScrollWheelEvent, SharedString, Styled, TextRun, Window, div, px,
};
use gpui_component::ActiveTheme;
use tracing::{debug, info};

use crate::app::session::SessionCoordinator;

mod clipboard;
pub mod colors;
mod font_metrics;
mod fonts;
mod hyperlink;
mod ime;
mod key_mapping;
mod mouse;
mod mouse_report;
mod resize;
mod search;
mod selection;
mod shortcuts;
mod terminal_element;
pub mod theme;
mod wide_char;

#[allow(unused_imports)]
pub use colors::{ColorPalette, NamedColor, Rgb, TerminalColor, terminal_color_to_hsla};
#[allow(unused_imports)]
pub use font_metrics::FontMetrics;
#[allow(unused_imports)]
pub use hyperlink::{Hyperlink, HyperlinkConfig, HyperlinkType, detect_urls_simple};
#[allow(unused_imports)]
pub use ime::{ImeConfig, ImeContext, ImePreeditLayout, ImeState, PreeditText};
#[allow(unused_imports)]
pub use key_mapping::{KeyMapping, Modifiers, keystroke_to_bytes};
#[allow(unused_imports)]
pub use search::{
    CellData, SearchConfig, SearchDirection, SearchMatch, SearchState, extract_lines_from_cells,
    search_in_lines,
};
pub use terminal_element::TerminalElement;
#[allow(unused_imports)]
pub use theme::{
    CursorColors, SelectionColors, TerminalTheme, ThemeManager, UiColors, rgb_to_hsla,
};
#[allow(unused_imports)]
pub use wide_char::{CellContent, CharWidth, char_width, is_wide_char, string_width};

// Phase 4 模块导出
#[allow(unused_imports)]
pub use clipboard::{ClipboardManager, PasteProcessor, TextExtractor};
#[allow(unused_imports)]
pub use mouse::{
    CellPosition, ClickDetector, ClickType, CoordinateConverter, MouseButton, MouseEventType,
    MousePosition,
};
#[allow(unused_imports)]
pub use mouse_report::{
    MouseModeFlags, MouseModifiers, MouseReportAction, MouseReportButton, MouseReportEvent,
    MouseReporter, should_report_event, to_sgr_coords,
};
#[allow(unused_imports)]
pub use resize::{PixelSize, ResizeHandler, TerminalDimensions};
#[allow(unused_imports)]
pub use selection::{
    Selection, SelectionPoint, SelectionRange, SelectionState, SelectionType, WordBoundaryDetector,
};
#[allow(unused_imports)]
pub use shortcuts::{Shortcut, ShortcutAction, ShortcutManager};

/// 渲染配置
///
/// 控制终端渲染的各种参数
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// 字体大小（像素）
    pub font_size: f32,
    /// 行高倍数
    pub line_height: f32,
    /// 字体族名称
    pub font_family: String,
    /// 是否启用连字
    pub ligatures: bool,
    /// 光标样式
    pub cursor_style: CursorStyle,
    /// 光标闪烁
    pub cursor_blink: bool,
    ///滚动缓冲区大小（行数）
    pub scrollback_lines: usize,
    /// 是否显示滚动条
    pub show_scrollbar: bool,
    /// 终端主题
    pub theme: TerminalTheme,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 1.2,
            font_family: "JetBrains Mono".to_string(),
            ligatures: false,
            cursor_style: CursorStyle::Block,
            cursor_blink: true,
            scrollback_lines: 10000,
            show_scrollbar: true,
            theme: TerminalTheme::dark(),
        }
    }
}

impl RenderConfig {
    /// 创建新的渲染配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置字体大小
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size.clamp(8.0, 72.0);
        self
    }

    /// 设置行高
    pub fn with_line_height(mut self, height: f32) -> Self {
        self.line_height = height.clamp(1.0, 2.0);
        self
    }

    /// 设置字体族
    pub fn with_font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }

    /// 设置光标样式
    pub fn with_cursor_style(mut self, style: CursorStyle) -> Self {
        self.cursor_style = style;
        self
    }

    /// 设置主题
    pub fn with_theme(mut self, theme: TerminalTheme) -> Self {
        self.theme = theme;
        self
    }

    /// 增大字体
    pub fn increase_font_size(&mut self) {
        self.font_size = (self.font_size + 1.0).min(72.0);
    }

    /// 减小字体
    pub fn decrease_font_size(&mut self) {
        self.font_size = (self.font_size - 1.0).max(8.0);
    }

    /// 重置字体大小
    pub fn reset_font_size(&mut self) {
        self.font_size = 14.0;
    }
}

/// 光标样式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// 方块光标
    Block,
    /// 竖线光标
    Beam,
    /// 下划线光标
    Underline,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// 终端视图
///
/// 终端的主视图组件，负责：
/// - 创建和管理 TerminalElement 进行渲染
/// - 处理键盘输入并转发到后端
/// - 管理焦点状态
/// - 处理鼠标事件（选择、滚动等）
pub struct TerminalView {
    /// 会话协调器，管理终端连接和数据流
    coordinator: Arc<SessionCoordinator>,
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 光标是否可见（用于闪烁状态）
    cursor_visible: bool,
    /// 是否启用光标闪烁
    cursor_blink_enabled: bool,
    /// 渲染配置
    render_config: RenderConfig,
    /// 搜索状态
    search_state: SearchState,
    //========== Phase 4 新增字段 ==========
    /// 文本选择状态
    selection: Selection,
    /// 快捷键管理器
    shortcut_manager: ShortcutManager,
    /// 点击检测器（用于双击、三击检测）
    click_detector: ClickDetector,
    /// 窗口 resize 处理器
    resize_handler: ResizeHandler,
    /// 滚动偏移量（行数）
    scroll_offset: i32,
    /// 鼠标报告器（用于远端鼠标模式）
    mouse_reporter: MouseReporter,
}

impl TerminalView {
    /// 创建新的终端视图
    pub fn new(coordinator: Arc<SessionCoordinator>, cx: &mut Context<Self>) -> Self {
        Self::with_config(coordinator, RenderConfig::default(), cx)
    }

    /// 使用指定配置创建终端视图
    pub fn with_config(
        coordinator: Arc<SessionCoordinator>,
        render_config: RenderConfig,
        cx: &mut Context<Self>,
    ) -> Self {
        info!(
            "Creating TerminalView with config: {:?}",
            render_config.font_size
        );

        Self {
            coordinator,
            focus_handle: cx.focus_handle(),
            cursor_visible: true,
            cursor_blink_enabled: true,
            render_config,
            search_state: SearchState::new(),
            // Phase 4 字段初始化
            selection: Selection::new(),
            shortcut_manager: ShortcutManager::new(),
            click_detector: ClickDetector::new(),
            resize_handler: ResizeHandler::new(8.0, 16.0), // 默认单元格尺寸
            scroll_offset: 0,
            // 鼠标报告模式
            mouse_reporter: MouseReporter::new(),
        }
    }

    /// 获取渲染配置
    pub fn render_config(&self) -> &RenderConfig {
        &self.render_config
    }

    /// 获取可变渲染配置
    pub fn render_config_mut(&mut self) -> &mut RenderConfig {
        &mut self.render_config
    }

    /// 设置主题
    pub fn set_theme(&mut self, theme: TerminalTheme) {
        self.render_config.theme = theme;
    }

    /// 获取当前主题
    pub fn theme(&self) -> &TerminalTheme {
        &self.render_config.theme
    }

    /// 切换亮色/暗色主题
    pub fn toggle_theme(&mut self) {
        if self.render_config.theme.is_dark {
            self.render_config.theme = TerminalTheme::light();
        } else {
            self.render_config.theme = TerminalTheme::dark();
        }
    }
    /// 检查当前是否为暗色主题
    pub fn is_dark_theme(&self) -> bool {
        self.render_config.theme.is_dark
    }

    //========================================================================
    // 搜索功能
    // ========================================================================

    /// 获取搜索状态
    pub fn search_state(&self) -> &SearchState {
        &self.search_state
    }

    /// 获取可变搜索状态
    pub fn search_state_mut(&mut self) -> &mut SearchState {
        &mut self.search_state
    }

    /// 开始搜索
    pub fn start_search(&mut self) {
        self.search_state.activate();
    }

    /// 结束搜索
    pub fn end_search(&mut self) {
        self.search_state.deactivate();
        self.search_state.clear();
    }

    /// 执行搜索
    pub fn search(&mut self, query: &str) {
        self.search_state.set_query(query);

        if query.is_empty() {
            self.search_state.set_matches(Vec::new());
            return;
        }

        // 从终端内容提取文本行
        let term = self.coordinator.terminal().term();
        let term_guard = term.lock();
        let content = term_guard.renderable_content();

        // 收集单元格数据
        let mut cells: Vec<search::CellData> = Vec::new();
        let mut max_col = 0i32;

        for cell in content.display_iter {
            let col = cell.point.column.0 as i32;
            if col > max_col {
                max_col = col;
            }
            cells.push(search::CellData {
                line: cell.point.line.0,
                col,
                c: cell.c,
            });
        }

        let cols = (max_col + 1) as usize;
        let lines = search::extract_lines_from_cells(&cells, cols);

        // 执行搜索
        let matches = search::search_in_lines(&lines, query, self.search_state.config());
        self.search_state.set_matches(matches);
    }

    /// 跳转到下一个匹配
    pub fn search_next(&mut self) {
        self.search_state.next_match();
    }

    /// 跳转到上一个匹配
    pub fn search_prev(&mut self) {
        self.search_state.prev_match();
    }

    /// 检查搜索是否激活
    pub fn is_search_active(&self) -> bool {
        self.search_state.is_active()
    }

    /// 切换光标闪烁
    pub fn toggle_cursor_blink(&mut self) {
        self.cursor_blink_enabled = !self.cursor_blink_enabled;
        if !self.cursor_blink_enabled {
            //禁用闪烁时，确保光标可见
            self.cursor_visible = true;
        }
    }

    /// 设置光标闪烁状态
    pub fn set_cursor_blink(&mut self, enabled: bool) {
        self.cursor_blink_enabled = enabled;
        if !enabled {
            self.cursor_visible = true;
        }
    }

    /// 检查光标闪烁是否启用
    pub fn is_cursor_blink_enabled(&self) -> bool {
        self.cursor_blink_enabled
    }

    /// 重置鼠标报告器状态
    ///
    /// 当窗口失去焦点或终端会话重置时调用，
    /// 避免远端应用认为鼠标按键仍然按下
    pub fn reset_mouse_reporter(&mut self) {
        self.mouse_reporter.reset();
        debug!("Mouse reporter state reset");
    }

    /// 获取鼠标报告器的引用（用于测试或调试）
    pub fn mouse_reporter(&self) -> &MouseReporter {
        &self.mouse_reporter
    }

    /// 切换光标可见性（用于闪烁动画）
    pub fn toggle_cursor_visibility(&mut self) {
        if self.cursor_blink_enabled {
            self.cursor_visible = !self.cursor_visible;
        }
    }

    /// 构建终端视图实体
    pub fn build(coordinator: Arc<SessionCoordinator>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(coordinator, cx))
    }

    /// 处理键盘按下事件
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        // 首先检查是否是快捷键
        if let Some(action) = self
            .shortcut_manager
            .find_action_from_key(key, &event.keystroke.modifiers)
        {
            if self.handle_shortcut(action, window, cx) {
                return; // 快捷键已处理
            }
        }

        // 不是快捷键，转换为终端输入
        let modifiers = key_mapping::Modifiers::new(
            event.keystroke.modifiers.control,
            event.keystroke.modifiers.alt,
            event.keystroke.modifiers.shift,
        );

        let mapping = key_mapping::keystroke_to_bytes(key, modifiers);
        if !mapping.is_empty() {
            debug!("Key pressed: {} -> {:?}", key, mapping.bytes);
            self.coordinator.send_input_sync(&mapping.bytes);
            cx.notify();
        }
    }

    /// 获取会话协调器
    pub fn coordinator(&self) -> &Arc<SessionCoordinator> {
        &self.coordinator
    }

    //========== Phase 4:鼠标事件处理 ==========

    /// 获取当前鼠标模式标志
    fn get_mouse_mode_flags(&self) -> MouseModeFlags {
        let terminal = self.coordinator.terminal();
        MouseModeFlags {
            report_click: terminal.is_mouse_report_click_enabled(),
            report_drag: terminal.is_mouse_drag_enabled(),
            report_motion: terminal.is_mouse_motion_enabled(),
            sgr_mode: terminal.is_sgr_mouse_enabled(),
        }
    }

    /// 检查是否应该使用远端鼠标模式
    ///
    /// 当远端应用启用鼠标模式且 Shift 未按下时返回 true
    fn should_use_remote_mouse_mode(&self, shift_pressed: bool) -> bool {
        if shift_pressed {
            // Shift 键强制使用本地选择
            return false;
        }
        self.coordinator.terminal().is_any_mouse_mode_enabled()
    }

    /// 发送鼠标事件到远端
    fn send_mouse_event_to_remote(&self, event: &MouseReportEvent) {
        let mode = self.get_mouse_mode_flags();

        // 根据模式选择编码格式
        let bytes = if mode.use_sgr() {
            self.mouse_reporter.encode_sgr(event)
        } else {
            self.mouse_reporter.encode_x10(event)
        };

        if !bytes.is_empty() {
            self.coordinator.send_input_sync(&bytes);
            debug!(
                "Mouse event sent to remote: {:?} at ({}, {})",
                event.action, event.col, event.row
            );
        }
    }

    /// 获取当前终端尺寸
    fn terminal_size(&self) -> (i32, i32) {
        let size = self.coordinator.terminal_size();
        (size.cols as i32, size.rows as i32)
    }

    /// 处理鼠标按下事件
    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 获取鼠标位置
        let mouse_pos = MousePosition::from_point(event.position);

        // 检查 Shift 键状态
        let shift_pressed = event.modifiers.shift;

        // 转换为单元格坐标
        if let Some(cell_pos) = self.screen_to_cell(mouse_pos) {
            // 检查是否应该报告给远端
            if self.should_use_remote_mouse_mode(shift_pressed) {
                // 远端鼠标模式：编码并发送事件
                let (max_col, max_row) = self.terminal_size();
                let (col, row) = to_sgr_coords(cell_pos.col, cell_pos.line, max_col, max_row);

                let button = match event.button {
                    gpui::MouseButton::Left => MouseReportButton::Left,
                    gpui::MouseButton::Middle => MouseReportButton::Middle,
                    gpui::MouseButton::Right => MouseReportButton::Right,
                    _ => MouseReportButton::Left,
                };

                let modifiers = MouseModifiers {
                    shift: event.modifiers.shift,
                    alt: event.modifiers.alt,
                    ctrl: event.modifiers.control,
                };

                let report_event =
                    MouseReportEvent::new(button, MouseReportAction::Press, col, row, modifiers);

                self.send_mouse_event_to_remote(&report_event);

                // 记录按下的按钮（用于拖拽追踪）
                self.mouse_reporter.button_pressed(button);

                cx.notify();
                return;
            }

            // 本地模式：处理文本选择
            // 检测点击类型（单击、双击、三击）
            let click_type = self.click_detector.record_click(mouse_pos);

            match click_type {
                ClickType::Single => {
                    // 单击：开始字符级选择
                    self.selection.clear();
                    self.selection.start(cell_pos.to_selection_point());
                    debug!("Selection started at ({}, {})", cell_pos.line, cell_pos.col);
                },
                ClickType::Double => {
                    // 双击：选择单词
                    self.select_word_at(cell_pos);
                    debug!("Word selection at ({}, {})", cell_pos.line, cell_pos.col);
                },
                ClickType::Triple => {
                    // 三击：选择整行
                    self.selection.start_line(cell_pos.line);
                    self.selection.finish();
                    debug!("Line selection at line {}", cell_pos.line);
                },
            }

            cx.notify();
        }
    }

    /// 处理鼠标移动事件
    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mouse_pos = MousePosition::from_point(event.position);
        let shift_pressed = event.modifiers.shift;

        if let Some(cell_pos) = self.screen_to_cell(mouse_pos) {
            // 检查远端鼠标模式
            if self.should_use_remote_mouse_mode(shift_pressed) {
                let mode = self.get_mouse_mode_flags();
                let pressed_button = self.mouse_reporter.pressed_button();

                // 判断是拖拽还是移动
                let should_report = if pressed_button.is_some() {
                    // 有按钮按下 = 拖拽
                    mode.should_report_drag()
                } else {
                    // 无按钮按下 = 移动
                    mode.should_report_motion()
                };

                if should_report {
                    let (max_col, max_row) = self.terminal_size();
                    let (col, row) = to_sgr_coords(cell_pos.col, cell_pos.line, max_col, max_row);

                    let button = pressed_button.unwrap_or(MouseReportButton::None);
                    let action = if pressed_button.is_some() {
                        MouseReportAction::Drag
                    } else {
                        MouseReportAction::Motion
                    };

                    let modifiers = MouseModifiers {
                        shift: event.modifiers.shift,
                        alt: event.modifiers.alt,
                        ctrl: event.modifiers.control,
                    };

                    let report_event = MouseReportEvent::new(button, action, col, row, modifiers);
                    self.send_mouse_event_to_remote(&report_event);
                }

                cx.notify();
                return;
            }

            // 本地模式：只在选择状态下处理
            if self.selection.is_selecting() {
                self.selection.update(cell_pos.to_selection_point());
                cx.notify();
            }
        }
    }

    /// 处理鼠标释放事件
    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shift_pressed = event.modifiers.shift;

        // 检查远端鼠标模式
        if self.should_use_remote_mouse_mode(shift_pressed) {
            if let Some(pressed_button) = self.mouse_reporter.pressed_button() {
                let mouse_pos = MousePosition::from_point(event.position);

                if let Some(cell_pos) = self.screen_to_cell(mouse_pos) {
                    let (max_col, max_row) = self.terminal_size();
                    let (col, row) = to_sgr_coords(cell_pos.col, cell_pos.line, max_col, max_row);

                    let modifiers = MouseModifiers {
                        shift: event.modifiers.shift,
                        alt: event.modifiers.alt,
                        ctrl: event.modifiers.control,
                    };

                    let report_event = MouseReportEvent::new(
                        pressed_button,
                        MouseReportAction::Release,
                        col,
                        row,
                        modifiers,
                    );

                    self.send_mouse_event_to_remote(&report_event);
                }

                // 清除按钮状态
                self.mouse_reporter.button_released();
            }

            cx.notify();
            return;
        }

        // 本地模式：完成选择
        if self.selection.is_selecting() {
            self.selection.finish();
            debug!("Selection finished");
            cx.notify();
        }
    }

    /// 处理滚轮事件
    fn handle_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 计算滚动行数
        let delta_y = event.delta.pixel_delta(px(16.0)).y;
        let lines = (f32::from(delta_y) / 16.0).round() as i32;

        if lines == 0 {
            return;
        }

        let shift_pressed = event.modifiers.shift;

        // 检查远端鼠标模式
        if self.should_use_remote_mouse_mode(shift_pressed) {
            let mode = self.get_mouse_mode_flags();

            if mode.should_report_click() {
                let mouse_pos = MousePosition::from_point(event.position);

                if let Some(cell_pos) = self.screen_to_cell(mouse_pos) {
                    let (max_col, max_row) = self.terminal_size();
                    let (col, row) = to_sgr_coords(cell_pos.col, cell_pos.line, max_col, max_row);

                    let modifiers = MouseModifiers {
                        shift: event.modifiers.shift,
                        alt: event.modifiers.alt,
                        ctrl: event.modifiers.control,
                    };

                    // 滚轮事件：每行发送一次
                    let scroll_up = lines < 0;
                    let scroll_count = lines.abs();

                    for _ in 0..scroll_count {
                        let report_event =
                            MouseReportEvent::scroll(scroll_up, col, row).with_modifiers(modifiers);
                        self.send_mouse_event_to_remote(&report_event);
                    }
                }

                cx.notify();
                return;
            }
        }

        // 本地模式：滚动缓冲区
        self.scroll(-lines); // 负号：向上滚动时 delta 为正
        cx.notify();
    }

    //========== Phase 4:辅助方法 ==========

    /// 更新单元格尺寸
    ///
    /// 当字体度量变化时调用此方法更新 resize_handler
    pub fn update_cell_size(&mut self, cell_width: f32, cell_height: f32) {
        self.resize_handler.set_cell_size(cell_width, cell_height);
    }

    /// 获取当前单元格尺寸
    pub fn cell_size(&self) -> (f32, f32) {
        (
            self.resize_handler.cell_width(),
            self.resize_handler.cell_height(),
        )
    }

    /// 将屏幕坐标转换为单元格坐标
    ///
    /// 注意：GPUI 的鼠标事件位置是相对于接收事件的元素的，
    /// 所以我们直接使用位置坐标，原点为(0, 0)。
    fn screen_to_cell(&self, pos: MousePosition) -> Option<CellPosition> {
        let cell_width = self.resize_handler.cell_width();
        let cell_height = self.resize_handler.cell_height();

        if cell_width <= 0.0 || cell_height <= 0.0 {
            return None;
        }

        let dims = self.resize_handler.current_dimensions();

        // GPUI 鼠标位置已经是相对于元素的，原点为 (0, 0)
        let converter = CoordinateConverter::new(
            0.0, // 原点 x
            0.0, // 原点 y
            cell_width,
            cell_height,
            dims.cols as i32,
            dims.rows as i32,
        )
        .with_scroll_offset(self.scroll_offset);

        // 检查是否在有效范围内
        if pos.x >= 0.0 && pos.y >= 0.0 {
            Some(converter.screen_to_cell(pos))
        } else {
            None
        }
    }

    /// 在指定位置选择单词
    fn select_word_at(&mut self, cell_pos: CellPosition) {
        // 从终端内容中提取单词边界
        let word_bounds = self.detect_word_bounds_at(cell_pos);

        let point = cell_pos.to_selection_point();
        self.selection.start_word(point, word_bounds);
        self.selection.finish();

        if let Some((start, end)) = word_bounds {
            tracing::debug!(
                "Word selected at ({}, {}): columns {} to {}",
                cell_pos.line,
                cell_pos.col,
                start,
                end
            );
        }
    }

    /// 检测指定位置的单词边界
    ///
    /// 从终端内容获取指定行的文本，然后使用 WordBoundaryDetector 检测单词边界
    fn detect_word_bounds_at(&self, cell_pos: CellPosition) -> Option<(i32, i32)> {
        // 获取指定行的文本
        let line_text = self.extract_line_text_at(cell_pos.line);

        if line_text.is_empty() {
            return None;
        }

        // 使用 WordBoundaryDetector 检测单词边界
        WordBoundaryDetector::detect(&line_text, cell_pos.col as usize)
    }

    /// 从终端中提取指定行的文本
    fn extract_line_text_at(&self, line: i32) -> String {
        use alacritty_terminal::index::{Column, Line};

        // 获取终端并锁定
        let term = self.coordinator.terminal().term();
        let term_guard = term.lock();

        // 获取 grid
        let grid = term_guard.grid();
        let term_line = Line(line);

        // 检查行是否在有效范围内
        if term_line.0 < 0 || term_line.0 >= grid.screen_lines() as i32 {
            return String::new();
        }

        // 获取行内容并转换为字符串
        let mut line_text = String::new();
        let cols = grid.columns();

        for col in 0..cols {
            let cell = &grid[term_line][Column(col)];
            line_text.push(cell.c);
        }

        // 去除尾部空格但保留内部结构
        line_text.trim_end().to_string()
    }

    /// 滚动终端
    pub fn scroll(&mut self, delta: i32) {
        self.scroll_offset += delta;
        // 限制滚动范围
        self.scroll_offset = self.scroll_offset.max(0);
        self.coordinator.scroll(delta);
    }

    /// 滚动到顶部
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.coordinator.reset_scroll();
    }

    /// 滚动到底部
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.coordinator.reset_scroll();
    }

    /// 获取选择状态
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// 清除选择
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// 检查是否有选择
    pub fn has_selection(&self) -> bool {
        self.selection.has_selection()
    }

    //========== Phase 4: 快捷键和剪贴板操作 ==========

    /// 处理快捷键
    fn handle_shortcut(
        &mut self,
        action: ShortcutAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match action {
            ShortcutAction::Copy => {
                self.copy_selection(cx);
                true
            },
            ShortcutAction::Paste => {
                self.paste_from_clipboard(cx);
                true
            },
            ShortcutAction::ZoomIn => {
                self.render_config.increase_font_size();
                cx.notify();
                true
            },
            ShortcutAction::ZoomOut => {
                self.render_config.decrease_font_size();
                cx.notify();
                true
            },
            ShortcutAction::ZoomReset => {
                self.render_config.reset_font_size();
                cx.notify();
                true
            },
            ShortcutAction::ScrollToTop => {
                self.scroll_to_top();
                cx.notify();
                true
            },
            ShortcutAction::ScrollToBottom => {
                self.scroll_to_bottom();
                cx.notify();
                true
            },
            ShortcutAction::ScrollPageUp => {
                self.scroll(-24); // 约一页
                cx.notify();
                true
            },
            ShortcutAction::ScrollPageDown => {
                self.scroll(24);
                cx.notify();
                true
            },
            ShortcutAction::ClearSelection => {
                self.clear_selection();
                cx.notify();
                true
            },
            ShortcutAction::Search => {
                self.start_search();
                cx.notify();
                true
            },
            ShortcutAction::Clear => {
                self.coordinator.reset_scroll();
                cx.notify();
                true
            },
            // 其他快捷键暂不处理
            _ => false,
        }
    }

    /// 复制选中内容到剪贴板
    pub fn copy_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.has_selection() {
            debug!("Copy: no selection");
            return;
        }

        // 从终端内容中提取选中的文本
        if let Some(text) = self.extract_selected_text() {
            if ClipboardManager::copy_text(cx, &text) {
                debug!("Copied {} characters", text.len());
            }
        }
    }

    /// 从剪贴板粘贴
    pub fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = ClipboardManager::paste_text(cx) {
            // 处理粘贴文本
            let processed = clipboard::PasteProcessor::process(&text, false);
            self.coordinator.send_input_sync(&processed);
            debug!("Pasted {} characters", text.len());
        }
    }

    /// 提取选中的文本
    fn extract_selected_text(&self) -> Option<String> {
        let range = self.selection.range()?;

        // 从终端内容中提取文本
        let term = self.coordinator.terminal().term();
        let term_guard = term.lock();
        let content = term_guard.renderable_content();

        let mut lines: Vec<String> = Vec::new();
        let mut current_line = i32::MIN;
        let mut current_line_chars: Vec<char> = Vec::new();

        for cell in content.display_iter {
            let line = cell.point.line.0;
            let col = cell.point.column.0 as i32;

            // 检查是否在选择范围内
            let point = SelectionPoint::new(line, col);
            if !range.contains(&point) {
                continue;
            }

            // 新行
            if line != current_line {
                if current_line != i32::MIN && !current_line_chars.is_empty() {
                    lines.push(current_line_chars.iter().collect());
                    current_line_chars.clear();
                }
                current_line = line;
            }

            current_line_chars.push(cell.c);
        }

        // 添加最后一行
        if !current_line_chars.is_empty() {
            lines.push(current_line_chars.iter().collect());
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n").trim_end().to_string())
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let focused = self.focus_handle.is_focused(_window);

        // 动态计算并更新单元格尺寸
        // 使用实际字体测量获取精确的单元格宽度
        let font_size = px(self.render_config.font_size);
        let font = fonts::terminal_font();
        let text: SharedString = "M".into();
        let text_run = TextRun {
            len: 1,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let text_system = _window.text_system();
        let shaped = text_system.shape_line(text, font_size, &[text_run], None);
        let cell_width = shaped.width.into();
        let cell_height = self.render_config.font_size * self.render_config.line_height;
        self.update_cell_size(cell_width, cell_height);

        // 获取选择范围用于渲染
        let selection_range = self.selection.range();

        div()
            .id("terminal-view")
            .size_full()
            .bg(theme.background)
            .track_focus(&self.focus_handle)
            // 键盘事件
            .on_key_down(cx.listener(Self::handle_key_down))
            // 鼠标事件
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(Self::handle_mouse_down),
            )
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(gpui::MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
            .child(
                // 终端渲染区域
                div().id("terminal-content").size_full().p_2().child({
                    // 使用 from_render_config 完全应用 RenderConfig 配置
                    // 包括主题颜色、光标设置、选择颜色、搜索匹配颜色等
                    let mut element = TerminalElement::from_render_config(
                        self.coordinator.clone(),
                        &self.render_config,
                        focused,
                        self.cursor_visible,
                    )
                    .with_search_matches(
                        self.search_state.matches().to_vec(),
                        self.search_state.current_index(),
                    );

                    // 添加选择范围
                    if let Some(range) = selection_range {
                        element = element.with_selection(range);
                    }

                    element
                }),
            )
    }
}
