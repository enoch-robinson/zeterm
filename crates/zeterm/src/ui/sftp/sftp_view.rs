//! SFTP 主视图组件
//!
//! 整合所有 SFTP 子组件，提供完整的文件管理界面，包括：
//! - 路径导航栏
//! - 文件列表
//! - 传输队列
//! - 工具栏操作

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::app::runtime;
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::ActiveTheme;

use parking_lot::RwLock;
use zeterm_ssh::{
    DirEntry, SftpClient, TransferDirection, TransferProgress, TransferState, TransferTask,
    TransferTaskId,
};

use super::file_list::{FileListEvent, FileListView};
use super::path_bar::{PathBar, PathBarEvent};
use super::transfer_queue::{TransferQueueEvent, TransferQueueView};

// ============================================================================
// 事件定义
// ============================================================================

/// SFTP 视图事件
#[derive(Debug, Clone)]
pub enum SftpViewEvent {
    /// 请求关闭视图
    CloseRequested,
    /// 文件被选中
    FileSelected(String),
    /// 文件被打开（双击）
    FileOpened(String),
    /// 请求下载文件
    DownloadRequested {
        remote_path: String,
        local_path: Option<String>,
    },
    /// 请求上传文件
    UploadRequested {
        local_paths: Vec<String>,
        remote_dir: String,
    },
    /// 传输开始
    TransferStarted(TransferTaskId),
    /// 传输完成
    TransferCompleted(TransferTaskId),
    /// 传输失败
    TransferFailed {
        task_id: TransferTaskId,
        error: String,
    },
    /// 错误发生
    Error(String),
}

// ============================================================================
// 视图模式
// ============================================================================

/// SFTP 视图模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SftpViewMode {
    /// 文件浏览模式
    #[default]
    Browse,
    /// 传输队列模式（全屏显示传输）
    Transfers,
    /// 分屏模式（左右两个目录）
    Split,
}

impl SftpViewMode {
    /// 获取模式标题
    pub fn title(&self) -> &'static str {
        match self {
            SftpViewMode::Browse => "Browse",
            SftpViewMode::Transfers => "Transfers",
            SftpViewMode::Split => "Split View",
        }
    }

    /// 获取模式图标
    pub fn icon(&self) -> &'static str {
        match self {
            SftpViewMode::Browse => "📁",
            SftpViewMode::Transfers => "📦",
            SftpViewMode::Split => "⧉",
        }
    }
}

// ============================================================================
// SFTP 视图
// ============================================================================

/// 目录加载结果（已废弃，使用 cx.spawn 直接处理）
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum LoadResult {
    /// 加载成功
    Success {
        path: String,
        entries: Vec<DirEntry>,
    },
    /// 加载失败
    Error { path: String, message: String },
}

/// 右键菜单状态
#[derive(Debug, Clone)]
struct ContextMenuState {
    /// 文件路径
    path: String,
    /// 文件名
    filename: String,
    /// 是否为目录
    is_directory: bool,
    /// 菜单位置
    position: (f32, f32),
}

/// 待处理的操作结果（已废弃，使用 cx.spawn 直接处理）
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum PendingOperationResult {
    /// 删除成功
    DeleteSuccess { path: String },
    /// 删除失败
    DeleteError { path: String, message: String },
    /// 重命名成功
    RenameSuccess { old_path: String, new_path: String },
    /// 重命名失败
    RenameError { path: String, message: String },
    /// 获取属性成功
    PropertiesLoaded { entry: DirEntry },
    /// 获取属性失败
    PropertiesError { path: String, message: String },
}

/// 重命名对话框状态
#[derive(Debug, Clone)]
struct RenameDialogState {
    /// 原路径
    old_path: String,
    /// 原文件名
    old_name: String,
    /// 新文件名（用户输入）
    new_name: String,
}

/// 属性对话框状态
#[derive(Debug, Clone)]
struct PropertiesDialogState {
    /// 文件条目信息
    entry: DirEntry,
}

/// 删除确认对话框状态
#[derive(Debug, Clone)]
struct DeleteConfirmState {
    /// 文件路径
    path: String,
    /// 文件名
    filename: String,
    /// 是否为目录
    is_directory: bool,
}

/// SFTP 视图组件
///
/// 整合路径栏、文件列表和传输队列，提供完整的 SFTP 文件管理界面。
pub struct SftpView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// SFTP 客户端（可选，因为可能还未连接）
    sftp_client: Option<Arc<SftpClient>>,
    /// 路径栏
    path_bar: Entity<PathBar>,
    /// 文件列表
    file_list: Entity<FileListView>,
    /// 传输队列
    transfer_queue: Entity<TransferQueueView>,
    /// 当前视图模式
    mode: SftpViewMode,
    /// 当前路径
    current_path: String,
    /// 主机名（用于显示）
    hostname: Option<String>,
    /// 用户名
    username: Option<String>,
    /// 是否显示传输队列
    show_transfer_queue: bool,
    /// 是否正在加载
    loading: bool,
    /// 错误消息
    error_message: Option<String>,
    /// 连接状态
    connected: bool,
    /// 正在加载的目录路径集合（防止重复加载）
    loading_paths: HashSet<String>,
    /// 传输任务映射（用于控制暂停/恢复/取消）
    transfer_tasks: Arc<RwLock<HashMap<TransferTaskId, TransferTask>>>,
    /// 右键菜单状态
    context_menu: Option<ContextMenuState>,
    /// 重命名对话框状态
    rename_dialog: Option<RenameDialogState>,
    /// 属性对话框状态
    properties_dialog: Option<PropertiesDialogState>,
    /// 删除确认对话框状态
    delete_confirm: Option<DeleteConfirmState>,
}

impl SftpView {
    /// 创建新的 SFTP 视图（未连接状态）
    pub fn new(cx: &mut Context<Self>) -> Self {
        let path_bar = cx.new(|cx| PathBar::new(cx));
        let file_list = cx.new(|cx| FileListView::new(cx));
        let transfer_queue = cx.new(|cx| TransferQueueView::new(cx));

        // 订阅子组件事件
        let this = Self {
            focus_handle: cx.focus_handle(),
            sftp_client: None,
            path_bar: path_bar.clone(),
            file_list: file_list.clone(),
            transfer_queue: transfer_queue.clone(),
            mode: SftpViewMode::Browse,
            current_path: "/".to_string(),
            hostname: None,
            username: None,
            show_transfer_queue: true,
            loading: false,
            error_message: None,
            connected: false,
            loading_paths: HashSet::new(),
            transfer_tasks: Arc::new(RwLock::new(HashMap::new())),
            context_menu: None,
            rename_dialog: None,
            properties_dialog: None,
            delete_confirm: None,
        };

        // 订阅路径栏事件
        cx.subscribe(&path_bar, Self::handle_path_bar_event)
            .detach();

        // 订阅文件列表事件
        cx.subscribe(&file_list, Self::handle_file_list_event)
            .detach();

        // 订阅传输队列事件
        cx.subscribe(&transfer_queue, Self::handle_transfer_queue_event)
            .detach();

        this
    }

    /// 创建并连接到 SFTP 客户端
    pub fn with_client(mut self, client: Arc<SftpClient>) -> Self {
        self.sftp_client = Some(client);
        self.connected = true;
        self
    }

    /// 设置 SFTP 客户端
    pub fn set_client(&mut self, client: Option<Arc<SftpClient>>, cx: &mut Context<Self>) {
        self.sftp_client = client;
        self.connected = self.sftp_client.is_some();
        if self.connected {
            self.error_message = None;
        }
        cx.notify();
    }

    /// 设置主机信息
    pub fn set_host_info(
        &mut self,
        hostname: Option<String>,
        username: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.hostname = hostname;
        self.username = username;

        // 更新路径栏的主目录
        if let Some(user) = &self.username {
            self.path_bar.update(cx, |path_bar, cx| {
                path_bar.set_home_path(Some(format!("/home/{}", user)), cx);
            });
        }

        cx.notify();
    }

    /// 设置视图模式
    pub fn set_mode(&mut self, mode: SftpViewMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    /// 设置是否显示传输队列
    pub fn set_show_transfer_queue(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_transfer_queue = show;
        cx.notify();
    }

    /// 切换传输队列显示
    pub fn toggle_transfer_queue(&mut self, cx: &mut Context<Self>) {
        self.show_transfer_queue = !self.show_transfer_queue;
        cx.notify();
    }

    /// 获取当前路径
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// 是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// 导航到路径
    pub fn navigate_to(&mut self, path: impl Into<String>, cx: &mut Context<Self>) {
        let path = path.into();
        self.current_path = path.clone();

        // 更新路径栏
        self.path_bar.update(cx, |path_bar, cx| {
            path_bar.set_path(&self.current_path, cx);
        });

        // 加载目录内容
        self.load_directory(cx);
    }

    /// 刷新当前目录
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_directory(cx);
    }

    /// 加载目录内容
    fn load_directory(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.sftp_client.clone() else {
            self.error_message = Some("Not connected to SFTP server".to_string());
            cx.notify();
            return;
        };

        let path = self.current_path.clone();

        // 防止重复加载同一目录
        if self.loading_paths.contains(&path) {
            tracing::debug!("Directory {} is already loading, skipping", path);
            return;
        }

        self.loading = true;
        self.error_message = None;
        self.loading_paths.insert(path.clone());

        // 更新文件列表状态
        self.file_list.update(cx, |file_list, cx| {
            file_list.set_loading(true, cx);
        });

        cx.notify();

        // 使用 cx.spawn 在 GPUI 上下文中执行异步任务
        // 完成后直接更新状态，无需在 render 中轮询
        cx.spawn(async move |this, cx| {
            let result = client.list_dir(&path).await;

            // 直接在 GPUI 上下文中更新状态
            let _ = this.update(cx, |this, cx| {
                // 移除加载标记
                this.loading_paths.remove(&path);

                match result {
                    Ok(entries) => {
                        tracing::info!("SFTP: Loaded {} entries from {}", entries.len(), path);

                        // 验证路径是否仍然是当前路径（可能用户已导航到其他目录）
                        if path == this.current_path {
                            // 更新文件列表
                            this.file_list.update(cx, |file_list, cx| {
                                file_list.set_entries(entries, cx);
                            });

                            // 更新状态
                            this.loading = false;
                            this.error_message = None;
                        } else {
                            tracing::debug!(
                                "Ignoring stale load result for {} (current path is {})",
                                path,
                                this.current_path
                            );
                            this.loading = false;
                        }
                    },
                    Err(e) => {
                        tracing::error!("SFTP: Failed to load directory {}: {}", path, e);

                        // 验证路径
                        if path == this.current_path {
                            // 更新错误状态
                            this.loading = false;
                            this.error_message = Some(e.to_string());

                            // 更新文件列表显示错误
                            this.file_list.update(cx, |file_list, cx| {
                                file_list.set_error(Some(e.to_string()), cx);
                            });
                        } else {
                            this.loading = false;
                        }
                    },
                }

                cx.notify();
            });
        })
        .detach();
    }

    /// 处理路径栏事件
    fn handle_path_bar_event(
        &mut self,
        _path_bar: Entity<PathBar>,
        event: &PathBarEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            PathBarEvent::NavigateTo(path) => {
                self.navigate_to(path.clone(), cx);
            },
            PathBarEvent::GoUp => {
                if self.current_path != "/" {
                    let parent = std::path::Path::new(&self.current_path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "/".to_string());
                    self.navigate_to(parent, cx);
                }
            },
            PathBarEvent::GoBack => {
                // 路径栏内部处理历史
            },
            PathBarEvent::GoForward => {
                // 路径栏内部处理历史
            },
            PathBarEvent::Refresh => {
                self.refresh(cx);
            },
            PathBarEvent::GoHome => {
                if let Some(user) = &self.username {
                    self.navigate_to(format!("/home/{}", user), cx);
                }
            },
        }
    }

    /// 处理文件列表事件
    fn handle_file_list_event(
        &mut self,
        _file_list: Entity<FileListView>,
        event: &FileListEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FileListEvent::Selected(path) => {
                cx.emit(SftpViewEvent::FileSelected(path.clone()));
            },
            FileListEvent::Opened(path) => {
                // 检查是否为目录
                // 简单判断：如果路径以目录分隔符结尾或者没有扩展名可能是目录
                // 实际应该检查 entry_type
                self.navigate_to(path.clone(), cx);
                cx.emit(SftpViewEvent::FileOpened(path.clone()));
            },
            FileListEvent::MultiSelected(_paths) => {
                // 多选不触发事件，等待操作
            },
            FileListEvent::ContextMenu { path, position } => {
                // 显示右键菜单
                self.show_context_menu(path.clone(), *position, cx);
            },
            FileListEvent::SortChanged { column, order } => {
                // 排序在文件列表内部处理
                let _ = (column, order);
            },
            FileListEvent::UploadRequested(paths) => {
                cx.emit(SftpViewEvent::UploadRequested {
                    local_paths: paths.clone(),
                    remote_dir: self.current_path.clone(),
                });
            },
        }
    }

    /// 处理传输队列事件
    fn handle_transfer_queue_event(
        &mut self,
        _transfer_queue: Entity<TransferQueueView>,
        event: &TransferQueueEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TransferQueueEvent::PauseRequested(id) => {
                self.pause_transfer(*id, cx);
            },
            TransferQueueEvent::ResumeRequested(id) => {
                self.resume_transfer(*id, cx);
            },
            TransferQueueEvent::CancelRequested(id) => {
                self.cancel_transfer(*id, cx);
            },
            TransferQueueEvent::RetryRequested(id) => {
                self.retry_transfer(*id, cx);
            },
            TransferQueueEvent::ClearCompleted => {
                self.transfer_queue.update(cx, |queue, cx| {
                    queue.clear_completed(cx);
                });
            },
            TransferQueueEvent::ClearAll => {
                self.transfer_queue.update(cx, |queue, cx| {
                    queue.clear_all(cx);
                });
            },
            TransferQueueEvent::OpenDestination(path) => {
                // 在文件管理器中打开
                let _ = path;
            },
        }
    }

    /// 添加传输任务到队列
    pub fn add_transfer(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        self.transfer_queue.update(cx, |queue, cx| {
            queue.add_transfer(id, progress, cx);
        });
        cx.emit(SftpViewEvent::TransferStarted(id));
    }

    /// 更新传输进度
    pub fn update_transfer_progress(
        &mut self,
        id: TransferTaskId,
        progress: TransferProgress,
        cx: &mut Context<Self>,
    ) {
        self.transfer_queue.update(cx, |queue, cx| {
            queue.update_progress(id, progress, cx);
        });
    }

    /// 暂停传输任务
    fn pause_transfer(&mut self, id: TransferTaskId, cx: &mut Context<Self>) {
        tracing::info!("Pausing transfer: {}", id);

        // 设置任务暂停标志
        if let Some(task) = self.transfer_tasks.write().get_mut(&id) {
            task.pause();
            task.set_state(TransferState::Paused);
        }

        // 更新 UI
        self.transfer_queue.update(cx, |queue, cx| {
            queue.update_transfer_state(id, TransferState::Paused, cx);
        });
    }

    /// 恢复传输任务
    fn resume_transfer(&mut self, id: TransferTaskId, cx: &mut Context<Self>) {
        tracing::info!("Resuming transfer: {}", id);

        // 清除暂停标志
        if let Some(task) = self.transfer_tasks.write().get_mut(&id) {
            task.resume();
            task.set_state(TransferState::InProgress);
        }

        // 更新 UI
        self.transfer_queue.update(cx, |queue, cx| {
            queue.update_transfer_state(id, TransferState::InProgress, cx);
        });
    }

    /// 取消传输任务
    fn cancel_transfer(&mut self, id: TransferTaskId, cx: &mut Context<Self>) {
        tracing::info!("Cancelling transfer: {}", id);

        // 设置取消标志
        if let Some(task) = self.transfer_tasks.write().get_mut(&id) {
            task.cancel();
            task.mark_cancelled();
        }

        // 更新 UI
        self.transfer_queue.update(cx, |queue, cx| {
            queue.update_transfer_state(id, TransferState::Cancelled, cx);
        });

        // 发出取消事件
        cx.emit(SftpViewEvent::TransferFailed {
            task_id: id,
            error: "Transfer cancelled by user".to_string(),
        });
    }

    /// 重试传输任务
    fn retry_transfer(&mut self, id: TransferTaskId, cx: &mut Context<Self>) {
        tracing::info!("Retrying transfer: {}", id);

        // 获取原始任务信息
        let task_info = {
            let tasks = self.transfer_tasks.read();
            tasks.get(&id).map(|t| {
                (
                    t.progress.direction.clone(),
                    t.progress.source_path.clone(),
                    t.progress.dest_path.clone(),
                    t.progress.total_bytes,
                )
            })
        };

        if let Some((direction, source, dest, total)) = task_info {
            // 从旧任务映射中移除
            self.transfer_tasks.write().remove(&id);

            // 从传输队列中移除旧任务
            self.transfer_queue.update(cx, |queue, cx| {
                queue.remove_transfer(id, cx);
            });

            // 创建新任务（使用 SFTP 客户端的下一个任务 ID）
            if let Some(client) = &self.sftp_client {
                let new_id = client.next_task_id();
                let new_task = TransferTask::new(new_id, direction.clone(), &source, &dest, total);

                // 获取控制标志
                let cancel_flag = new_task.cancel_flag();

                // 添加到任务映射
                self.transfer_tasks.write().insert(new_id, new_task);

                // 添加到传输队列 UI
                let progress =
                    TransferProgress::new(direction.clone(), source.clone(), dest.clone(), total);
                self.transfer_queue.update(cx, |queue, cx| {
                    queue.add_transfer(new_id, progress.clone(), cx);
                });

                tracing::info!("Created new transfer task: {} (retry of {})", new_id, id);

                // 启动实际的传输操作
                let client_clone = client.clone();
                let transfer_tasks = self.transfer_tasks.clone();
                let source_clone = source.clone();
                let dest_clone = dest.clone();
                let direction_clone = direction.clone();

                runtime::spawn_blocking(async move {
                    let result = match direction_clone {
                        TransferDirection::Download => {
                            client_clone
                                .download_file_cancellable(
                                    &source_clone,
                                    &dest_clone,
                                    cancel_flag,
                                    None,
                                )
                                .await
                        },
                        TransferDirection::Upload => {
                            client_clone
                                .upload_file_cancellable(
                                    &source_clone,
                                    &dest_clone,
                                    cancel_flag,
                                    None,
                                )
                                .await
                        },
                    };

                    // 更新任务状态
                    if let Some(task) = transfer_tasks.write().get_mut(&new_id) {
                        match result {
                            Ok(progress) => {
                                task.progress = progress;
                                task.mark_completed();
                                tracing::info!("Transfer {} completed successfully", new_id);
                            },
                            Err(e) => {
                                task.set_error(e.to_string());
                                tracing::error!("Transfer {} failed: {}", new_id, e);
                            },
                        }
                    }
                });

                cx.emit(SftpViewEvent::TransferStarted(new_id));
            }
        } else {
            tracing::warn!("Cannot retry transfer {}: task not found", id);
        }
    }

    /// 注册传输任务（供外部调用以便控制）
    pub fn register_transfer_task(&mut self, task: TransferTask) {
        let id = task.id;
        self.transfer_tasks.write().insert(id, task);
    }

    /// 获取传输任务的暂停标志（供异步传输使用）
    pub fn get_pause_flag(
        &self,
        id: TransferTaskId,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        self.transfer_tasks.read().get(&id).map(|t| t.pause_flag())
    }

    /// 获取传输任务的取消标志（供异步传输使用）
    pub fn get_cancel_flag(
        &self,
        id: TransferTaskId,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        self.transfer_tasks.read().get(&id).map(|t| t.cancel_flag())
    }

    /// 显示右键菜单
    fn show_context_menu(&mut self, path: String, position: (f32, f32), cx: &mut Context<Self>) {
        // 从路径提取文件名
        let filename = path.rsplit('/').next().unwrap_or(&path).to_string();

        // 判断是否为目录（简单判断：以 / 结尾或无扩展名）
        let is_directory = path.ends_with('/') || !filename.contains('.');

        self.context_menu = Some(ContextMenuState {
            path,
            filename,
            is_directory,
            position,
        });
        cx.notify();
    }

    /// 隐藏右键菜单
    fn hide_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.notify();
    }

    /// 处理右键菜单操作：下载
    fn context_menu_download(&mut self, cx: &mut Context<Self>) {
        if let Some(ref menu) = self.context_menu {
            let path = menu.path.clone();
            tracing::info!("Context menu: Download {}", path);
            cx.emit(SftpViewEvent::DownloadRequested {
                remote_path: path,
                local_path: None,
            });
        }
        self.hide_context_menu(cx);
    }

    /// 处理右键菜单操作：删除
    fn context_menu_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(ref menu) = self.context_menu {
            // 显示删除确认对话框
            self.delete_confirm = Some(DeleteConfirmState {
                path: menu.path.clone(),
                filename: menu.filename.clone(),
                is_directory: menu.is_directory,
            });
            tracing::info!("Context menu: Show delete confirm for {}", menu.path);
        }
        self.hide_context_menu(cx);
    }

    /// 执行删除操作
    fn do_delete(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.delete_confirm.take() else {
            return;
        };

        let Some(client) = self.sftp_client.clone() else {
            self.error_message = Some("未连接到 SFTP 服务器".to_string());
            cx.notify();
            return;
        };

        let path = confirm.path.clone();
        let is_directory = confirm.is_directory;
        let current_path = self.current_path.clone();

        tracing::info!("Executing delete: {} (is_dir: {})", path, is_directory);

        // 使用 cx.spawn 在 GPUI 上下文中执行异步任务
        cx.spawn(async move |this, cx| {
            let result = if is_directory {
                client.rmdir_all(&path).await
            } else {
                client.remove(&path).await
            };

            // 直接在 GPUI 上下文中更新状态
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        tracing::info!("Successfully deleted: {}", path);
                        // 如果被删除的文件在当前目录，刷新目录
                        if path.starts_with(&current_path) {
                            this.refresh(cx);
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to delete {}: {}", path, e);
                        this.error_message = Some(format!("删除失败: {}", e));
                        cx.notify();
                    },
                }
            });
        })
        .detach();

        cx.notify();
    }

    /// 取消删除操作
    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.delete_confirm = None;
        cx.notify();
    }

    /// 处理右键菜单操作：重命名
    fn context_menu_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(ref menu) = self.context_menu {
            // 显示重命名对话框
            self.rename_dialog = Some(RenameDialogState {
                old_path: menu.path.clone(),
                old_name: menu.filename.clone(),
                new_name: menu.filename.clone(),
            });
            tracing::info!("Context menu: Show rename dialog for {}", menu.path);
        }
        self.hide_context_menu(cx);
    }

    /// 更新重命名对话框的新名称
    fn update_rename_input(&mut self, new_name: String, cx: &mut Context<Self>) {
        if let Some(ref mut dialog) = self.rename_dialog {
            dialog.new_name = new_name;
            cx.notify();
        }
    }

    /// 执行重命名操作
    fn do_rename(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.rename_dialog.take() else {
            return;
        };

        if dialog.new_name.is_empty() || dialog.new_name == dialog.old_name {
            // 名称为空或未改变，取消操作
            cx.notify();
            return;
        }

        let Some(client) = self.sftp_client.clone() else {
            self.error_message = Some("未连接到 SFTP 服务器".to_string());
            cx.notify();
            return;
        };

        // 构建新路径
        let old_path = dialog.old_path.clone();
        let new_path = if let Some(parent) = old_path.rsplit_once('/') {
            format!("{}/{}", parent.0, dialog.new_name)
        } else {
            dialog.new_name.clone()
        };

        tracing::info!("Executing rename: {} -> {}", old_path, new_path);

        // 使用 cx.spawn 在 GPUI 上下文中执行异步任务
        cx.spawn(async move |this, cx| {
            let result = client.rename(&old_path, &new_path).await;

            // 直接在 GPUI 上下文中更新状态
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        tracing::info!("Successfully renamed: {} -> {}", old_path, new_path);
                        // 刷新当前目录
                        this.refresh(cx);
                    },
                    Err(e) => {
                        tracing::error!("Failed to rename {}: {}", old_path, e);
                        this.error_message = Some(format!("重命名失败: {}", e));
                        cx.notify();
                    },
                }
            });
        })
        .detach();

        cx.notify();
    }

    /// 取消重命名操作
    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.rename_dialog = None;
        cx.notify();
    }

    /// 处理右键菜单操作：复制路径
    fn context_menu_copy_path(&mut self, cx: &mut Context<Self>) {
        if let Some(ref menu) = self.context_menu {
            let path = menu.path.clone();
            tracing::info!("Context menu: Copy path {}", path);

            // 复制到剪贴板
            let item = ClipboardItem::new_string(path.clone());
            cx.write_to_clipboard(item);

            tracing::info!("Path copied to clipboard: {}", path);
        }
        self.hide_context_menu(cx);
    }

    /// 处理右键菜单操作：查看属性
    fn context_menu_properties(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.context_menu.take() else {
            return;
        };

        let path = menu.path.clone();
        tracing::info!("Context menu: Properties {}", path);

        let Some(client) = self.sftp_client.clone() else {
            self.error_message = Some("未连接到 SFTP 服务器".to_string());
            cx.notify();
            return;
        };

        // 使用 cx.spawn 在 GPUI 上下文中执行异步任务
        cx.spawn(async move |this, cx| {
            let result = client.stat(&path).await;

            // 直接在 GPUI 上下文中更新状态
            let _ = this.update(cx, |this, cx| match result {
                Ok(entry) => {
                    tracing::info!("Loaded properties for: {}", path);
                    this.properties_dialog = Some(PropertiesDialogState { entry });
                    cx.notify();
                },
                Err(e) => {
                    tracing::error!("Failed to get properties for {}: {}", path, e);
                    this.error_message = Some(format!("获取属性失败: {}", e));
                    cx.notify();
                },
            });
        })
        .detach();
    }

    /// 关闭属性对话框
    fn close_properties_dialog(&mut self, cx: &mut Context<Self>) {
        self.properties_dialog = None;
        cx.notify();
    }

    /// 渲染右键菜单
    fn render_context_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let Some(ref menu) = self.context_menu else {
            return div().into_any_element();
        };

        let is_dir = menu.is_directory;

        div()
            .id("sftp-context-menu")
            .absolute()
            .left(px(menu.position.0))
            .top(px(menu.position.1))
            .w_48()
            .py_1()
            .rounded_md()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            // 阻止点击事件冒泡
            .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
            .child(
                div()
                    .flex()
                    .flex_col()
                    // 打开/进入
                    .child(
                        div()
                            .id("ctx-open")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.muted))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    if let Some(ref menu) = this.context_menu {
                                        let path = menu.path.clone();
                                        let is_dir = menu.is_directory;
                                        this.hide_context_menu(cx);
                                        if is_dir {
                                            this.navigate_to(&path, cx);
                                        } else {
                                            cx.emit(SftpViewEvent::FileOpened(path));
                                        }
                                    }
                                }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(if is_dir { "📂" } else { "📄" })
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.foreground)
                                            .child(if is_dir { "打开" } else { "查看" }),
                                    ),
                            ),
                    )
                    // 下载
                    .child(
                        div()
                            .id("ctx-download")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.muted))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.context_menu_download(cx);
                                }),
                            )
                            .child(
                                div().flex().items_center().gap_2().child("⬇️").child(
                                    div().text_sm().text_color(theme.foreground).child("下载"),
                                ),
                            ),
                    )
                    // 分隔线
                    .child(div().h_px().my_1().mx_2().bg(theme.border))
                    // 重命名
                    .child(
                        div()
                            .id("ctx-rename")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.muted))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.context_menu_rename(cx);
                                }),
                            )
                            .child(div().flex().items_center().gap_2().child("✏️").child(
                                div().text_sm().text_color(theme.foreground).child("重命名"),
                            )),
                    )
                    // 复制路径
                    .child(
                        div()
                            .id("ctx-copy-path")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.muted))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.context_menu_copy_path(cx);
                                }),
                            )
                            .child(
                                div().flex().items_center().gap_2().child("📋").child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.foreground)
                                        .child("复制路径"),
                                ),
                            ),
                    )
                    // 分隔线
                    .child(div().h_px().my_1().mx_2().bg(theme.border))
                    // 删除
                    .child(
                        div()
                            .id("ctx-delete")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(gpui::hsla(0.0, 0.7, 0.5, 0.1)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.context_menu_delete(cx);
                                }),
                            )
                            .child(
                                div().flex().items_center().gap_2().child("🗑️").child(
                                    div()
                                        .text_sm()
                                        .text_color(gpui::hsla(0.0, 0.7, 0.5, 1.0))
                                        .child("删除"),
                                ),
                            ),
                    )
                    // 分隔线
                    .child(div().h_px().my_1().mx_2().bg(theme.border))
                    // 属性
                    .child(
                        div()
                            .id("ctx-properties")
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .hover(|el| el.bg(theme.muted))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.context_menu_properties(cx);
                                }),
                            )
                            .child(
                                div().flex().items_center().gap_2().child("ℹ️").child(
                                    div().text_sm().text_color(theme.foreground).child("属性"),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 渲染删除确认对话框
    fn render_delete_confirm_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let Some(ref confirm) = self.delete_confirm else {
            return div().into_any_element();
        };

        let type_name = if confirm.is_directory {
            "目录"
        } else {
            "文件"
        };

        div()
            .id("delete-confirm-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .child(
                div()
                    .id("delete-confirm-dialog")
                    .w_80()
                    .p_4()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    // 阻止点击事件冒泡
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    // 标题
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_3()
                            .child(div().text_color(gpui::hsla(0.0, 0.7, 0.5, 1.0)).child("⚠️"))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(format!("删除{}", type_name)),
                            ),
                    )
                    // 描述
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .mb_4()
                            .child(format!(
                                "确定要删除{} \"{}\" 吗？此操作不可撤销。",
                                type_name, confirm.filename
                            )),
                    )
                    // 按钮
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            // 取消按钮
                            .child(
                                div()
                                    .id("delete-cancel")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .cursor_pointer()
                                    .hover(|el| el.bg(theme.muted))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.cancel_delete(cx);
                                        }),
                                    )
                                    .child("取消"),
                            )
                            // 删除按钮
                            .child(
                                div()
                                    .id("delete-confirm")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(gpui::hsla(0.0, 0.7, 0.5, 1.0))
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|el| el.bg(gpui::hsla(0.0, 0.8, 0.4, 1.0)))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.do_delete(cx);
                                        }),
                                    )
                                    .child("删除"),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 渲染重命名对话框
    fn render_rename_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let Some(ref dialog) = self.rename_dialog else {
            return div().into_any_element();
        };

        let new_name = dialog.new_name.clone();

        div()
            .id("rename-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .child(
                div()
                    .id("rename-dialog")
                    .w_80()
                    .p_4()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    // 标题
                    .child(
                        div()
                            .text_base()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .mb_3()
                            .child("重命名"),
                    )
                    // 输入框
                    .child(
                        div().w_full().mb_4().child(
                            div()
                                .id("rename-input")
                                .w_full()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.secondary)
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(new_name),
                        ),
                    )
                    // 按钮
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("rename-cancel")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .cursor_pointer()
                                    .hover(|el| el.bg(theme.muted))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.cancel_rename(cx);
                                        }),
                                    )
                                    .child("取消"),
                            )
                            .child(
                                div()
                                    .id("rename-confirm")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.primary)
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .hover(|el| el.opacity(0.9))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.do_rename(cx);
                                        }),
                                    )
                                    .child("确定"),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// 渲染属性对话框
    fn render_properties_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        let Some(ref dialog) = self.properties_dialog else {
            return div().into_any_element();
        };

        let entry = &dialog.entry;
        let type_name = if entry.is_dir() { "目录" } else { "文件" };
        let size_str = entry.formatted_size();
        let modified_str = entry.formatted_modified();
        let perms_str = entry
            .permissions
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_else(|| "---------".to_string());

        div()
            .id("properties-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.5))
            .child(
                div()
                    .id("properties-dialog")
                    .w_80()
                    .p_4()
                    .rounded_lg()
                    .bg(theme.background)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_lg()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, _| {})
                    // 标题
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb_4()
                            .child(if entry.is_dir() { "📁" } else { "📄" })
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(entry.name.clone()),
                            ),
                    )
                    // 属性列表
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .text_sm()
                            // 类型
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .w_20()
                                            .text_color(theme.muted_foreground)
                                            .child("类型"),
                                    )
                                    .child(div().text_color(theme.foreground).child(type_name)),
                            )
                            // 路径
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .w_20()
                                            .text_color(theme.muted_foreground)
                                            .child("路径"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_color(theme.foreground)
                                            .overflow_hidden()
                                            .child(entry.path.clone()),
                                    ),
                            )
                            // 大小
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .w_20()
                                            .text_color(theme.muted_foreground)
                                            .child("大小"),
                                    )
                                    .child(div().text_color(theme.foreground).child(size_str)),
                            )
                            // 修改时间
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .w_20()
                                            .text_color(theme.muted_foreground)
                                            .child("修改时间"),
                                    )
                                    .child(div().text_color(theme.foreground).child(modified_str)),
                            )
                            // 权限
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        div()
                                            .w_20()
                                            .text_color(theme.muted_foreground)
                                            .child("权限"),
                                    )
                                    .child(div().text_color(theme.foreground).child(perms_str)),
                            ),
                    )
                    // 关闭按钮
                    .child(
                        div().flex().justify_end().mt_4().child(
                            div()
                                .id("properties-close")
                                .px_4()
                                .py_1()
                                .rounded_md()
                                .bg(theme.primary)
                                .text_sm()
                                .text_color(gpui::white())
                                .cursor_pointer()
                                .hover(|el| el.opacity(0.9))
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.close_properties_dialog(cx);
                                    }),
                                )
                                .child("关闭"),
                        ),
                    ),
            )
            .into_any_element()
    }

    /// 渲染工具栏
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let connected = self.connected;

        div()
            .w_full()
            .h(px(40.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme.secondary)
            .border_b_1()
            .border_color(theme.border)
            // 左侧：连接信息
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(if connected {
                                gpui::hsla(0.33, 0.7, 0.45, 1.0)
                            } else {
                                theme.muted_foreground
                            })
                            .child(if connected { "●" } else { "○" }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(SharedString::from(
                                self.hostname
                                    .clone()
                                    .unwrap_or_else(|| "Not connected".to_string()),
                            )),
                    )
                    .when(self.username.is_some(), |el| {
                        el.child(div().text_sm().text_color(theme.muted_foreground).child(
                            SharedString::from(format!("({})", self.username.as_ref().unwrap())),
                        ))
                    }),
            )
            // 右侧：操作按钮
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    // 上传按钮
                    .child(
                        div()
                            .id("btn-upload")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if connected {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("⬆️ Upload"),
                    )
                    // 新建文件夹按钮
                    .child(
                        div()
                            .id("btn-new-folder")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if connected {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("📁 New Folder"),
                    )
                    // 传输队列切换
                    .child(
                        div()
                            .id("btn-toggle-transfers")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .text_color(if self.show_transfer_queue {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .when(self.show_transfer_queue, |el| {
                                el.bg(theme.primary.opacity(0.1))
                            })
                            .hover(|el| el.bg(theme.muted.opacity(0.2)))
                            .child("📦 Transfers"),
                    )
                    // 关闭按钮
                    .child(
                        div()
                            .id("btn-close")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .hover(|el| {
                                el.bg(gpui::hsla(0.0, 0.8, 0.5, 0.2))
                                    .text_color(gpui::hsla(0.0, 0.8, 0.5, 1.0))
                            })
                            .child("✕"),
                    ),
            )
    }

    /// 渲染未连接状态
    fn render_disconnected(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .text_color(theme.muted_foreground)
                    .child("🔌"),
            )
            .child(
                div()
                    .text_lg()
                    .text_color(theme.foreground)
                    .child("Not connected"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Connect to an SSH server to browse files"),
            )
    }

    /// 渲染主内容区域
    fn render_content(&self, cx: &Context<Self>) -> impl IntoElement {
        let _theme = cx.theme();

        div()
            .w_full()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            // 路径栏
            .child(self.path_bar.clone())
            // 文件列表
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .overflow_hidden()
                    .child(self.file_list.clone()),
            )
    }

    /// 渲染传输队列面板
    fn render_transfer_panel(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .max_h(px(250.0))
            .p_2()
            .child(self.transfer_queue.clone())
    }

    // 注意：process_pending_load_result 和 process_pending_operation 已移除
    // 现在使用 cx.spawn 在 GPUI 上下文中直接处理异步任务结果
    // 这消除了 render() 方法中的状态修改问题
}

impl EventEmitter<SftpViewEvent> for SftpView {}

impl Focusable for SftpView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SftpView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 注意：不在 render 中处理异步结果
        // 所有异步操作现在使用 cx.spawn 在 GPUI 上下文中直接更新状态
        // 参见 load_directory, do_delete, do_rename, context_menu_properties 方法

        let theme = cx.theme();
        let connected = self.connected;
        let show_transfers = self.show_transfer_queue;

        div()
            .id("sftp-view")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_lg()
            .overflow_hidden()
            // 工具栏
            .child(self.render_toolbar(cx))
            // 主内容
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .relative()
                    .child(if connected {
                        self.render_content(cx).into_any_element()
                    } else {
                        self.render_disconnected(cx).into_any_element()
                    })
                    // 右键菜单
                    .when(self.context_menu.is_some(), |el| {
                        el.child(self.render_context_menu(cx))
                    })
                    // 点击其他区域关闭右键菜单
                    .when(self.context_menu.is_some(), |el| {
                        el.on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.hide_context_menu(cx);
                            }),
                        )
                    })
                    // 删除确认对话框
                    .when(self.delete_confirm.is_some(), |el| {
                        el.child(self.render_delete_confirm_dialog(cx))
                    })
                    // 重命名对话框
                    .when(self.rename_dialog.is_some(), |el| {
                        el.child(self.render_rename_dialog(cx))
                    })
                    // 属性对话框
                    .when(self.properties_dialog.is_some(), |el| {
                        el.child(self.render_properties_dialog(cx))
                    }),
            )
            // 传输队列（可选）
            .when(show_transfers && connected, |el| {
                el.child(self.render_transfer_panel(cx))
            })
    }
}

/// 创建 SFTP 视图的便捷方法
pub fn create_sftp_view(cx: &mut Context<SftpView>) -> SftpView {
    SftpView::new(cx)
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sftp_view_mode_title() {
        assert_eq!(SftpViewMode::Browse.title(), "Browse");
        assert_eq!(SftpViewMode::Transfers.title(), "Transfers");
        assert_eq!(SftpViewMode::Split.title(), "Split View");
    }

    #[test]
    fn test_sftp_view_mode_icon() {
        assert_eq!(SftpViewMode::Browse.icon(), "📁");
        assert_eq!(SftpViewMode::Transfers.icon(), "📦");
        assert_eq!(SftpViewMode::Split.icon(), "⧉");
    }

    #[test]
    fn test_sftp_view_event_debug() {
        let event = SftpViewEvent::FileSelected("/test/path".to_string());
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("FileSelected"));

        let event2 = SftpViewEvent::CloseRequested;
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("CloseRequested"));
    }

    #[test]
    fn test_sftp_view_event_transfer() {
        let event = SftpViewEvent::TransferStarted(TransferTaskId(42));
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("TransferStarted"));

        let event2 = SftpViewEvent::TransferFailed {
            task_id: TransferTaskId(1),
            error: "Connection lost".to_string(),
        };
        let debug_str2 = format!("{:?}", event2);
        assert!(debug_str2.contains("TransferFailed"));
        assert!(debug_str2.contains("Connection lost"));
    }

    #[test]
    fn test_sftp_view_event_download() {
        let event = SftpViewEvent::DownloadRequested {
            remote_path: "/remote/file.txt".to_string(),
            local_path: Some("/local/file.txt".to_string()),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("DownloadRequested"));
        assert!(debug_str.contains("remote/file.txt"));
    }

    #[test]
    fn test_sftp_view_event_upload() {
        let event = SftpViewEvent::UploadRequested {
            local_paths: vec!["/local/a.txt".to_string(), "/local/b.txt".to_string()],
            remote_dir: "/remote/dir".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("UploadRequested"));
        assert!(debug_str.contains("remote/dir"));
    }
}
