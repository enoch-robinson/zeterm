# 测试策略

> Zeterm 的测试体系与 Mock 使用指南

---

## 一、测试金字塔

```╱╲
       ╱  ╲       E2E 测试 (少量)
      ╱────╲      - 完整连接流程
     ╱      ╲
    ╱────────╲    集成测试 (适量)
   ╱          ╲   - 模块间协作
  ╱────────────╲
 ╱              ╲ 单元测试 (大量)
╱────────────────╲- 核心逻辑验证
```

---

## 二、测试分层

| 层级 | 覆盖目标 | 依赖 | 执行频率 |
|------|----------|------|----------|
| 单元测试 | 纯函数、状态机、解析器 | 无外部依赖 | 每次提交 |
| 集成测试 | 模块协作、数据流| Mock后端 | 每次 PR |
| E2E 测试 | 完整用户流程 | 真实/容器化 SSH | 发布前 |

---

## 三、Mock 体系

### 3.1 MockConnection

```rust
pub struct MockConnection {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Option<mpsc::Receiver<Vec<u8>>>,
    written: Arc<Mutex<Vec<Vec<u8>>>>,
    size: (u16, u16),
}

impl MockConnection {
    /// 创建 Mock 连接
    pub fn new() -> Self;
    
    /// 模拟服务器输出
    pub async fn inject(&self, data: &[u8]);
    
    /// 获取已发送的数据
    pub fn written_data(&self) -> Vec<Vec<u8>>;
}
```

### 3.2 使用示例

```rust
#[tokio::test]
async fn test_echo_input() {
    let mock = MockConnection::new();
    let mut session = SessionCoordinator::new(Box::new(mock.clone()));
    
    // 发送输入
    session.send_input(b"ls\n").await.unwrap();
    
    // 验证数据已发送
    assert_eq!(mock.written_data(), vec![b"ls\n".to_vec()]);
}

#[tokio::test]
async fn test_receive_output() {
    let mock = MockConnection::new();
    let session = SessionCoordinator::new(Box::new(mock.clone()));
    
    // 模拟服务器响应
    mock.inject(b"\x1b[32muser@host\x1b[0m:~$ ").await;
    
    // 验证终端状态更新
    let content = session.renderable_content();
    assert!(content.contains("user@host"));
}
```

---

## 四、核心模块测试要点

### 4.1 状态机测试

```rust
#[test]
fn test_state_transitions() {
    let mut sm = ConnectionStateMachine::new();
    
    assert_eq!(sm.state(), ConnectionState::Idle);
    
    sm.handle(Event::Connect);
    assert_eq!(sm.state(), ConnectionState::Connecting { attempt: 1 });
    
    sm.handle(Event::TcpConnected);
    assert_eq!(sm.state(), ConnectionState::Authenticating);
    
    sm.handle(Event::AuthSuccess);
    assert!(matches!(sm.state(), ConnectionState::Connected { .. }));
}

#[test]
fn test_invalid_transition_ignored() {
    let mut sm = ConnectionStateMachine::new();
    sm.handle(Event::AuthSuccess); // 无效：Idle状态不能直接认证成功
    assert_eq!(sm.state(), ConnectionState::Idle);
}
```

### 4.2 ANSI 解析测试

```rust
#[test]
fn test_color_parsing() {
    let mut term = Term::new(TermConfig::default());
    term.advance_bytes(b"\x1b[31mRed\x1b[0m Normal");
    let content = term.renderable_content();
    // 验证 "Red" 为红色，"Normal" 为默认色
}

#[test]
fn test_cursor_movement() {
    let mut term = Term::new(TermConfig::default());
    term.advance_bytes(b"\x1b[5;10H*"); // 移动到(5,10) 并输出*
    
    assert_eq!(term.cursor().point, Point::new(4, 9)); // 0-indexed
}
```

### 4.3 配置解析测试

```rust
#[test]
fn test_parse_host_config() {
    let toml = r#"
        id = "server-1"
        name = "My Server"
        host = "192.168.1.1"
        username = "admin"
        [auth]
        type = "password""#;
    
    let host:HostConfig = toml::from_str(toml).unwrap();
    assert_eq!(host.id, "server-1");
    assert_eq!(host.auth, AuthConfig::Password);
}

#[test]
fn test_invalid_config_error() {
    let toml = r#"host = "missing-required-fields""#;
    let result: Result<HostConfig, _> = toml::from_str(toml);
    assert!(result.is_err());
}
```

---

## 五、测试工具

| 工具 | 用途 |
|------|------|
| `cargo test` | 运行所有测试 |
| `cargo nextest` | 并行测试执行 |
| `cargo llvm-cov` | 覆盖率报告 |
| `testcontainers` | E2E 容器化测试 |

### 5.1 常用命令

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test -p zeterm-core

# 运行匹配名称的测试
cargo test state_machine

# 生成覆盖率报告
cargo llvm-cov --html
```

---

## 六、CI集成

```yaml
# .github/workflows/test.yml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo test --all
    - run: cargo llvm-cov --lcov --output-path lcov.info
    - uses: codecov/codecov-action@v4
```

---

## 七、覆盖率目标

| 模块 | 目标 | 重点 |
|------|------|------|
| `zeterm-core` | ≥ 80% | 状态机、Trait 定义 |
| `zeterm-terminal` | ≥ 70% | ANSI 解析 |
| `zeterm-ssh` | ≥ 60% | 连接逻辑 (IO 难测) |
| `zeterm` (UI) | ≥ 40% | 事件处理 |

---

## 八、相关文档

- [MockConnection 实现](../modules/session-model.md)
- [连接状态机](./state-machine.md)
- [错误处理](./error-handling.md)