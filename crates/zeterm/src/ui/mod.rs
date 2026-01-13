//! UI 模块 - 表现层组件
//!
//! 包含所有 GPUI 视图组件和UI 相关实现。

mod dialogs;
mod main_window;
mod terminal_view;

pub use dialogs::{
    HostKeyConfirmChannel, HostKeyConfirmRequest, HostKeyConfirmResponse, HostKeyDialog,
    HostKeyInfo, HostKeyRequestReceiver, HostKeyRequestSender, HostKeyResponse,
};
pub use main_window::MainWindow;
pub use terminal_view::{TerminalElement, TerminalView};
