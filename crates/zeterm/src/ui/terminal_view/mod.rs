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

use gpui::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Window, div,
};
use gpui_component::ActiveTheme;
use tracing::{debug, info};

use crate::app::session::SessionCoordinator;

mod colors;
mod font_metrics;
mod fonts;
mod hyperlink;
mod ime;
mod key_mapping;
mod search;
mod terminal_element;
mod theme;
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
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

        div()
            .id("terminal-view")
            .size_full()
            .bg(theme.background)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(
                // 终端渲染区域
                div().id("terminal-content").size_full().p_2().child(
                    TerminalElement::with_config(
                        self.coordinator.clone(),
                        focused,
                        self.cursor_visible,
                        self.render_config.font_size,
                        self.render_config.line_height,
                        Some(rgb_to_hsla(self.render_config.theme.cursor.color)),
                    )
                    .with_search_matches(
                        self.search_state.matches().to_vec(),
                        self.search_state.current_index(),
                    ),
                ),
            )
    }
}
