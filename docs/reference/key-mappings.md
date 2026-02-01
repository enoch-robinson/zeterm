#按键映射表

> 终端按键到ANSI 转义序列的完整映射参考

---

## 一、设计概述

### 1.1 按键处理流程

```
用户按键 → GPUI KeyDownEvent → 按键转换器 → ANSI 序列 → 发送到后端│
                                    ├── 普通模式 (Normal Mode)
                                    └── 应用模式 (Application Mode)
```

### 1.2 模式说明

| 模式 | 说明 | 触发条件 |
|------|------|----------|
| **普通模式** | 默认模式，标准 ANSI 序列 | 默认 |
| **应用模式** | 光标键发送不同序列 | 程序发送 `\x1b[?1h` 启用 |
| **数字键盘应用模式** | 数字键盘发送应用序列 | 程序发送 `\x1b[?66h` 启用 |

---

## 二、基础按键映射

### 2.1 控制字符

| 按键 | ASCII | 十六进制 | 说明 |
|------|-------|----------|------|
| Enter | CR | `0x0D` | 回车 |
| Tab | HT | `0x09` | 水平制表符 |
| Backspace | DEL | `0x7F` | 删除 |
| Escape | ESC | `0x1B` | 转义 |
| Space | SP | `0x20` | 空格 |

### 2.2 Ctrl 组合键

| 按键 | 序列 | 十六进制 | 说明 |
|------|------|----------|------|
| Ctrl+@ | NUL | `0x00` | 空字符 |
| Ctrl+A | SOH | `0x01` | 行首 |
| Ctrl+B | STX | `0x02` | 后退 |
| Ctrl+C | ETX | `0x03` | 中断 (SIGINT) |
| Ctrl+D | EOT | `0x04` | EOF |
| Ctrl+E | ENQ | `0x05` | 行尾 |
| Ctrl+F | ACK | `0x06` | 前进 |
| Ctrl+G | BEL | `0x07` | 响铃 |
| Ctrl+H | BS | `0x08` | 退格 |
| Ctrl+I | HT | `0x09` | Tab |
| Ctrl+J | LF | `0x0A` | 换行 |
| Ctrl+K | VT | `0x0B` | 删除到行尾 |
| Ctrl+L | FF | `0x0C` | 清屏 |
| Ctrl+M | CR | `0x0D` | 回车 |
| Ctrl+N | SO | `0x0E` | 下一行 |
| Ctrl+O | SI | `0x0F` | - |
| Ctrl+P | DLE | `0x10` | 上一行 |
| Ctrl+Q | DC1 | `0x11` | XON (恢复输出) |
| Ctrl+R | DC2 | `0x12` | 反向搜索 |
| Ctrl+S | DC3 | `0x13` | XOFF (暂停输出) |
| Ctrl+T | DC4 | `0x14` | 交换字符 |
| Ctrl+U | NAK | `0x15` | 删除到行首 |
| Ctrl+V | SYN | `0x16` | 字面输入 |
| Ctrl+W | ETB | `0x17` | 删除单词 |
| Ctrl+X | CAN | `0x18` | - |
| Ctrl+Y | EM | `0x19` | 粘贴 |
| Ctrl+Z | SUB | `0x1A` | 挂起 (SIGTSTP) |
| Ctrl+[ | ESC | `0x1B` | 转义 |
| Ctrl+\ | FS | `0x1C` | SIGQUIT |
| Ctrl+] | GS | `0x1D` | - |
| Ctrl+^ | RS | `0x1E` | - |
| Ctrl+_ | US | `0x1F` | - |

---

## 三、方向键和导航键

### 3.1 方向键

| 按键 | 普通模式 | 应用模式 |
|------|----------|----------|
| ↑ Up | `\x1b[A` | `\x1bOA` |
| ↓ Down | `\x1b[B` | `\x1bOB` |
| → Right | `\x1b[C` | `\x1bOC` |
| ← Left | `\x1b[D` | `\x1bOD` |

### 3.2 带修饰键的方向键

| 按键 | 序列 | 说明 |
|------|------|------|
| Shift+↑ | `\x1b[1;2A` | 选择向上 |
| Shift+↓ | `\x1b[1;2B` | 选择向下 |
| Shift+→ | `\x1b[1;2C` | 选择向右 |
| Shift+← | `\x1b[1;2D` | 选择向左 |
| Alt+↑ | `\x1b[1;3A` | - |
| Alt+↓ | `\x1b[1;3B` | - |
| Alt+→ | `\x1b[1;3C` | 下一个单词 |
| Alt+← | `\x1b[1;3D` | 上一个单词 |
| Ctrl+↑ | `\x1b[1;5A` | - |
| Ctrl+↓ | `\x1b[1;5B` | - |
| Ctrl+→ | `\x1b[1;5C` | 下一个单词 |
| Ctrl+← | `\x1b[1;5D` | 上一个单词 |

### 3.3 修饰键编码

| 修饰键 | 编码值 |
|--------|--------|
| Shift | 2 |
| Alt | 3 |
| Shift+Alt | 4 |
| Ctrl | 5 |
| Shift+Ctrl | 6 |
| Alt+Ctrl | 7 |
| Shift+Alt+Ctrl | 8 |

### 3.4 导航键

| 按键 | 序列 | 说明 |
|------|------|------|
| Home | `\x1b[H` | 行首 |
| End | `\x1b[F` | 行尾 |
| Insert | `\x1b[2~` | 插入模式 |
| Delete | `\x1b[3~` | 删除字符 |
| Page Up | `\x1b[5~` | 上翻页 |
| Page Down | `\x1b[6~` | 下翻页 |

---

## 四、功能键 (F1-F12)

### 4.1 标准功能键

| 按键 | 序列 | 备用序列 |
|------|------|----------|
| F1 | `\x1bOP` | `\x1b[11~` |
| F2 | `\x1bOQ` | `\x1b[12~` |
| F3 | `\x1bOR` | `\x1b[13~` |
| F4 | `\x1bOS` | `\x1b[14~` |
| F5 | `\x1b[15~` | - |
| F6 | `\x1b[17~` | - |
| F7 | `\x1b[18~` | - |
| F8 | `\x1b[19~` | - |
| F9 | `\x1b[20~` | - |
| F10 | `\x1b[21~` | - |
| F11 | `\x1b[23~` | - |
| F12 | `\x1b[24~` | - |

### 4.2 带修饰键的功能键

格式: `\x1b[<code>;<modifier>~`

| 按键 | 序列 |
|------|------|
| Shift+F1 | `\x1b[1;2P` |
| Ctrl+F1 | `\x1b[1;5P` |
| Alt+F1 | `\x1b[1;3P` |
| Shift+F5 | `\x1b[15;2~` |
| Ctrl+F5 | `\x1b[15;5~` |

---

## 五、数字键盘

### 5.1 普通模式

| 按键 | 序列 |
|------|------|
| Numpad 0-9 | `0`-`9` |
| Numpad + | `+` |
| Numpad - | `-` |
| Numpad * | `*` |
| Numpad / | `/` |
| Numpad . | `.` |
| Numpad Enter | `\r` |

### 5.2 应用模式

| 按键 | 序列 |
|------|------|
| Numpad 0 | `\x1bOp` |
| Numpad 1 | `\x1bOq` |
| Numpad 2 | `\x1bOr` |
| Numpad 3 | `\x1bOs` |
| Numpad 4 | `\x1bOt` |
| Numpad 5 | `\x1bOu` |
| Numpad 6 | `\x1bOv` |
| Numpad 7 | `\x1bOw` |
| Numpad 8 | `\x1bOx` |
| Numpad 9 | `\x1bOy` |
| Numpad + | `\x1bOk` |
| Numpad - | `\x1bOm` |
| Numpad * | `\x1bOj` |
| Numpad / | `\x1bOo` |
| Numpad . | `\x1bOn` |
| Numpad Enter | `\x1bOM` |

---

## 六、实现代码

### 6.1 按键转换器结构

```rust
use gpui::*;

/// 终端模式标志
#[derive(Clone, Copy, Default)]
pub struct TerminalMode {
    /// 应用光标键模式 (DECCKM)
    pub application_cursor: bool,
    /// 应用数字键盘模式 (DECNKM)
    pub application_keypad: bool,
}

/// 按键转换器
pub struct KeyMapper {
    mode: TerminalMode,
}

impl KeyMapper {
    pub fn new() -> Self {
        Self {
            mode: TerminalMode::default(),
        }
    }

    pub fn set_mode(&mut self, mode: TerminalMode) {
        self.mode = mode;
    }

    /// 将按键事件转换为字节序列
    pub fn key_to_bytes(&self, event: &KeyDownEvent) -> Option<Vec<u8>> {
        let key = &event.keystroke.key;
        let modifiers = &event.keystroke.modifiers;

        // 处理 Ctrl 组合键
        if modifiers.control && !modifiers.alt && !modifiers.shift {
            if let Some(bytes) = self.ctrl_key_bytes(key) {
                return Some(bytes);
            }
        }

        // 处理功能键
        if let Some(bytes) = self.function_key_bytes(key, modifiers) {
            return Some(bytes);
        }

        // 处理方向键
        if let Some(bytes) = self.arrow_key_bytes(key, modifiers) {
            return Some(bytes);
        }

        // 处理导航键
        if let Some(bytes) = self.navigation_key_bytes(key, modifiers) {
            return Some(bytes);
        }

        // 处理普通字符
        if let Some(bytes) = self.char_bytes(key, modifiers) {
            return Some(bytes);
        }

        None
    }
}
```

### 6.2 Ctrl 组合键处理

```rust
impl KeyMapper {
    fn ctrl_key_bytes(&self, key: &str) -> Option<Vec<u8>> {
        let byte = match key.to_lowercase().as_str() {
            "a" => 0x01, "b" => 0x02, "c" => 0x03, "d" => 0x04,
            "e" => 0x05, "f" => 0x06, "g" => 0x07, "h" => 0x08,
            "i" => 0x09, "j" => 0x0A, "k" => 0x0B, "l" => 0x0C,
            "m" => 0x0D, "n" => 0x0E, "o" => 0x0F, "p" => 0x10,
            "q" => 0x11, "r" => 0x12, "s" => 0x13, "t" => 0x14,
            "u" => 0x15, "v" => 0x16, "w" => 0x17, "x" => 0x18,
            "y" => 0x19, "z" => 0x1A,
            "@" | "2" => 0x00,
            "[" => 0x1B,
            "\\" => 0x1C,
            "]" => 0x1D,
            "^" | "6" => 0x1E,
            "_" | "-" => 0x1F,
            _ => return None,
        };
        Some(vec![byte])
    }
```

### 6.3 方向键处理

```rust
impl KeyMapper {
    fn arrow_key_bytes(&self, key: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
        let base = match key {
            "up" => 'A',
            "down" => 'B',
            "right" => 'C',
            "left" => 'D',
            _ => return None,
        };

        let modifier_code = self.modifier_code(modifiers);

        let bytes = if modifier_code > 1 {
            // 带修饰键: \x1b[1;<mod><dir>
            format!("\x1b[1;{}{}", modifier_code, base).into_bytes()
        } else if self.mode.application_cursor {
            // 应用模式: \x1bO<dir>
            format!("\x1bO{}", base).into_bytes()
        } else {
            // 普通模式: \x1b[<dir>
            format!("\x1b[{}", base).into_bytes()
        };

        Some(bytes)
    }

    fn modifier_code(&self, modifiers: &Modifiers) -> u8 {
        let mut code = 1u8;
        if modifiers.shift { code += 1; }
        if modifiers.alt { code += 2; }
        if modifiers.control { code += 4; }
        code
    }
}
```

### 6.4 功能键处理

```rust
impl KeyMapper {
    fn function_key_bytes(&self, key: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
        let (prefix, code) = match key {
            "f1" => ("\x1bO", "P"),
            "f2" => ("\x1bO", "Q"),
            "f3" => ("\x1bO", "R"),
            "f4" => ("\x1bO", "S"),
            "f5" => ("\x1b[", "15~"),
            "f6" => ("\x1b[", "17~"),
            "f7" => ("\x1b[", "18~"),
            "f8" => ("\x1b[", "19~"),
            "f9" => ("\x1b[", "20~"),
            "f10" => ("\x1b[", "21~"),
            "f11" => ("\x1b[", "23~"),
            "f12" => ("\x1b[", "24~"),
            _ => return None,
        };

        let modifier_code = self.modifier_code(modifiers);

        let bytes = if modifier_code > 1 {
            // 带修饰键
            if prefix == "\x1bO" {
                format!("\x1b[1;{}{}", modifier_code, code).into_bytes()
            } else {
                let num: u8 = code.trim_end_matches('~').parse().unwrap_or(0);
                format!("\x1b[{};{}~", num, modifier_code).into_bytes()
            }
        } else {
            format!("{}{}", prefix, code).into_bytes()
        };

        Some(bytes)
    }
}
```

### 6.5 导航键处理

```rust
impl KeyMapper {
    fn navigation_key_bytes(&self, key: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
        let modifier_code = self.modifier_code(modifiers);

        let bytes = match key {
            "home" => {
                if modifier_code > 1 {
                    format!("\x1b[1;{}H", modifier_code).into_bytes()
                } else {
                    b"\x1b[H".to_vec()
                }
            }
            "end" => {
                if modifier_code > 1 {
                    format!("\x1b[1;{}F", modifier_code).into_bytes()
                } else {
                    b"\x1b[F".to_vec()
                }
            }
            "insert" => {
                if modifier_code > 1 {
                    format!("\x1b[2;{}~", modifier_code).into_bytes()
                } else {
                    b"\x1b[2~".to_vec()
                }
            }
            "delete" => {
                if modifier_code > 1 {
                    format!("\x1b[3;{}~", modifier_code).into_bytes()
                } else {
                    b"\x1b[3~".to_vec()
                }
            }
            "pageup" => {
                if modifier_code > 1 {
                    format!("\x1b[5;{}~", modifier_code).into_bytes()
                } else {
                    b"\x1b[5~".to_vec()
                }
            }
            "pagedown" => {
                if modifier_code > 1 {
                    format!("\x1b[6;{}~", modifier_code).into_bytes()
                } else {
                    b"\x1b[6~".to_vec()
                }
            }
            _ => return None,
        };

        Some(bytes)
    }
}
```

### 6.6 普通字符处理

```rust
impl KeyMapper {
    fn char_bytes(&self, key: &str, modifiers: &Modifiers) -> Option<Vec<u8>> {
        match key {
            "enter" => Some(b"\r".to_vec()),
            "tab" => Some(b"\t".to_vec()),
            "backspace" => Some(vec![0x7F]),
            "escape" => Some(vec![0x1B]),
            "space" => Some(b" ".to_vec()),
            _ => {
                // 普通字符
                if key.len() == 1 {
                    let c = key.chars().next()?;
                    if modifiers.alt {
                        // Alt+字符:发送 ESC + 字符
                        let mut bytes = vec![0x1B];
                        bytes.extend(c.to_string().as_bytes());
                        Some(bytes)
                    } else {
                        Some(c.to_string().into_bytes())
                    }
                } else {
                    None
                }
            }
        }
    }
}
```

---

## 七、应用模式切换

### 7.1 模式控制序列

| 序列 | 说明 |
|------|------|
| `\x1b[?1h` | 启用应用光标键模式 (DECCKM) |
| `\x1b[?1l` | 禁用应用光标键模式 |
| `\x1b[?66h` | 启用应用数字键盘模式 |
| `\x1b[?66l` | 禁用应用数字键盘模式 |
| `\x1b=` | 启用应用键盘模式 (DECKPAM) |
| `\x1b>` | 禁用应用键盘模式 (DECKPNM) |

### 7.2 模式检测

```rust
/// 从终端状态获取当前模式
pub fn get_terminal_mode(term: &Term<impl EventListener>) -> TerminalMode {
    let mode = term.mode();
    TerminalMode {
        application_cursor: mode.contains(TermMode::APP_CURSOR),
        application_keypad: mode.contains(TermMode::APP_KEYPAD),
    }
}
```

---

## 八、常用快捷键参考

### 8.1 Shell 快捷键 (Bash/Zsh)

| 快捷键 | 功能 |
|--------|------|
| Ctrl+A | 移动到行首 |
| Ctrl+E | 移动到行尾 |
| Ctrl+B | 向左移动一个字符 |
| Ctrl+F | 向右移动一个字符 |
| Alt+B | 向左移动一个单词 |
| Alt+F | 向右移动一个单词 |
| Ctrl+U | 删除到行首 |
| Ctrl+K | 删除到行尾 |
| Ctrl+W | 删除前一个单词 |
| Ctrl+Y | 粘贴删除的内容 |
| Ctrl+L | 清屏 |
| Ctrl+R | 反向搜索历史 |
| Ctrl+C | 中断当前命令 |
| Ctrl+Z | 挂起当前进程 |
| Ctrl+D | 退出 / EOF |

### 8.2 Vim 常用键

| 按键 | 功能 |
|------|------|
| Escape | 返回普通模式 |
| i | 进入插入模式 |
| : | 进入命令模式 |
| h/j/k/l | 左/下/上/右移动 |
| dd | 删除行 |
| yy | 复制行 |
| p | 粘贴 |
| u | 撤销 |
| Ctrl+R | 重做 |

---

## 九、相关文档

- [终端渲染实现](./terminal-rendering.md) - 渲染细节
- [TerminalView](./terminal-view.md) - 视图组件
- [Zed 终端分析](./zed-terminal-analysis.md) - 参考实现