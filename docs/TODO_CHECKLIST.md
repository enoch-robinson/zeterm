# Zeterm 修复计划 Checklist

> 创建日期: 2025-01-22
> 详细说明见: [TODO_CODEBASE_REVIEW.md](./TODO_CODEBASE_REVIEW.md)

---

## ✅ 已完成

- [x] MockConnection 取消标志 Bug (2025-01-22)
- [x] 公钥编码使用 Debug 格式 (2025-01-22)

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

- [ ] **P3** KeyboardInteractive 认证
- [ ] **P3** 系统主题检测
- [ ] **P3** 删除主机确认对话框
- [ ] **P3** 终端单词选择优化
- [ ] **P3** SFTP 右键菜单

---

## Phase 4: 代码清理

- [ ] **P4** 清理未使用的 imports
- [ ] **P4** 密码引用解析错误处理
- [ ] **P4** 终端尺寸边界验证

---

## 统计

| 阶段 | 总计 | 完成 | 进度 |
|------|------|------|------|
| 已完成 | 2 | 2 | 100% |
| Phase 1 | 4 | 4 | 100% |
| Phase 2 | 4 | 4 | 100% |
| Phase 3 | 5 | 0 | 0% |
| Phase 4 | 3 | 0 | 0% |
| **总计** | **18** | **10** | **56%** |

---

*最后更新: 2026-01-22*