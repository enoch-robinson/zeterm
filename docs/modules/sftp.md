# SFTP 文件管理模块

> 基于 SSH 连接的远程文件系统访问与管理

---

## 一、设计目标

| 目标 | 说明 |
|------|------|
| 无缝集成 | 复用现有 SSH 连接，无需额外认证 |
| 异步传输 | 非阻塞的文件上传下载 |
| 进度反馈 | 实时传输进度与速度显示 |
| 断点续传 | 支持大文件传输中断恢复 |

---

## 二、核心接口

### 2.1 FileSystemBackend Trait

```rust
#[async_trait]
pub trait FileSystemBackend: Send + Sync {
    async fn list_dir(&self, path: &str) -> Result<Vec<FileEntry>>;
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn create_dir(&self, path: &str) -> Result<()>;
    async fn delete(&self, path: &str, recursive: bool) -> Result<()>;
    async fn rename(&self, from: &str, to: &str) -> Result<()>;
    async fn stat(&self, path: &str) -> Result<FileInfo>;
}
```

### 2.2 文件条目

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | String | 文件名 |
| `path` | String | 完整路径 |
| `file_type` | FileType | File/Directory/Symlink |
| `size` | u64 | 文件大小 |
| `modified` | SystemTime | 修改时间 |
| `permissions` | u32 | 权限位 |

---

## 三、SftpClient 实现

### 3.1 结构

```rust
pub struct SftpClient {
    session: SftpSession,  // russh-sftp 会话
    cwd: String,           // 当前工作目录
}
```

### 3.2 创建方式

从现有 SSH 连接创建，无需重新认证：

```rust
impl SftpClient {
    pub async fn from_ssh_channel(channel: Channel) -> Result<Self>;
}
```

---

## 四、文件传输管理

### 4.1 传输任务状态

| 状态 | 说明 |
|------|------|
| `Pending` | 等待开始 |
| `InProgress` | 传输中 |
| `Paused` | 已暂停 |
| `Completed` | 已完成 |
| `Failed` | 失败 |
| `Cancelled` | 已取消 |

### 4.2 TransferManager

| 方法 | 说明 |
|------|------|
| `upload(local, remote)` | 上传文件 |
| `download(remote, local)` | 下载文件 |
| `pause(task_id)` | 暂停传输 |
| `resume(task_id)` | 恢复传输 |
| `cancel(task_id)` | 取消传输 |
| `progress_stream()` | 获取进度通知流 |

### 4.3 进度通知

```rust
pub struct TransferProgress {
    pub task_id: TransferId,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub speed_bytes_per_sec: u64,
}
```

---

## 五、UI 组件 (基于 gpui-component)

### 5.1 界面布局

```
┌─────────────────────────────────────────────────┐
│  /home/user/projects                [↑] [↓] [×] │
├─────────────────────────────────────────────────┤
│  📁 ..                                          │
│  📁 src                    4.0 KB   Jan 1       │
│  📁 docs                   2.1 KB   Jan 2       │
│  📄 Cargo.toml             1.2 KB   Jan 3       │
│  📄 README.md              3.4 KB   Jan 4       │
├─────────────────────────────────────────────────┤
│  传输队列 (2)                                   │
│  ↑ file.zip      45%  ████░░░░░░  2.3 MB/s      │
│  ↓ data.csv      完成  ██████████               │
└─────────────────────────────────────────────────┘
```

### 5.2 使用的 gpui-component 组件

| 组件 | 用途 |
|------|------|
| `Table` | 文件列表 |
| `Tree` | 目录树 |
| `Progress` | 传输进度条 |
| `ContextMenu` | 右键菜单 |
| `Modal` | 确认对话框 |

---

## 六、操作支持

| 操作 | 快捷键 | 说明 |
|------|--------|------|
| 上传 | 拖放 | 拖放本地文件到列表 |
| 下载 | Enter | 下载选中文件 |
| 删除 | Delete | 删除选中项 |
| 重命名 | F2 | 重命名文件 |
| 新建目录 | Ctrl+N | 创建新目录 |
| 刷新 | F5 | 刷新文件列表 |

---

## 七、实现优先级

| 优先级 | 功能 | Phase |
|--------|------|-------|
| P0 | 目录浏览 | 5 |
| P0 | 文件上传/下载 | 5 |
| P1 | 进度显示 | 5 |
| P1 | 拖放支持 | 5 |
| P2 | 断点续传 | 后续 |
| P2 | 批量操作 | 后续 |

---

## 八、相关文档

- [SSH 后端实现](./ssh-backend.md) - SFTP 依赖的 SSH 连接
- [API.md](../API.md) - 接口定义
- [ARCHITECTURE.md](../ARCHITECTURE.md) - 架构总览