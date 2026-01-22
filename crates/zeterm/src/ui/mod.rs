//! UI 模块 - 表现层组件
//!
//! 包含所有 GPUI 视图组件和 UI 相关实现。
//!
//! # 模块结构
//!
//! - `main_window` - 主窗口，整合所有 UI 组件
//! - `tab_manager` - Tab 管理器，管理标签页生命周期
//! - `tab_view` - Tab 视图，渲染标签栏和内容区域
//! - `split_pane` - 分屏布局，支持水平/垂直分屏
//! - `terminal_view` - 终端视图，渲染终端内容
//! - `status_bar` - 状态栏，显示连接状态等信息
//! - `app_theme` - 主题管理器，支持多种内置主题
//! - `host_list` - 主机列表视图
//! - `dialogs` - 对话框组件
//! - `components` - 通用 UI 组件
//! - `sftp` - SFTP 文件管理视图

// 允许暂时未使用但将来会用到的代码
#![allow(dead_code)]

mod app_theme;
mod components;
mod dialogs;
mod host_list;
mod main_window;
pub mod sftp;
pub mod split_pane;
mod status_bar;
mod tab_manager;
mod tab_view;
pub mod terminal_view;

// ==================== 主题相关 ====================
#[allow(unused_imports)]
pub use app_theme::{AppThemeManager, BuiltinTheme, ThemeConfig, ThemeEvent, ThemeMode};

// ==================== 主窗口 ====================
pub use main_window::MainWindow;

// ==================== Tab 管理 ====================
#[allow(unused_imports)]
pub use tab_manager::{TabData, TabId, TabInfo, TabManager, TabManagerEvent};
#[allow(unused_imports)]
pub use tab_view::TabView;

// ==================== 分屏布局 ====================
#[allow(unused_imports)]
pub use split_pane::{Pane, PaneContent, PaneId, SplitDirection, SplitManager, SplitView};

// ==================== 状态栏 ====================
#[allow(unused_imports)]
pub use status_bar::{ConnectionStatus, StatusBar, StatusInfo};

// ==================== 终端视图 ====================
#[allow(unused_imports)]
pub use terminal_view::TerminalView;

// ==================== SFTP 文件管理 ====================
#[allow(unused_imports)]
pub use sftp::{
    FileListEvent, FileListView, PathBar, PathBarEvent, SftpView, SftpViewEvent, SftpViewMode,
    SortColumn, SortOrder, TransferQueueEvent, TransferQueueView,
};
