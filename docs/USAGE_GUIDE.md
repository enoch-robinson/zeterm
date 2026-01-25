# Zeterm 使用指南

**版本**: v0.1.0  
**更新日期**: 2026-01-25  
**适用范围**: HostListView 集成修复后

---

## 一、快速开始

### 1.1 启动 Zeterm

```bash
# 开发模式
cargo run

# Release 模式
cargo build --release
./target/release/zeterm
```

### 1.2 首次启动

首次启动时，Zeterm 会：
1. 自动创建配置目录
2. 初始化数据库
3. 显示欢迎界面

**配置目录位置**:
- **Linux**: `~/.config/zeterm/`
- **macOS**: `~/Library/Application Support/zeterm/`
- **Windows**: `%APPDATA%\zeterm\`

---

## 二、主机管理

### 2.1 界面布局

```
┌─────────────────────────────────────────────────┐
│ Zeterm                                    × □ ─  │
├──────────────┬──────────────────────────────────┤
│ 主机列表 [新建] [刷新]                           │
│              │                                  │
│ ▼ 默认分组   │       欢迎界面                    │
│   □ 服务器1  │    或 Tab 内容区域                │
│   □ 服务器2  │                                  │
│              │                                  │
├──────────────┴──────────────────────────────────┤
│ ○ Disconnected  │  UTF-8  │  80×24              │
└─────────────────────────────────────────────────┘

左侧: 主机列表侧边栏 (280px 宽)
右侧: 主内容区域
底部: 状态栏
```

### 2.2 添加新主机

#### 方法 1: 点击"新建"按钮

1. 点击主机列表顶部的 **[新建]** 按钮
2. 在弹出的对话框中填写信息：
   - **名称**: 主机显示名称（如 "生产服务器"）
   - **主机地址**: IP 或域名（如 `192.168.1.100`）
   - **端口**: SSH 端口（默认 22）
   - **用户名**: SSH 用户名（如 `root`）
   - **认证方式**: 密码/公钥/Agent
   - **分组**: 可选分组名称（如 "生产环境"）
3. 点击 **[确定]** 保存

#### 方法 2: 通过配置文件

编辑 `~/.config/zeterm/hosts.toml`:

```toml
[[hosts]]
name = "测试服务器"
host = "192.168.1.100"
port = 22
username = "root"
group = "开发环境"

[hosts.auth]
type = "password"
password = "keychain:test_server"  # 从密钥链读取

[[hosts]]
name = "生产服务器"
host = "example.com"
port = 22
username = "admin"
group = "生产环境"

[hosts.auth]
type = "publickey"
key_path = "~/.ssh/id_rsa"
```

保存后点击 **[刷新]** 按钮重新加载。

### 2.3 编辑主机

1. 在主机列表中 **右键点击** 主机项
2. 选择 **"编辑"**
3. 在对话框中修改信息
4. 点击 **[确定]** 保存

### 2.4 删除主机

1. 在主机列表中 **右键点击** 主机项
2. 选择 **"删除"**
3. 在确认对话框中点击 **[确定]**

⚠️ **注意**: 删除操作不可恢复

### 2.5 分组管理

主机会按分组自动组织：
- 点击分组名称前的 **▶** 展开/折叠
- 未指定分组的主机归入 **"默认分组"**
- 分组会自动排序

---

## 三、连接主机

### 3.1 快速连接

**方法 1**: 双击主机项  
**方法 2**: 右键菜单选择 "连接"

### 3.2 连接流程

```
双击主机
    ↓
创建新 Tab
    ↓
建立 SSH 连接
    ↓
显示终端界面
```

### 3.3 连接状态

状态栏左侧显示连接状态：
- **○ Disconnected**: 未连接（灰色）
- **◐ Connecting...**: 连接中（黄色）
- **● Connected**: 已连接（绿色）
- **✕ Error**: 连接错误（红色）

### 3.4 认证方式

#### 密码认证
```toml
[hosts.auth]
type = "password"
password = "plain:mypassword"      # 明文（不推荐）
# 或
password = "keychain:server_name"  # 从系统密钥链读取（推荐）
# 或
password = "env:SSH_PASSWORD"      # 从环境变量读取
```

#### 公钥认证
```toml
[hosts.auth]
type = "publickey"
key_path = "~/.ssh/id_rsa"
passphrase = "keychain:key_pass"   # 可选：私钥密码
```

#### SSH Agent
```toml
[hosts.auth]
type = "agent"  # 使用系统 SSH Agent
```

---

## 四、Tab 管理

### 4.1 创建 Tab

- **快捷键**: `Ctrl+Shift+T`
- **按钮**: 点击 Tab 栏的 **[+]** 按钮
- **连接主机**: 双击主机自动创建 Tab

### 4.2 切换 Tab

- **快捷键**: `Ctrl+Tab` (下一个) / `Ctrl+Shift+Tab` (上一个)
- **鼠标**: 点击 Tab 标签

### 4.3 关闭 Tab

- **快捷键**: `Ctrl+Shift+W`
- **鼠标**: 点击 Tab 上的 **[×]** 按钮

⚠️ **注意**: 如果有活动连接，会弹出确认对话框

### 4.4 拖拽排序

- 按住 Tab 标签拖动到目标位置
- 松开鼠标完成排序

---

## 五、分屏操作

### 5.1 创建分屏

| 快捷键 | 方向 | 说明 |
|--------|------|------|
| `Ctrl+\` | 水平分屏 | 左右排列 |
| `Ctrl+Shift+-` | 垂直分屏 | 上下排列 |

### 5.2 切换焦点

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+]` | 下一个面板 |
| `Ctrl+[` | 上一个面板 |

### 5.3 调整大小

- 拖动分隔条调整面板比例
- 分隔条宽度 6px，鼠标悬停时高亮

### 5.4 关闭面板

- **快捷键**: `Ctrl+Alt+W`
- 关闭当前焦点面板

---

## 六、快捷键汇总

### 6.1 应用级

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Q` | 退出应用 |
| `Ctrl+B` | 切换侧边栏 |

### 6.2 Tab 管理

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+T` | 新建 Tab |
| `Ctrl+Shift+W` | 关闭 Tab |
| `Ctrl+Tab` | 下一个 Tab |
| `Ctrl+Shift+Tab` | 上一个 Tab |

### 6.3 分屏管理

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+\` | 水平分屏 |
| `Ctrl+Shift+-` | 垂直分屏 |
| `Ctrl+Alt+W` | 关闭面板 |
| `Ctrl+]` | 下一个面板 |
| `Ctrl+[` | 上一个面板 |

### 6.4 终端操作

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+C` | 复制选中内容 |
| `Ctrl+Shift+V` | 粘贴 |
| `Ctrl+Shift+F` | 搜索 |
| `Ctrl+=` | 放大字体 |
| `Ctrl+-` | 缩小字体 |
| `Ctrl+0` | 重置字体大小 |

### 6.5 主题切换

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Alt+T` | 切换深色/浅色 |
| `Ctrl+.` | 下一个主题 |
| `Ctrl+,` | 上一个主题 |

---

## 七、配置文件

### 7.1 配置文件位置

- **主配置**: `~/.config/zeterm/config.toml`
- **主机配置**: `~/.config/zeterm/hosts.toml`
- **数据库**: `~/.config/zeterm/zeterm.db`

### 7.2 主配置示例

```toml
[app]
theme = "Dark"  # Dark, Light, Dracula, OneDark, etc.
language = "zh-CN"

[terminal]
font_size = 14.0
line_height = 1.2
scrollback_lines = 10000
cursor_blink = true

[keybindings]
copy = "Ctrl+Shift+C"
paste = "Ctrl+Shift+V"
new_tab = "Ctrl+Shift+T"
close_tab = "Ctrl+Shift+W"

[network]
connect_timeout_ms = 10000
keepalive_interval_sec = 60
```

### 7.3 热更新

修改配置文件后，Zeterm 会自动重新加载（无需重启）。

---

## 八、常见问题

### Q1: 主机列表显示"主机列表"占位符？

**A**: 这是修复前的 bug，请确保：
1. 已更新到最新版本
2. 运行 `cargo build --release` 重新编译
3. 检查数据库文件是否正常

### Q2: 双击主机无响应？

**A**: 检查以下事项：
1. 主机配置是否正确（IP、端口、用户名）
2. 网络连接是否正常
3. 查看日志输出 `RUST_LOG=debug cargo run`

### Q3: 如何存储密码？

**A**: 推荐使用系统密钥链：
```toml
[hosts.auth]
type = "password"
password = "keychain:my_server"
```

首次连接时会提示输入密码并保存到密钥链。

### Q4: 连接超时怎么办？

**A**: 调整超时配置：
```toml
[network]
connect_timeout_ms = 30000  # 增加到 30 秒
```

### Q5: 如何备份主机列表？

**A**: 复制配置文件：
```bash
cp ~/.config/zeterm/hosts.toml ~/zeterm-hosts-backup.toml
cp ~/.config/zeterm/zeterm.db ~/zeterm-db-backup.db
```

### Q6: 支持跳板机吗？

**A**: 当前版本暂不支持，计划在后续版本实现。

---

## 九、调试技巧

### 9.1 启用详细日志

```bash
# 所有模块 debug 级别
RUST_LOG=debug cargo run

# 特定模块 trace 级别
RUST_LOG=zeterm_ssh=trace cargo run

# 多模块组合
RUST_LOG=zeterm_ssh=trace,zeterm_core=debug cargo run
```

### 9.2 查看数据库

```bash
sqlite3 ~/.config/zeterm/zeterm.db

# 查看主机表
SELECT * FROM hosts;

# 查看连接历史
SELECT * FROM connection_history;
```

### 9.3 重置配置

```bash
# 备份后删除配置目录
rm -rf ~/.config/zeterm
```

下次启动会重新初始化。

---

## 十、获取帮助

### 10.1 文档

- [设计文档](./README.md)
- [API 文档](./api.md)
- [修复报告](./fix-host-list-integration.md)

### 10.2 问题反馈

遇到问题请提供：
1. Zeterm 版本号
2. 操作系统和版本
3. 复现步骤
4. 日志输出

### 10.3 开发状态

当前版本功能状态：
- ✅ 主机列表管理
- ✅ Tab 管理
- ✅ 分屏布局
- ✅ 主题系统
- ⚠️ SSH 连接（部分实现）
- ⏳ SFTP 文件传输
- ⏳ 端口转发
- ⏳ 跳板机支持

---

**文档版本**: 1.0  
**最后更新**: 2026-01-25