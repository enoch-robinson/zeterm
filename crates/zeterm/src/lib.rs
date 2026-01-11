//! Zeterm - 现代化 SSH 终端客户端
//!
//! 基于 Rust + GPUI 的跨平台 SSH 终端平台。
//!
//! # 模块结构
//!
//! - `app` - 应用层，包含会话协调器和终端状态管理
//! - `logging` - 日志系统配置
//! - `ui` - 表现层，包含终端视图和主窗口

pub mod app;
pub mod logging;
pub mod ui;
