//! 对话框模块
//!
//! 包含应用程序中使用的各种对话框组件。

mod close_confirm_dialog;
mod delete_confirm_dialog;
mod error_notification;
mod host_connection_dialog;
mod host_key_channel;
mod host_key_dialog;

// CloseConfirmDialog is currently not used, keeping module for future use
// pub use close_confirm_dialog::{
//     CloseConfirmDialog, CloseConfirmEvent, CloseConfirmType, create_close_confirm_dialog,
// };
pub use delete_confirm_dialog::{
    DeleteConfirmDialog,
    DeleteConfirmEvent,
    // DeleteTarget is only used internally in DeleteConfirmEvent
    // create_delete_confirm_dialog is not used
};
pub use error_notification::ErrorNotification;
pub use host_connection_dialog::HostConnectionDialog;
