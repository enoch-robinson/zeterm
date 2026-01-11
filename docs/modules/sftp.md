# SFTP 文件管理模块

> 基于 SSH 连接的远程文件系统访问与管理

---

## 一、设计目标

1. **无缝集成** - 复用现有 SSH 连接，无需额外认证
2. **异步传输** - 非阻塞的文件上传下载
3. **进度反馈** - 实时传输进度与速度显示
4. **断点续传** - 支持大文件传输中断恢复

---

## 二、核心接口

### 2.1 FileSystemBackend Trait

```rust
use async_trait::async_trait;
use anyhow::Result;

/// 文件系统后端抽象
#[async_trait]
pub trait FileSystemBackend: Send + Sync {
    /// 列出目录内容
    async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>>;
    
    /// 读取文件内容
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    
    /// 写入文件
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    
    /// 创建目录
    async fn create_dir(&self, path: &str) -> Result<()>;
    
    /// 删除文件或目录
    async fn delete(&self, path: &str, recursive: bool) -> Result<()>;
    
    /// 重命名/移动
    async fn rename(&self, from: &str, to: &str) -> Result<()>;
    
    /// 获取文件信息
    async fn stat(&self, path: &str) -> Result<FileInfo>;
}
```

### 2.2 文件条目结构

```rust
/// 文件条目
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: SystemTime,
    pub permissions: u32,
}

/// 文件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    File,
    Directory,
    Symlink,
    Other,
}
```

---

## 三、SFTP 实现

### 3.1 SftpClient 结构

```rust
use russh_sftp::client::SftpSession;

pub struct SftpClient {
    /// SFTP 会话
    session: SftpSession,
    
    /// 当前工作目录
    cwd: String,
}

impl SftpClient {
    /// 从现有 SSH 连接创建 SFTP 客户端
    pub async fn from_ssh_channel(channel: Channel) -> Result<Self> {
        let session = SftpSession::new(channel.into_stream()).await?;
        
        Ok(Self {
            session,
            cwd: "/".to_string(),
        })
    }
}
```

### 3.2Trait 实现

```rust
#[async_trait]
impl FileSystemBackend for SftpClient {
    async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>> {
        let full_path = self.resolve_path(path);
        let entries = self.session.read_dir(&full_path).await?;
        
        entries
            .into_iter()
            .map(|e| FileEntry::from_sftp_entry(e))
            .collect()
    }
    
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path);
        let mut file = self.session.open(&full_path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;
        
        Ok(buffer)
    }
    
    // ... 其他方法实现
}
```

---

## 四、文件传输管理

### 4.1 传输任务

```rust
/// 传输任务
pub struct TransferTask {
    pub id: TransferId,
    pub direction: TransferDirection,
    pub local_path: PathBuf,
    pub remote_path: String,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub status: TransferStatus,
    pub started_at: Instant,
}

/// 传输方向
pub enum TransferDirection {
    Upload,
    Download,
}

/// 传输状态
pub enum TransferStatus {
    Pending,
    InProgress,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
}
```

### 4.2 传输管理器

```rust
pub struct TransferManager {
    /// 活跃传输任务
    tasks: HashMap<TransferId, TransferTask>,
    
    /// 进度通知
    progress_tx: broadcast::Sender<TransferProgress>,
    
    /// 并发限制
    max_concurrent: usize,
}

impl TransferManager {
    /// 上传文件
    pub async fn upload(
        &mut self,
        sftp: &SftpClient,
        local: &Path,
        remote: &str,
    ) -> Result<TransferId> {
        let task = TransferTask::new_upload(local, remote);
        let id = task.id;
        
        self.tasks.insert(id, task);
        self.start_upload(id, sftp).await?;
        
        Ok(id)
    }
    
    /// 下载文件
    pub async fn download(
        &mut self,
        sftp: &SftpClient,
        remote: &str,
        local: &Path,
    ) -> Result<TransferId> {
        // 类似实现...
    }
}
```

---

## 五、UI 组件

### 5.1 SftpView 结构

```rust
pub struct SftpView {
    /// SFTP 客户端
    sftp: Arc<SftpClient>,
    
    /// 当前路径
    current_path: String,
    
    /// 文件列表
    entries: Vec<FileEntry>,
    
    /// 选中项
    selected: HashSet<usize>,
    
    /// 传输管理器
    transfer_manager: Model<TransferManager>,
}
```

### 5.2 界面布局

```
┌─────────────────────────────────────────────────┐
│  /home/user/projects[↑] │
├─────────────────────────────────────────────────┤
│  📁 ..│
│  📁 src4.0 KBJan 1 │
│  📁 docs                         2.1 KB   Jan 2 │
│  📄Cargo.toml                   1.2 KB   Jan 3 │
│  📄 README.md                    3.4 KB   Jan 4 │
├─────────────────────────────────────────────────┤
│  传输队列 (2)│
│  ↑ file.zip45%████░░░░░░  2.3 MB/s     │
│  ↓ data.csv      完成  ██████████               │
└─────────────────────────────────────────────────┘
```

---

## 六、相关文档

- [SSH 后端实现](./ssh-backend.md) - SFTP 依赖的SSH 连接
- [TerminalConnection Trait](../core/connection-trait.md) - 连接抽象
- [实现路径](../roadmap.md) - Phase 5SFTP 实现计划