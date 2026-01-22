//! SFTP 客户端模块
//!
//! 提供基于 russh-sftp 的 SFTP 文件操作功能。
//!
//! # 功能特性
//!
//! - 目录列表和导航
//! - 文件上传/下载
//! - 文件/目录创建、删除、重命名
//! - 传输进度回调
//! - 传输队列管理
//!
//! # 示例
//!
//! ```ignore
//! use zeterm_ssh::{SshConnection, SftpClient};
//!
//! // 从已连接的 SSH 会话创建 SFTP 客户端
//! let sftp = SftpClient::from_ssh_session(&ssh_session).await?;
//!
//! // 列出目录
//! let entries = sftp.list_dir("/home/user").await?;
//!
//! // 下载文件
//! sftp.download_file("/remote/file.txt", "/local/file.txt", |progress| {
//!     println!("Progress: {}%", progress.percentage());
//! }).await?;
//! ```

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use russh::client::Handle;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, FileType, OpenFlags};
use thiserror::Error;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::handler::SshHandler;

// ============================================================================
// 错误类型
// ============================================================================

/// SFTP 错误类型
#[derive(Debug, Error)]
pub enum SftpError {
    /// 连接错误
    #[error("SFTP connection error: {0}")]
    Connection(String),

    /// 会话未建立
    #[error("SFTP session not established")]
    NotConnected,

    /// 文件不存在
    #[error("File not found: {0}")]
    FileNotFound(String),

    /// 目录不存在
    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),

    /// 权限被拒绝
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// 文件已存在
    #[error("File already exists: {0}")]
    FileExists(String),

    /// 目录非空
    #[error("Directory not empty: {0}")]
    DirectoryNotEmpty(String),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 协议错误
    #[error("SFTP protocol error: {0}")]
    Protocol(String),

    /// 传输被取消
    #[error("Transfer cancelled")]
    Cancelled,

    /// 传输超时
    #[error("Transfer timeout")]
    Timeout,

    /// 无效路径
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// 其他错误
    #[error("SFTP error: {0}")]
    Other(String),
}

impl From<russh_sftp::client::error::Error> for SftpError {
    fn from(err: russh_sftp::client::error::Error) -> Self {
        let msg = err.to_string();
        if msg.contains("No such file") {
            SftpError::FileNotFound(msg)
        } else if msg.contains("Permission denied") {
            SftpError::PermissionDenied(msg)
        } else if msg.contains("File exists") {
            SftpError::FileExists(msg)
        } else if msg.contains("Directory not empty") {
            SftpError::DirectoryNotEmpty(msg)
        } else {
            SftpError::Protocol(msg)
        }
    }
}

impl From<russh::Error> for SftpError {
    fn from(err: russh::Error) -> Self {
        SftpError::Connection(err.to_string())
    }
}

/// SFTP 操作结果类型
pub type SftpResult<T> = Result<T, SftpError>;

// ============================================================================
// 文件条目类型
// ============================================================================

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryType {
    /// 普通文件
    File,
    /// 目录
    Directory,
    /// 符号链接
    Symlink,
    /// 其他类型
    Other,
}

impl EntryType {
    /// 获取类型图标
    pub fn icon(&self) -> &'static str {
        match self {
            EntryType::File => "📄",
            EntryType::Directory => "📁",
            EntryType::Symlink => "🔗",
            EntryType::Other => "❓",
        }
    }

    /// 是否为目录
    pub fn is_dir(&self) -> bool {
        matches!(self, EntryType::Directory)
    }

    /// 是否为文件
    pub fn is_file(&self) -> bool {
        matches!(self, EntryType::File)
    }
}

impl From<FileType> for EntryType {
    fn from(ft: FileType) -> Self {
        match ft {
            FileType::File => EntryType::File,
            FileType::Dir => EntryType::Directory,
            FileType::Symlink => EntryType::Symlink,
            FileType::Other => EntryType::Other,
        }
    }
}

/// 文件权限
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilePermissions {
    /// 权限位 (Unix 模式)
    pub mode: u32,
}

impl FilePermissions {
    /// 创建新的权限
    pub fn new(mode: u32) -> Self {
        Self { mode }
    }

    /// 是否可读（所有者）
    pub fn owner_read(&self) -> bool {
        self.mode & 0o400 != 0
    }

    /// 是否可写（所有者）
    pub fn owner_write(&self) -> bool {
        self.mode & 0o200 != 0
    }

    /// 是否可执行（所有者）
    pub fn owner_execute(&self) -> bool {
        self.mode & 0o100 != 0
    }

    /// 转换为 Unix 权限字符串 (如 "rwxr-xr-x")
    pub fn to_string_unix(&self) -> String {
        let mut s = String::with_capacity(9);

        // Owner
        s.push(if self.mode & 0o400 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o200 != 0 { 'w' } else { '-' });
        s.push(if self.mode & 0o100 != 0 { 'x' } else { '-' });

        // Group
        s.push(if self.mode & 0o040 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o020 != 0 { 'w' } else { '-' });
        s.push(if self.mode & 0o010 != 0 { 'x' } else { '-' });

        // Other
        s.push(if self.mode & 0o004 != 0 { 'r' } else { '-' });
        s.push(if self.mode & 0o002 != 0 { 'w' } else { '-' });
        s.push(if self.mode & 0o001 != 0 { 'x' } else { '-' });

        s
    }

    /// 转换为八进制字符串 (如 "755")
    pub fn to_string_octal(&self) -> String {
        format!("{:03o}", self.mode & 0o777)
    }
}

impl Default for FilePermissions {
    fn default() -> Self {
        Self { mode: 0o644 }
    }
}

impl std::fmt::Display for FilePermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_unix())
    }
}

/// 目录条目
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// 文件名
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 文件类型
    pub entry_type: EntryType,
    /// 文件大小（字节）
    pub size: u64,
    /// 修改时间
    pub modified: Option<SystemTime>,
    /// 访问时间
    pub accessed: Option<SystemTime>,
    /// 权限
    pub permissions: Option<FilePermissions>,
    /// 所有者 UID
    pub uid: Option<u32>,
    /// 所有者 GID
    pub gid: Option<u32>,
}

impl DirEntry {
    /// 创建新的目录条目
    pub fn new(name: impl Into<String>, path: impl Into<String>, entry_type: EntryType) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            entry_type,
            size: 0,
            modified: None,
            accessed: None,
            permissions: None,
            uid: None,
            gid: None,
        }
    }

    /// 设置大小
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    /// 设置修改时间
    pub fn with_modified(mut self, time: SystemTime) -> Self {
        self.modified = Some(time);
        self
    }

    /// 设置权限
    pub fn with_permissions(mut self, perms: FilePermissions) -> Self {
        self.permissions = Some(perms);
        self
    }

    /// 从 FileAttributes 创建
    pub fn from_attrs(
        name: String,
        parent_path: &str,
        attrs: &FileAttributes,
        file_type: Option<FileType>,
    ) -> Self {
        let path = if parent_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };

        let entry_type = file_type.map(EntryType::from).unwrap_or(EntryType::Other);

        let modified = attrs
            .mtime
            .map(|t| UNIX_EPOCH + Duration::from_secs(t as u64));
        let accessed = attrs
            .atime
            .map(|t| UNIX_EPOCH + Duration::from_secs(t as u64));
        let permissions = attrs.permissions.map(|p| FilePermissions::new(p));

        Self {
            name,
            path,
            entry_type,
            size: attrs.size.unwrap_or(0),
            modified,
            accessed,
            permissions,
            uid: attrs.uid,
            gid: attrs.gid,
        }
    }

    /// 是否为目录
    pub fn is_dir(&self) -> bool {
        self.entry_type.is_dir()
    }

    /// 是否为文件
    pub fn is_file(&self) -> bool {
        self.entry_type.is_file()
    }

    /// 是否为隐藏文件
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    /// 获取文件扩展名
    pub fn extension(&self) -> Option<&str> {
        if self.is_file() {
            Path::new(&self.name).extension().and_then(|s| s.to_str())
        } else {
            None
        }
    }

    /// 格式化文件大小
    pub fn formatted_size(&self) -> String {
        format_file_size(self.size)
    }

    /// 格式化修改时间
    pub fn formatted_modified(&self) -> String {
        self.modified
            .map(|t| format_time(t))
            .unwrap_or_else(|| "-".to_string())
    }
}

// ============================================================================
// 传输进度
// ============================================================================

/// 传输方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    /// 上传（本地 -> 远程）
    Upload,
    /// 下载（远程 -> 本地）
    Download,
}

/// 传输状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferState {
    /// 等待中
    Pending,
    /// 进行中
    InProgress,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 传输进度信息
#[derive(Debug, Clone)]
pub struct TransferProgress {
    /// 传输方向
    pub direction: TransferDirection,
    /// 源路径
    pub source_path: String,
    /// 目标路径
    pub dest_path: String,
    /// 总大小（字节）
    pub total_bytes: u64,
    /// 已传输大小（字节）
    pub transferred_bytes: u64,
    /// 传输状态
    pub state: TransferState,
    /// 开始时间
    pub started_at: Option<SystemTime>,
    /// 当前传输速度（字节/秒）
    pub speed_bps: u64,
    /// 错误消息
    pub error: Option<String>,
}

impl TransferProgress {
    /// 创建新的传输进度
    pub fn new(
        direction: TransferDirection,
        source: impl Into<String>,
        dest: impl Into<String>,
        total: u64,
    ) -> Self {
        Self {
            direction: direction,
            source_path: source.into(),
            dest_path: dest.into(),
            total_bytes: total,
            transferred_bytes: 0,
            state: TransferState::Pending,
            started_at: None,
            speed_bps: 0,
            error: None,
        }
    }

    /// 计算完成百分比
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.transferred_bytes as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// 估算剩余时间（秒）
    pub fn estimated_remaining_secs(&self) -> Option<u64> {
        if self.speed_bps == 0 || self.transferred_bytes >= self.total_bytes {
            return None;
        }
        let remaining = self.total_bytes - self.transferred_bytes;
        Some(remaining / self.speed_bps)
    }

    /// 格式化进度显示
    pub fn formatted_progress(&self) -> String {
        format!(
            "{} / {} ({:.1}%)",
            format_file_size(self.transferred_bytes),
            format_file_size(self.total_bytes),
            self.percentage()
        )
    }

    /// 格式化速度显示
    pub fn formatted_speed(&self) -> String {
        format!("{}/s", format_file_size(self.speed_bps))
    }

    /// 格式化剩余时间
    pub fn formatted_eta(&self) -> String {
        self.estimated_remaining_secs()
            .map(format_duration)
            .unwrap_or_else(|| "-".to_string())
    }

    /// 是否已完成
    pub fn is_completed(&self) -> bool {
        matches!(self.state, TransferState::Completed)
    }

    /// 是否失败或取消
    pub fn is_terminated(&self) -> bool {
        matches!(
            self.state,
            TransferState::Failed | TransferState::Cancelled | TransferState::Completed
        )
    }
}

/// 传输进度回调
pub type ProgressCallback = Arc<dyn Fn(&TransferProgress) + Send + Sync>;

// ============================================================================
// 传输任务
// ============================================================================

/// 传输任务 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferTaskId(pub u64);

impl std::fmt::Display for TransferTaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "transfer-{}", self.0)
    }
}

/// 传输任务
#[derive(Debug)]
pub struct TransferTask {
    /// 任务 ID
    pub id: TransferTaskId,
    /// 传输进度
    pub progress: TransferProgress,
    /// 取消标志
    cancelled: Arc<AtomicBool>,
    /// 已传输字节数（原子计数器，用于进度更新）
    transferred: Arc<AtomicU64>,
}

impl TransferTask {
    /// 创建新的传输任务
    pub fn new(
        id: TransferTaskId,
        direction: TransferDirection,
        source: impl Into<String>,
        dest: impl Into<String>,
        total: u64,
    ) -> Self {
        Self {
            id,
            progress: TransferProgress::new(direction, source, dest, total),
            cancelled: Arc::new(AtomicBool::new(false)),
            transferred: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 取消任务
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// 是否已取消
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// 获取取消标志的克隆
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    /// 获取传输计数器的克隆
    pub fn transferred_counter(&self) -> Arc<AtomicU64> {
        self.transferred.clone()
    }

    /// 更新已传输字节数
    pub fn update_transferred(&mut self, bytes: u64) {
        self.transferred.store(bytes, Ordering::SeqCst);
        self.progress.transferred_bytes = bytes;
    }
}

// ============================================================================
// SFTP 客户端
// ============================================================================

/// SFTP 客户端配置
#[derive(Debug, Clone)]
pub struct SftpConfig {
    /// 传输缓冲区大小（默认 64KB）
    pub buffer_size: usize,
    /// 最大并发传输数
    pub max_concurrent_transfers: usize,
    /// 传输超时（秒）
    pub transfer_timeout_secs: u64,
}

impl Default for SftpConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024, // 64KB
            max_concurrent_transfers: 4,
            transfer_timeout_secs: 3600, // 1 小时
        }
    }
}

/// SFTP 客户端
///
/// 提供 SFTP 文件操作功能。
pub struct SftpClient {
    /// SFTP 会话
    session: Arc<Mutex<SftpSession>>,
    /// 配置
    config: SftpConfig,
    /// 当前工作目录
    cwd: Arc<Mutex<String>>,
    /// 下一个任务 ID
    next_task_id: Arc<AtomicU64>,
}

impl SftpClient {
    /// 从 SSH 会话创建 SFTP 客户端
    ///
    /// # Arguments
    ///
    /// * `ssh_handle` - SSH 会话句柄
    ///
    /// # Returns
    ///
    /// 创建的 SFTP 客户端
    pub async fn from_ssh_session(ssh_handle: &Handle<SshHandler>) -> SftpResult<Self> {
        Self::from_ssh_session_with_config(ssh_handle, SftpConfig::default()).await
    }

    /// 从 SSH 会话创建 SFTP 客户端（带配置）
    pub async fn from_ssh_session_with_config(
        ssh_handle: &Handle<SshHandler>,
        config: SftpConfig,
    ) -> SftpResult<Self> {
        info!("Creating SFTP session...");

        // 打开 SFTP 子系统通道
        let channel = ssh_handle
            .channel_open_session()
            .await
            .map_err(|e| SftpError::Connection(format!("Failed to open channel: {}", e)))?;

        // 请求 SFTP 子系统
        channel.request_subsystem(true, "sftp").await.map_err(|e| {
            SftpError::Connection(format!("Failed to request SFTP subsystem: {}", e))
        })?;

        // 创建 SFTP 会话
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| SftpError::Connection(format!("Failed to create SFTP session: {}", e)))?;

        info!("SFTP session established");

        Ok(Self {
            session: Arc::new(Mutex::new(sftp)),
            config,
            cwd: Arc::new(Mutex::new("/".to_string())),
            next_task_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// 获取配置
    pub fn config(&self) -> &SftpConfig {
        &self.config
    }

    /// 获取当前工作目录
    pub async fn cwd(&self) -> String {
        self.cwd.lock().await.clone()
    }

    /// 设置当前工作目录
    pub async fn set_cwd(&self, path: impl Into<String>) -> SftpResult<()> {
        let path = path.into();
        // 验证目录存在
        self.stat(&path)
            .await
            .map_err(|_| SftpError::DirectoryNotFound(path.clone()))?;
        *self.cwd.lock().await = path;
        Ok(())
    }

    /// 生成新的任务 ID
    pub fn next_task_id(&self) -> TransferTaskId {
        TransferTaskId(self.next_task_id.fetch_add(1, Ordering::SeqCst))
    }

    /// 规范化路径
    fn normalize_path(&self, path: &str, cwd: &str) -> String {
        if path.starts_with('/') {
            path.to_string()
        } else if path == "." {
            cwd.to_string()
        } else if path == ".." {
            Path::new(cwd)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "/".to_string())
        } else {
            if cwd == "/" {
                format!("/{}", path)
            } else {
                format!("{}/{}", cwd, path)
            }
        }
    }

    // ========================================================================
    // 目录操作
    // ========================================================================

    /// 列出目录内容
    ///
    /// # Arguments
    ///
    /// * `path` - 目录路径（支持相对路径）
    ///
    /// # Returns
    ///
    /// 目录条目列表
    pub async fn list_dir(&self, path: &str) -> SftpResult<Vec<DirEntry>> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        debug!("Listing directory: {}", full_path);

        let session = self.session.lock().await;
        let mut entries = Vec::new();

        let mut read_dir = session.read_dir(&full_path).await?;

        while let Some(entry) = read_dir.next() {
            let name = entry.file_name();

            // 跳过 . 和 ..
            if name == "." || name == ".." {
                continue;
            }

            let file_type = Some(entry.file_type());
            let dir_entry = DirEntry::from_attrs(name, &full_path, &entry.metadata(), file_type);
            entries.push(dir_entry);
        }

        // 排序：目录在前，然后按名称排序
        entries.sort_by(|a, b| match (a.is_dir(), b.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        debug!("Found {} entries in {}", entries.len(), full_path);
        Ok(entries)
    }

    /// 获取文件/目录属性
    pub async fn stat(&self, path: &str) -> SftpResult<DirEntry> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let session = self.session.lock().await;
        let metadata = session.metadata(&full_path).await?;

        let name = Path::new(&full_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());

        let parent = Path::new(&full_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());

        // Determine file type from metadata permissions (if available)
        let file_type = metadata.permissions.map(|p| {
            let mode = p & 0o170000;
            if mode == 0o040000 {
                FileType::Dir
            } else if mode == 0o120000 {
                FileType::Symlink
            } else if mode == 0o100000 {
                FileType::File
            } else {
                FileType::Other
            }
        });

        Ok(DirEntry::from_attrs(name, &parent, &metadata, file_type))
    }

    /// 创建目录
    pub async fn mkdir(&self, path: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        debug!("Creating directory: {}", full_path);

        let session = self.session.lock().await;
        session.create_dir(&full_path).await?;

        info!("Directory created: {}", full_path);
        Ok(())
    }

    /// 递归创建目录
    pub async fn mkdir_all(&self, path: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let parts: Vec<&str> = full_path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = String::new();

        for part in parts {
            current = format!("{}/{}", current, part);
            // 尝试创建，忽略已存在的错误
            match self.mkdir(&current).await {
                Ok(_) => {},
                Err(SftpError::FileExists(_)) => {},
                Err(SftpError::Protocol(msg)) if msg.contains("exists") => {},
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// 删除空目录
    pub async fn rmdir(&self, path: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        debug!("Removing directory: {}", full_path);

        let session = self.session.lock().await;
        session.remove_dir(&full_path).await?;

        info!("Directory removed: {}", full_path);
        Ok(())
    }

    /// 递归删除目录
    pub async fn rmdir_all(&self, path: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        // 先列出所有内容
        let entries = self.list_dir(&full_path).await?;

        // 递归删除
        for entry in entries {
            if entry.is_dir() {
                // 使用 Box::pin 处理递归异步调用
                Box::pin(self.rmdir_all(&entry.path)).await?;
            } else {
                self.remove(&entry.path).await?;
            }
        }

        // 删除空目录
        self.rmdir(&full_path).await
    }

    // ========================================================================
    // 文件操作
    // ========================================================================

    /// 删除文件
    pub async fn remove(&self, path: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        debug!("Removing file: {}", full_path);

        let session = self.session.lock().await;
        session.remove_file(&full_path).await?;

        info!("File removed: {}", full_path);
        Ok(())
    }

    /// 重命名文件或目录
    pub async fn rename(&self, from: &str, to: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let from_path = self.normalize_path(from, &cwd);
        let to_path = self.normalize_path(to, &cwd);

        debug!("Renaming: {} -> {}", from_path, to_path);

        let session = self.session.lock().await;
        session.rename(&from_path, &to_path).await?;

        info!("Renamed: {} -> {}", from_path, to_path);
        Ok(())
    }

    /// 创建符号链接
    pub async fn symlink(&self, target: &str, link: &str) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let link_path = self.normalize_path(link, &cwd);

        debug!("Creating symlink: {} -> {}", link_path, target);

        let session = self.session.lock().await;
        session.symlink(target, &link_path).await?;

        info!("Symlink created: {} -> {}", link_path, target);
        Ok(())
    }

    /// 读取符号链接目标
    pub async fn read_link(&self, path: &str) -> SftpResult<String> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let session = self.session.lock().await;
        let target = session.read_link(&full_path).await?;

        Ok(target)
    }

    /// 获取真实路径（解析符号链接）
    pub async fn realpath(&self, path: &str) -> SftpResult<String> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let session = self.session.lock().await;
        let real = session.canonicalize(&full_path).await?;

        Ok(real)
    }

    /// 设置文件权限
    pub async fn chmod(&self, path: &str, mode: u32) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        debug!("Setting permissions: {} -> {:o}", full_path, mode);

        let session = self.session.lock().await;
        let mut attrs = FileAttributes::default();
        attrs.permissions = Some(mode);
        session.set_metadata(&full_path, attrs).await?;

        info!("Permissions set: {} -> {:o}", full_path, mode);
        Ok(())
    }

    // ========================================================================
    // 文件传输
    // ========================================================================

    /// 下载文件
    ///
    /// # Arguments
    ///
    /// * `remote_path` - 远程文件路径
    /// * `local_path` - 本地文件路径
    /// * `progress_callback` - 可选的进度回调
    ///
    /// # Returns
    ///
    /// 传输任务信息
    pub async fn download_file(
        &self,
        remote_path: &str,
        local_path: impl AsRef<Path>,
        progress_callback: Option<ProgressCallback>,
    ) -> SftpResult<TransferProgress> {
        let cwd = self.cwd().await;
        let remote_full = self.normalize_path(remote_path, &cwd);
        let local_path = local_path.as_ref();

        info!("Downloading: {} -> {}", remote_full, local_path.display());

        // 获取远程文件大小
        let stat = self.stat(&remote_full).await?;
        if stat.is_dir() {
            return Err(SftpError::InvalidPath(format!(
                "{} is a directory, not a file",
                remote_full
            )));
        }
        let total_size = stat.size;

        // 创建进度信息
        let mut progress = TransferProgress::new(
            TransferDirection::Download,
            remote_full.clone(),
            local_path.to_string_lossy().to_string(),
            total_size,
        );
        progress.state = TransferState::InProgress;
        progress.started_at = Some(SystemTime::now());

        // 打开远程文件
        let session = self.session.lock().await;
        let mut remote_file = session
            .open_with_flags(&remote_full, OpenFlags::READ)
            .await?;

        // 创建本地文件
        let mut local_file = TokioFile::create(local_path).await?;

        // 传输数据
        let buffer_size = self.config.buffer_size;
        let mut buffer = vec![0u8; buffer_size];
        let mut transferred: u64 = 0;
        let start_time = std::time::Instant::now();

        loop {
            let n = remote_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            local_file.write_all(&buffer[..n]).await?;
            transferred += n as u64;

            // 更新进度
            let elapsed = start_time.elapsed().as_secs_f64();
            progress.transferred_bytes = transferred;
            progress.speed_bps = if elapsed > 0.0 {
                (transferred as f64 / elapsed) as u64
            } else {
                0
            };

            // 调用进度回调
            if let Some(ref callback) = progress_callback {
                callback(&progress);
            }
        }

        // 确保数据写入磁盘
        local_file.flush().await?;

        progress.state = TransferState::Completed;
        info!(
            "Download completed: {} ({} bytes)",
            local_path.display(),
            transferred
        );

        // 最终回调
        if let Some(ref callback) = progress_callback {
            callback(&progress);
        }

        Ok(progress)
    }

    /// 上传文件
    ///
    /// # Arguments
    ///
    /// * `local_path` - 本地文件路径
    /// * `remote_path` - 远程文件路径
    /// * `progress_callback` - 可选的进度回调
    ///
    /// # Returns
    ///
    /// 传输任务信息
    pub async fn upload_file(
        &self,
        local_path: impl AsRef<Path>,
        remote_path: &str,
        progress_callback: Option<ProgressCallback>,
    ) -> SftpResult<TransferProgress> {
        let cwd = self.cwd().await;
        let remote_full = self.normalize_path(remote_path, &cwd);
        let local_path = local_path.as_ref();

        info!("Uploading: {} -> {}", local_path.display(), remote_full);

        // 获取本地文件大小
        let metadata = tokio::fs::metadata(local_path).await?;
        if metadata.is_dir() {
            return Err(SftpError::InvalidPath(format!(
                "{} is a directory, not a file",
                local_path.display()
            )));
        }
        let total_size = metadata.len();

        // 创建进度信息
        let mut progress = TransferProgress::new(
            TransferDirection::Upload,
            local_path.to_string_lossy().to_string(),
            remote_full.clone(),
            total_size,
        );
        progress.state = TransferState::InProgress;
        progress.started_at = Some(SystemTime::now());

        // 打开本地文件
        let mut local_file = TokioFile::open(local_path).await?;

        // 创建远程文件
        let session = self.session.lock().await;
        let mut remote_file = session
            .open_with_flags(
                &remote_full,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            )
            .await?;

        // 传输数据
        let buffer_size = self.config.buffer_size;
        let mut buffer = vec![0u8; buffer_size];
        let mut transferred: u64 = 0;
        let start_time = std::time::Instant::now();

        loop {
            let n = local_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            remote_file.write_all(&buffer[..n]).await?;
            transferred += n as u64;

            // 更新进度
            let elapsed = start_time.elapsed().as_secs_f64();
            progress.transferred_bytes = transferred;
            progress.speed_bps = if elapsed > 0.0 {
                (transferred as f64 / elapsed) as u64
            } else {
                0
            };

            // 调用进度回调
            if let Some(ref callback) = progress_callback {
                callback(&progress);
            }
        }

        // 确保数据写入
        remote_file.flush().await?;

        progress.state = TransferState::Completed;
        info!("Upload completed: {} ({} bytes)", remote_full, transferred);

        // 最终回调
        if let Some(ref callback) = progress_callback {
            callback(&progress);
        }

        Ok(progress)
    }

    /// 下载文件（支持取消）
    pub async fn download_file_cancellable(
        &self,
        remote_path: &str,
        local_path: impl AsRef<Path>,
        cancel_flag: Arc<AtomicBool>,
        progress_callback: Option<ProgressCallback>,
    ) -> SftpResult<TransferProgress> {
        let cwd = self.cwd().await;
        let remote_full = self.normalize_path(remote_path, &cwd);
        let local_path = local_path.as_ref();

        // 获取远程文件大小
        let stat = self.stat(&remote_full).await?;
        if stat.is_dir() {
            return Err(SftpError::InvalidPath(format!(
                "{} is a directory",
                remote_full
            )));
        }
        let total_size = stat.size;

        let mut progress = TransferProgress::new(
            TransferDirection::Download,
            remote_full.clone(),
            local_path.to_string_lossy().to_string(),
            total_size,
        );
        progress.state = TransferState::InProgress;
        progress.started_at = Some(SystemTime::now());

        let session = self.session.lock().await;
        let mut remote_file = session
            .open_with_flags(&remote_full, OpenFlags::READ)
            .await?;

        let mut local_file = TokioFile::create(local_path).await?;

        let buffer_size = self.config.buffer_size;
        let mut buffer = vec![0u8; buffer_size];
        let mut transferred: u64 = 0;
        let start_time = std::time::Instant::now();

        loop {
            // 检查取消标志
            if cancel_flag.load(Ordering::SeqCst) {
                progress.state = TransferState::Cancelled;
                warn!("Download cancelled: {}", remote_full);
                // 删除不完整的本地文件
                drop(local_file);
                let _ = tokio::fs::remove_file(local_path).await;
                return Err(SftpError::Cancelled);
            }

            let n = remote_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            local_file.write_all(&buffer[..n]).await?;
            transferred += n as u64;

            let elapsed = start_time.elapsed().as_secs_f64();
            progress.transferred_bytes = transferred;
            progress.speed_bps = if elapsed > 0.0 {
                (transferred as f64 / elapsed) as u64
            } else {
                0
            };

            if let Some(ref callback) = progress_callback {
                callback(&progress);
            }
        }

        local_file.flush().await?;
        progress.state = TransferState::Completed;

        if let Some(ref callback) = progress_callback {
            callback(&progress);
        }

        Ok(progress)
    }

    /// 上传文件（支持取消）
    pub async fn upload_file_cancellable(
        &self,
        local_path: impl AsRef<Path>,
        remote_path: &str,
        cancel_flag: Arc<AtomicBool>,
        progress_callback: Option<ProgressCallback>,
    ) -> SftpResult<TransferProgress> {
        let cwd = self.cwd().await;
        let remote_full = self.normalize_path(remote_path, &cwd);
        let local_path = local_path.as_ref();

        let metadata = tokio::fs::metadata(local_path).await?;
        if metadata.is_dir() {
            return Err(SftpError::InvalidPath(format!(
                "{} is a directory",
                local_path.display()
            )));
        }
        let total_size = metadata.len();

        let mut progress = TransferProgress::new(
            TransferDirection::Upload,
            local_path.to_string_lossy().to_string(),
            remote_full.clone(),
            total_size,
        );
        progress.state = TransferState::InProgress;
        progress.started_at = Some(SystemTime::now());

        let mut local_file = TokioFile::open(local_path).await?;

        let session = self.session.lock().await;
        let mut remote_file = session
            .open_with_flags(
                &remote_full,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            )
            .await?;

        let buffer_size = self.config.buffer_size;
        let mut buffer = vec![0u8; buffer_size];
        let mut transferred: u64 = 0;
        let start_time = std::time::Instant::now();

        loop {
            // 检查取消标志
            if cancel_flag.load(Ordering::SeqCst) {
                progress.state = TransferState::Cancelled;
                warn!("Upload cancelled: {}", local_path.display());
                // 删除不完整的远程文件
                drop(remote_file);
                let _ = session.remove_file(&remote_full).await;
                return Err(SftpError::Cancelled);
            }

            let n = local_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            remote_file.write_all(&buffer[..n]).await?;
            transferred += n as u64;

            let elapsed = start_time.elapsed().as_secs_f64();
            progress.transferred_bytes = transferred;
            progress.speed_bps = if elapsed > 0.0 {
                (transferred as f64 / elapsed) as u64
            } else {
                0
            };

            if let Some(ref callback) = progress_callback {
                callback(&progress);
            }
        }

        remote_file.flush().await?;
        progress.state = TransferState::Completed;

        if let Some(ref callback) = progress_callback {
            callback(&progress);
        }

        Ok(progress)
    }

    /// 读取远程文件内容（小文件）
    pub async fn read_file(&self, path: &str) -> SftpResult<Vec<u8>> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let session = self.session.lock().await;
        let mut file = session.open_with_flags(&full_path, OpenFlags::READ).await?;

        let mut content = Vec::new();
        let mut buffer = vec![0u8; self.config.buffer_size];

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            content.extend_from_slice(&buffer[..n]);
        }

        Ok(content)
    }

    /// 写入远程文件内容（小文件）
    pub async fn write_file(&self, path: &str, content: &[u8]) -> SftpResult<()> {
        let cwd = self.cwd().await;
        let full_path = self.normalize_path(path, &cwd);

        let session = self.session.lock().await;
        let mut file = session
            .open_with_flags(
                &full_path,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            )
            .await?;

        file.write_all(content).await?;
        file.flush().await?;

        Ok(())
    }
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 格式化文件大小
pub fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// 格式化时间
pub fn format_time(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let secs = duration.as_secs();

    // 简单的时间格式化（实际应用中应使用 chrono）
    let days = secs / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;

    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        year, month, day, hour, minute
    )
}

/// 格式化持续时间
pub fn format_duration(secs: u64) -> String {
    if secs >= 3600 {
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    } else if secs >= 60 {
        let minutes = secs / 60;
        let seconds = secs % 60;
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", secs)
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_type_icon() {
        assert_eq!(EntryType::File.icon(), "📄");
        assert_eq!(EntryType::Directory.icon(), "📁");
        assert_eq!(EntryType::Symlink.icon(), "🔗");
    }

    #[test]
    fn test_entry_type_checks() {
        assert!(EntryType::Directory.is_dir());
        assert!(!EntryType::File.is_dir());
        assert!(EntryType::File.is_file());
        assert!(!EntryType::Directory.is_file());
    }

    #[test]
    fn test_file_permissions_unix_string() {
        let perms = FilePermissions::new(0o755);
        assert_eq!(perms.to_string_unix(), "rwxr-xr-x");

        let perms2 = FilePermissions::new(0o644);
        assert_eq!(perms2.to_string_unix(), "rw-r--r--");

        let perms3 = FilePermissions::new(0o000);
        assert_eq!(perms3.to_string_unix(), "---------");
    }

    #[test]
    fn test_file_permissions_octal_string() {
        let perms = FilePermissions::new(0o755);
        assert_eq!(perms.to_string_octal(), "755");

        let perms2 = FilePermissions::new(0o644);
        assert_eq!(perms2.to_string_octal(), "644");
    }

    #[test]
    fn test_file_permissions_checks() {
        let perms = FilePermissions::new(0o755);
        assert!(perms.owner_read());
        assert!(perms.owner_write());
        assert!(perms.owner_execute());

        let perms2 = FilePermissions::new(0o444);
        assert!(perms2.owner_read());
        assert!(!perms2.owner_write());
        assert!(!perms2.owner_execute());
    }

    #[test]
    fn test_dir_entry_basic() {
        let entry =
            DirEntry::new("test.txt", "/home/user/test.txt", EntryType::File).with_size(1024);

        assert_eq!(entry.name, "test.txt");
        assert_eq!(entry.path, "/home/user/test.txt");
        assert!(entry.is_file());
        assert!(!entry.is_dir());
        assert_eq!(entry.size, 1024);
    }

    #[test]
    fn test_dir_entry_hidden() {
        let hidden = DirEntry::new(".hidden", "/home/.hidden", EntryType::File);
        assert!(hidden.is_hidden());

        let visible = DirEntry::new("visible", "/home/visible", EntryType::File);
        assert!(!visible.is_hidden());
    }

    #[test]
    fn test_dir_entry_extension() {
        let txt = DirEntry::new("file.txt", "/file.txt", EntryType::File);
        assert_eq!(txt.extension(), Some("txt"));

        let no_ext = DirEntry::new("file", "/file", EntryType::File);
        assert_eq!(no_ext.extension(), None);

        let dir = DirEntry::new("dir.d", "/dir.d", EntryType::Directory);
        assert_eq!(dir.extension(), None);
    }

    #[test]
    fn test_transfer_progress_percentage() {
        let progress = TransferProgress::new(
            TransferDirection::Download,
            "/remote/file",
            "/local/file",
            1000,
        );
        assert_eq!(progress.percentage(), 0.0);

        let mut progress2 = progress.clone();
        progress2.transferred_bytes = 500;
        assert_eq!(progress2.percentage(), 50.0);

        let mut progress3 = progress.clone();
        progress3.transferred_bytes = 1000;
        assert_eq!(progress3.percentage(), 100.0);
    }

    #[test]
    fn test_transfer_progress_eta() {
        let mut progress =
            TransferProgress::new(TransferDirection::Download, "/remote", "/local", 1000);
        progress.transferred_bytes = 500;
        progress.speed_bps = 100;

        // 剩余 500 字节，速度 100 B/s，预计 5 秒
        assert_eq!(progress.estimated_remaining_secs(), Some(5));
    }

    #[test]
    fn test_transfer_task_cancel() {
        let task = TransferTask::new(
            TransferTaskId(1),
            TransferDirection::Upload,
            "/local",
            "/remote",
            1000,
        );

        assert!(!task.is_cancelled());
        task.cancel();
        assert!(task.is_cancelled());
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.00 KB");
        assert_eq!(format_file_size(1536), "1.50 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_file_size(1024 * 1024 * 1024 * 1024), "1.00 TB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3700), "1h 1m");
    }

    #[test]
    fn test_sftp_config_default() {
        let config = SftpConfig::default();
        assert_eq!(config.buffer_size, 64 * 1024);
        assert_eq!(config.max_concurrent_transfers, 4);
        assert_eq!(config.transfer_timeout_secs, 3600);
    }

    #[test]
    fn test_sftp_error_conversion() {
        let err = SftpError::FileNotFound("/test".to_string());
        assert!(matches!(err, SftpError::FileNotFound(_)));

        let err2 = SftpError::PermissionDenied("/root".to_string());
        assert!(matches!(err2, SftpError::PermissionDenied(_)));
    }

    #[test]
    fn test_transfer_task_id_display() {
        let id = TransferTaskId(42);
        assert_eq!(id.to_string(), "transfer-42");
    }

    #[test]
    fn test_transfer_progress_formatted() {
        let mut progress = TransferProgress::new(
            TransferDirection::Download,
            "/remote/file.zip",
            "/local/file.zip",
            1024 * 1024, // 1 MB
        );
        progress.transferred_bytes = 512 * 1024; // 512 KB
        progress.speed_bps = 1024 * 100; // 100 KB/s

        assert!(progress.formatted_progress().contains("512.00 KB"));
        assert!(progress.formatted_progress().contains("1.00 MB"));
        assert!(progress.formatted_progress().contains("50.0%"));

        assert_eq!(progress.formatted_speed(), "100.00 KB/s");
    }
}
