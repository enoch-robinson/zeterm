//! 终端视图组件
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

mod terminal_element;

pub use terminal_element::TerminalElement;

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
    /// 光标是否可见
    cursor_visible: bool,
}

impl TerminalView {
    /// 创建新的终端视图
    pub fn new(coordinator: Arc<SessionCoordinator>, cx: &mut Context<Self>) -> Self {
        info!("Creating TerminalView");

        Self {
            coordinator,
            focus_handle: cx.focus_handle(),
            cursor_visible: true,
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
        debug!("Key down: {:?}", event.keystroke);

        // 将按键转换为终端输入
        let input = self.keystroke_to_input(&event.keystroke);

        if !input.is_empty() {
            // 发送到后端
            let coordinator = self.coordinator.clone();
            let input_clone = input.clone();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                rt.block_on(async {
                    coordinator.send_input(&input_clone).await;
                });
            });

            cx.notify();
        }
    }

    /// 将GPUI 按键转换为终端输入字节
    fn keystroke_to_input(&self, keystroke: &gpui::Keystroke) -> Vec<u8> {
        let mut input = Vec::new();

        // 处理修饰键
        let ctrl = keystroke.modifiers.control;
        let alt = keystroke.modifiers.alt;

        // 获取按键字符
        let key = keystroke.key.as_str();

        match key {
            // 特殊键
            "enter" => input.push(b'\r'),
            "backspace" => input.push(0x7f),
            "tab" => input.push(b'\t'),
            "escape" => input.push(0x1b),
            "space" => input.push(b' '),

            // 方向键
            "up" => input.extend_from_slice(b"\x1b[A"),
            "down" => input.extend_from_slice(b"\x1b[B"),
            "right" => input.extend_from_slice(b"\x1b[C"),
            "left" => input.extend_from_slice(b"\x1b[D"),

            // Home/End/PageUp/PageDown
            "home" => input.extend_from_slice(b"\x1b[H"),
            "end" => input.extend_from_slice(b"\x1b[F"),
            "pageup" => input.extend_from_slice(b"\x1b[5~"),
            "pagedown" => input.extend_from_slice(b"\x1b[6~"),

            // Delete/Insert
            "delete" => input.extend_from_slice(b"\x1b[3~"),
            "insert" => input.extend_from_slice(b"\x1b[2~"),

            // 功能键 F1-F12
            "f1" => input.extend_from_slice(b"\x1bOP"),
            "f2" => input.extend_from_slice(b"\x1bOQ"),
            "f3" => input.extend_from_slice(b"\x1bOR"),
            "f4" => input.extend_from_slice(b"\x1bOS"),
            "f5" => input.extend_from_slice(b"\x1b[15~"),
            "f6" => input.extend_from_slice(b"\x1b[17~"),
            "f7" => input.extend_from_slice(b"\x1b[18~"),
            "f8" => input.extend_from_slice(b"\x1b[19~"),
            "f9" => input.extend_from_slice(b"\x1b[20~"),
            "f10" => input.extend_from_slice(b"\x1b[21~"),
            "f11" => input.extend_from_slice(b"\x1b[23~"),
            "f12" => input.extend_from_slice(b"\x1b[24~"),

            // 普通字符
            _ => {
                if key.len() == 1 {
                    let c = key.chars().next().unwrap();

                    if ctrl {
                        // Ctrl+字母-> 控制字符
                        if c.is_ascii_lowercase() {
                            input.push(c as u8 - b'a' + 1);
                        } else if c.is_ascii_uppercase() {
                            input.push(c as u8 - b'A' + 1);
                        }
                    } else if alt {
                        // Alt+字符 -> ESC + 字符
                        input.push(0x1b);
                        input.push(c as u8);
                    } else {
                        // 普通字符
                        input.extend_from_slice(c.to_string().as_bytes());
                    }
                }
            },
        }

        input
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
                div()
                    .id("terminal-content")
                    .size_full()
                    .p_2()
                    .child(TerminalElement::new(
                        self.coordinator.clone(),
                        focused,
                        self.cursor_visible,
                    )),
            )
    }
}
