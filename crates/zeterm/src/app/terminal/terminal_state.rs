//! 终端状态机封装
//!
//! 封装 Alacritty 终端状态机，提供线程安全的终端状态管理。

use std::sync::Arc;

use alacritty_terminal::{
    event::{Event, EventListener},
    sync::FairMutex,
    term::{Config as TermConfig, Term, TermMode, test::TermSize},
    vte::ansi::{Processor, StdSyncHandler},
};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info};
use zeterm_core::TerminalSize;

/// 默认终端列数
const DEFAULT_COLS: u16 = 80;
/// 默认终端行数
const DEFAULT_ROWS: u16 = 24;
/// 默认滚动缓冲区大小（行数）
const DEFAULT_SCROLLBACK_LINES: u32 = 10000;

/// 终端事件代理
///
/// 实现 `EventListener` trait，将终端事件转发到 channel。
#[derive(Clone)]
pub struct EventProxy {
    /// 事件发送器
    sender: mpsc::UnboundedSender<TerminalEvent>,
}

impl EventProxy {
    /// 创建新的事件代理
    pub fn new(sender: mpsc::UnboundedSender<TerminalEvent>) -> Self {
        Self { sender }
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                let _ = self.sender.send(TerminalEvent::PtyWrite(text));
            },
            Event::Title(title) => {
                let _ = self.sender.send(TerminalEvent::TitleChanged(title));
            },
            Event::Bell => {
                let _ = self.sender.send(TerminalEvent::Bell);
            },
            Event::Exit => {
                let _ = self.sender.send(TerminalEvent::Exit);
            },
            Event::CursorBlinkingChange => {
                let _ = self.sender.send(TerminalEvent::CursorBlinkingChange);
            },
            Event::ResetTitle => {
                let _ = self.sender.send(TerminalEvent::ResetTitle);
            },
            Event::Wakeup => {
                let _ = self.sender.send(TerminalEvent::Wakeup);
            },
            _ => {
                // 忽略其他事件 (ClipboardStore, ClipboardLoad, ColorRequest, TextAreaSizeRequest)
            },
        }
    }
}

/// 终端事件
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// PTY 写入请求
    PtyWrite(String),
    /// 标题变更
    TitleChanged(String),
    /// 响铃
    Bell,
    /// 退出
    Exit,
    /// 光标闪烁变更
    CursorBlinkingChange,
    /// 重置标题
    ResetTitle,
    /// 唤醒
    Wakeup,
}

/// 终端状态机配置
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    /// 终端列数
    pub cols: u16,
    /// 终端行数
    pub rows: u16,
    /// 滚动缓冲区大小（行数）
    pub scrollback_lines: u32,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
        }
    }
}

impl TerminalConfig {
    /// 创建新的终端配置
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            ..Default::default()
        }
    }

    /// 设置滚动缓冲区大小
    pub fn with_scrollback(mut self, lines: u32) -> Self {
        self.scrollback_lines = lines;
        self
    }

    /// 从TerminalSize 创建配置
    pub fn from_size(size: TerminalSize) -> Self {
        Self::new(size.cols, size.rows)
    }
}

/// 终端状态机
///
/// 封装 Alacritty 的`Term` 结构，提供线程安全的终端状态管理。
///
/// # 示例
///
/// ```ignore
/// let config = TerminalConfig::default();
/// let (state, event_rx) = TerminalState::new(config);
///
/// // 处理输入数据
/// state.advance_bytes(b"\x1b[31mHello\x1b[0m");
///
/// // 获取终端内容
/// let term = state.term();
/// let content = term.lock().renderable_content();
/// ```
pub struct TerminalState {
    /// Alacritty 终端实例
    term: Arc<FairMutex<Term<EventProxy>>>,
    /// 当前终端尺寸
    size: RwLock<TerminalSize>,
    /// 终端配置
    config: TerminalConfig,
}

impl TerminalState {
    /// 创建新的终端状态机
    ///
    /// # Arguments
    ///
    /// * `config` - 终端配置
    ///
    /// # Returns
    ///
    /// 返回终端状态机和事件接收器的元组
    pub fn new(config: TerminalConfig) -> (Self, mpsc::UnboundedReceiver<TerminalEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let event_proxy = EventProxy::new(event_tx);

        // 创建终端尺寸
        let term_size = TermSize::new(config.cols as usize, config.rows as usize);

        // 创建终端配置
        let term_config = TermConfig::default();

        // 创建终端实例
        let term = Term::new(term_config, &term_size, event_proxy);
        let term = Arc::new(FairMutex::new(term));

        let size = TerminalSize::new(config.rows, config.cols);

        info!(
            "Terminal state created: {}x{}, scrollback: {} lines",
            config.cols, config.rows, config.scrollback_lines
        );

        (
            Self {
                term,
                size: RwLock::new(size),
                config,
            },
            event_rx,
        )
    }

    /// 使用默认配置创建终端状态机
    pub fn with_defaults() -> (Self, mpsc::UnboundedReceiver<TerminalEvent>) {
        Self::new(TerminalConfig::default())
    }

    /// 获取终端实例的引用
    ///
    /// 返回 `Arc<FairMutex<Term>>` 用于渲染和查询终端状态。
    pub fn term(&self) -> Arc<FairMutex<Term<EventProxy>>> {
        Arc::clone(&self.term)
    }

    /// 处理输入字节数据
    ///
    /// 将字节数据送入终端状态机进行解析和处理。
    ///
    /// # Arguments
    ///
    /// * `bytes` - 要处理的字节数据
    pub fn advance_bytes(&self, bytes: &[u8]) {
        let mut term = self.term.lock();
        let mut processor: Processor<StdSyncHandler> = Processor::new();
        processor.advance(&mut *term, bytes);
        debug!("Advanced {} bytes to terminal", bytes.len());
    }

    /// 调整终端大小
    ///
    /// # Arguments
    ///
    /// * `rows` - 新的行数
    /// * `cols` - 新的列数
    pub fn resize(&self, rows: u16, cols: u16) {
        let new_size = TermSize::new(cols as usize, rows as usize);
        let mut term = self.term.lock();
        term.resize(new_size);

        let mut size = self.size.write();
        *size = TerminalSize::new(rows, cols);

        info!("Terminal resized to {}x{}", cols, rows);
    }

    /// 获取当前终端尺寸
    pub fn size(&self) -> TerminalSize {
        *self.size.read()
    }

    /// 获取终端配置
    pub fn config(&self) -> &TerminalConfig {
        &self.config
    }

    /// 滚动终端
    ///
    /// # Arguments
    ///
    /// * `delta` - 滚动行数，正数向上滚动，负数向下滚动
    pub fn scroll(&self, delta: i32) {
        let mut term = self.term.lock();
        let scroll = alacritty_terminal::grid::Scroll::Delta(delta);
        term.scroll_display(scroll);
        debug!("Terminal scrolled by {} lines", delta);
    }

    /// 重置滚动位置到底部
    pub fn reset_scroll(&self) {
        let mut term = self.term.lock();
        term.scroll_display(alacritty_terminal::grid::Scroll::Bottom);
        debug!("Terminal scroll reset to bottom");
    }

    /// 清空终端（重置滚动位置）
    pub fn clear(&self) {
        self.reset_scroll();
        debug!("Terminal cleared");
    }

    /// 获取终端标题
    pub fn title(&self) -> Option<String> {
        // 标题通过事件获取，这里返回 None
        None
    }

    // ========== 鼠标模式检测 ==========

    /// 获取终端模式
    ///
    /// 返回当前终端的模式标志，用于检测鼠标报告模式等
    pub fn mode(&self) -> TermMode {
        let term = self.term.lock();
        *term.mode()
    }

    /// 检查是否启用了鼠标点击报告 (?1000h)
    pub fn is_mouse_report_click_enabled(&self) -> bool {
        self.mode().contains(TermMode::MOUSE_REPORT_CLICK)
    }

    /// 检查是否启用了鼠标拖拽报告 (?1002h)
    pub fn is_mouse_drag_enabled(&self) -> bool {
        self.mode().contains(TermMode::MOUSE_DRAG)
    }

    /// 检查是否启用了鼠标移动报告 (?1003h)
    pub fn is_mouse_motion_enabled(&self) -> bool {
        self.mode().contains(TermMode::MOUSE_MOTION)
    }

    /// 检查是否使用 SGR 鼠标编码 (?1006h)
    pub fn is_sgr_mouse_enabled(&self) -> bool {
        self.mode().contains(TermMode::SGR_MOUSE)
    }

    /// 检查是否启用了任何鼠标报告模式
    ///
    /// 当远端应用（如 vim、tmux）启用鼠标支持时返回 true
    pub fn is_any_mouse_mode_enabled(&self) -> bool {
        let mode = self.mode();
        mode.contains(TermMode::MOUSE_REPORT_CLICK)
            || mode.contains(TermMode::MOUSE_DRAG)
            || mode.contains(TermMode::MOUSE_MOTION)
    }

    /// 检查是否应该报告鼠标点击事件
    ///
    /// 当任意鼠标报告模式启用时返回 true
    pub fn should_report_mouse_click(&self) -> bool {
        self.is_any_mouse_mode_enabled()
    }

    /// 检查是否应该报告鼠标拖拽事件
    pub fn should_report_mouse_drag(&self) -> bool {
        let mode = self.mode();
        mode.contains(TermMode::MOUSE_DRAG) || mode.contains(TermMode::MOUSE_MOTION)
    }

    /// 检查是否应该报告鼠标移动事件（非拖拽）
    pub fn should_report_mouse_motion(&self) -> bool {
        self.mode().contains(TermMode::MOUSE_MOTION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_config_default() {
        let config = TerminalConfig::default();
        assert_eq!(config.cols, DEFAULT_COLS);
        assert_eq!(config.rows, DEFAULT_ROWS);
        assert_eq!(config.scrollback_lines, DEFAULT_SCROLLBACK_LINES);
    }

    #[test]
    fn test_terminal_config_custom() {
        let config = TerminalConfig::new(120, 40).with_scrollback(5000);
        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
        assert_eq!(config.scrollback_lines, 5000);
    }

    #[test]
    fn test_terminal_state_creation() {
        let config = TerminalConfig::default();
        let (state, _rx) = TerminalState::new(config);

        let size = state.size();
        assert_eq!(size.cols, DEFAULT_COLS);
        assert_eq!(size.rows, DEFAULT_ROWS);
    }

    #[test]
    fn test_terminal_state_resize() {
        let (state, _rx) = TerminalState::with_defaults();

        state.resize(40, 120);

        let size = state.size();
        assert_eq!(size.cols, 120);
        assert_eq!(size.rows, 40);
    }

    #[test]
    fn test_terminal_advance_bytes() {
        let (state, _rx) = TerminalState::with_defaults();

        // 发送简单文本
        state.advance_bytes(b"Hello, World!");

        // 发送 ANSI 转义序列
        state.advance_bytes(b"\x1b[31mRed Text\x1b[0m");
    }

    #[test]
    fn test_terminal_mouse_mode_default() {
        let (state, _rx) = TerminalState::with_defaults();

        // 默认情况下，鼠标模式应该都是关闭的
        assert!(!state.is_mouse_report_click_enabled());
        assert!(!state.is_mouse_drag_enabled());
        assert!(!state.is_mouse_motion_enabled());
        assert!(!state.is_sgr_mouse_enabled());
        assert!(!state.is_any_mouse_mode_enabled());
    }

    #[test]
    fn test_terminal_mouse_mode_enabled() {
        let (state, _rx) = TerminalState::with_defaults();

        // 发送启用鼠标点击报告的转义序列 (?1000h)
        state.advance_bytes(b"\x1b[?1000h");
        assert!(state.is_mouse_report_click_enabled());
        assert!(state.is_any_mouse_mode_enabled());
        assert!(state.should_report_mouse_click());

        // 发送启用 SGR 鼠标编码的转义序列 (?1006h)
        state.advance_bytes(b"\x1b[?1006h");
        assert!(state.is_sgr_mouse_enabled());

        // 发送禁用鼠标模式的转义序列
        state.advance_bytes(b"\x1b[?1000l");
        assert!(!state.is_mouse_report_click_enabled());
    }
}
