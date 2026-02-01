//! SFTP 文件管理模块
//!
//! 提供 SFTP 文件浏览和传输的 UI 组件。
//!
//! # 模块结构
//!
//! - `sftp_view` - 主 SFTP 视图，整合所有子组件
//! - `file_list` - 文件列表表格组件
//! - `path_bar` - 路径导航栏组件
//! - `transfer_queue` - 传输队列 UI 组件
//!
//! # 示例
//!
//! ```ignore
//! use zeterm::ui::sftp::{SftpView, SftpViewEvent};
//!
//! // 创建 SFTP 视图
//! let sftp_view = cx.new(|cx| SftpView::new(sftp_client, cx));
//!
//! // 订阅事件
//! cx.subscribe(&sftp_view, |this, _, event, cx| {
//!     match event {
//!         SftpViewEvent::FileSelected(path) => { /* ... */ }
//!         SftpViewEvent::TransferStarted(task_id) => { /* ... */ }
//!     }
//! });
//! ```

mod file_list;
mod path_bar;
mod sftp_view;
mod transfer_queue;

// ==================== 主视图 ====================
pub use sftp_view::{SftpView, SftpViewEvent, SftpViewMode};

// ==================== 文件列表 ====================
pub use file_list::{FileListEvent, FileListView, SortColumn, SortOrder};

// ==================== 路径导航 ====================
pub use path_bar::{PathBar, PathBarEvent};

// ==================== 传输队列 ====================
pub use transfer_queue::{TransferQueueEvent, TransferQueueView};
