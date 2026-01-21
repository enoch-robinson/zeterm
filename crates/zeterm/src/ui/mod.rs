//! UI 模块 - 表现层组件
//!
//! 包含所有 GPUI 视图组件和UI 相关实现。

// 允许暂时未使用但将来会用到的代码
#![allow(dead_code)]

mod app_theme;
mod components;
mod dialogs;
mod host_list;
mod main_window;
pub mod split_pane;
mod status_bar;
mod tab_manager;
mod tab_view;
pub mod terminal_view;

#[allow(unused_imports)]
pub use app_theme::{AppThemeManager, BuiltinTheme, ThemeConfig, ThemeEvent, ThemeMode};
pub use main_window::MainWindow;
#[allow(unused_imports)]
pub use status_bar::{ConnectionStatus, StatusBar, StatusInfo};
