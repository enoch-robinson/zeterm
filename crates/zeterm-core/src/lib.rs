//! Zeterm Core -领域层
//!
//! 定义 Zeterm 的核心抽象、实体和业务规则。
//!
//! # 模块结构
//!
//! - `traits` - 核心 Trait 定义（TerminalConnection等）
//! - `errors` - 错误类型定义
//! - `entities` - 业务实体（TerminalSize、SessionId 等）
//! - `state` - 状态定义（ConnectionState 等）
//!
//! # 示例
//!
//! ```ignore
//! use zeterm_core::prelude::*;
//!
//! // 使用终端尺寸
//! let size = TerminalSize::default();
//! assert!(size.is_valid());
//!
//! // 创建会话 ID
//! let session_id = SessionId::new();
//! ```

pub mod config;
pub mod entities;
pub mod errors;
pub mod state;
pub mod traits;

/// Prelude 模块
///
/// 导出常用类型，方便使用。
pub mod prelude {
    pub use crate::config::{
        AppConfig, AppearanceConfig, CursorStyle, KeybindingsConfig, NetworkConfig, TerminalConfig,
    };
    pub use crate::entities::{SessionId, TerminalSize};
    pub use crate::errors::{AuthError, ConfigError, ConnectionError, StorageError};
    pub use crate::state::{
        ConnectionEvent, ConnectionState, ConnectionStateMachine, DisconnectReason,
        StateChangeCallback, UiConnectionStatus,
    };
    pub use crate::traits::{ConnectionInfo, ConnectionType, TerminalConnection};
}

// Re-export commonly used types at crate root
pub use config::{
    AppConfig, AppearanceConfig, CursorStyle, KeybindingsConfig, NetworkConfig, TerminalConfig,
};
pub use entities::{SessionId, TerminalSize};
pub use errors::{AuthError, ConfigError, ConnectionError, StorageError};
pub use state::{
    ConnectionEvent, ConnectionState, ConnectionStateMachine, DisconnectReason,
    StateChangeCallback, UiConnectionStatus,
};
pub use traits::{ConnectionInfo, ConnectionType, TerminalConnection};
