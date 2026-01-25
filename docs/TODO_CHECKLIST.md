# Zeterm 修复计划 Checklist

> 创建日期: 2025-01-22
> 详细说明见: [TODO_CODEBASE_REVIEW.md](./TODO_CODEBASE_REVIEW.md)

---

## ✅ 已完成

- [x] MockConnection 取消标志 Bug (2025-01-22)
- [x] 公钥编码使用 Debug 格式 (2025-01-22)
- [x] SFTP 右键菜单功能完善 (2026-01-25)

---

## Phase 1: 紧急修复

- [x] **P0** ConfigWatcher 变量混淆 (2025-01-22)
  - 文件: `zeterm-core/src/config/watcher.rs`
  - 使用结构体字段替代局部变量

- [x] **P1** 数据库启动时初始化 (2026-01-22)
  - 文件: `zeterm/src/main.rs`
  - 添加 Database 初始化逻辑

- [x] **P1** 启动时清理僵尸连接记录 (2026-01-22)
  - 调用 `mark_all_active_as_disconnected()`

- [x] **P1** 共享 Tokio Runtime (2026-01-22)
  - 创建应用级 Runtime (`app/runtime.rs`)
  - 替换多处临时 Runtime 创建

---

## Phase 2: 核心功能

- [x] **P1** SFTP 异步加载更新 UI (2026-01-22)
  - 文件: `zeterm/src/ui/sftp/sftp_view.rs`
  - 通过共享状态传递加载结果，在 render 时处理

- [x] **P2** SFTP 传输控制 (2026-01-22)
  - [x] 暂停功能
  - [x] 恢复功能
  - [x] 取消功能
  - [x] 重试功能

- [x] **P2** 重连逻辑集成 (2026-01-22)
  - [x] 创建 ReconnectController
  - [x] 实现连接工厂模式 (ConnectionFactory trait)
  - [x] 监听 ConnectionLost 事件
  - [x] 在数据泵中检查状态变化

- [x] **P3** 状态机补充转换 (2026-01-22)
  - 添加 `(Connecting, ConnectionLost) => Disconnected` 处理

---

## Phase 3: 功能增强

- [x] **P3** KeyboardInteractive 认证 (2026-01-22)
- [x] **P3** 系统主题检测 (2026-01-22)
- [x] **P3** 删除主机确认对话框 (2026-01-22)
- [x] **P3** 终端单词选择优化 (2026-01-22)
- [x] **P3** SFTP 右键菜单 (2026-01-22)

---

## Phase 4: 代码清理

- [x] **P4** 清理未使用的 imports (2025-01-23)
- [x] **P4** 密码引用解析错误处理 (2025-01-23)
- [x] **P4** 终端尺寸边界验证 (2025-01-23)

---

## Phase 5: SFTP 右键菜单完善

- [x] **P1** 复制路径到剪贴板 (2026-01-25)
  - 使用 GPUI ClipboardItem API

- [x] **P1** 删除文件/目录 (2026-01-25)
  - 添加 DeleteConfirmState 确认对话框
  - 调用 SftpClient::remove() / rmdir_all()
  - 异步执行，完成后刷新目录

- [x] **P1** 重命名文件/目录 (2026-01-25)
  - 添加 RenameDialogState 输入对话框
  - 调用 SftpClient::rename()

- [x] **P1** 查看属性 (2026-01-25)
  - 添加 PropertiesDialogState 显示对话框
  - 调用 SftpClient::stat() 获取文件信息
  - 显示类型、路径、大小、修改时间、权限

- [x] **P2** 重试传输 (2026-01-25)
  - 实现实际传输操作调用
  - 根据 TransferDirection 调用 download/upload_file_cancellable

- [x] **P2** 右键菜单事件绑定 (2026-01-25)
  - 为所有菜单项绑定 on_mouse_down 事件

---

## 后续可选优化 (P2)

> 这些功能为可选增强，不影响核心功能使用。

- [x] **P2** Tab 拖拽排序 (2026-01-25)
  - 📄 实现文档: [impl-tab-drag-sort.md](./impl-tab-drag-sort.md)
  - 使用 GPUI 的 `on_drag` / `on_drop` / `drag_over` API
  - 在 `tab_view.rs` 中添加 `DraggedTab` 结构体和拖拽逻辑
  - 复用现有 `TabManager::move_tab` 方法

- [x] **P2** 自定义主题文件 (2026-01-25)
  - 为 `Rgb`, `ColorPalette` 添加 Serde 支持
  - 实现 `TerminalTheme::from_toml()` / `to_toml()` 方法
  - 在 `AppThemeManager` 中实现 `load_custom_theme()`
  - 添加示例主题文件 `examples/themes/`

- [ ] **P2** 鼠标报告模式
  - 利用 Alacritty 已有的 `Mode` 检测
  - 将鼠标事件编码为 SGR 序列发送到远端
  - 支持 vim/tmux/htop 等工具的鼠标操作
  - 预估工作量: 1-2 天

- [ ] **P2** 快捷键配置
  - `KeybindingsConfig` 结构已定义
  - 从 `config.toml` 加载用户快捷键
  - 替换 `main.rs` 中的硬编码快捷键
  - 预估工作量: 0.5 天

---

## 统计

| 阶段 | 总计 | 完成 | 进度 |
|------|------|------|------|
| 已完成 | 3 | 3 | 100% |
| Phase 1 | 4 | 4 | 100% |
| Phase 2 | 4 | 4 | 100% |
| Phase 3 | 5 | 5 | 100% |
| Phase 4 | 3 | 3 | 100% |
| Phase 5 | 6 | 6 | 100% |
| P2 可选 | 4 | 2 | 50% |
| **核心总计** | **25** | **25** | **100%** |

---

## 相关文档

- [Tab 拖拽排序实现指南](./impl-tab-drag-sort.md) - P2 功能详细实现方案

---

*最后更新: 2026-01-25 (自定义主题文件完成)*