//! 终端模块
//!
//! 封装 Alacritty 终端状态机，提供终端渲染和输入处理功能。

mod terminal_state;

pub use terminal_state::{EventProxy, TerminalConfig, TerminalEvent, TerminalState};
