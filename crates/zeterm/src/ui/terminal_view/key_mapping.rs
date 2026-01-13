//! 按键映射模块
//!
//! 将GPUI 按键事件转换为终端 ANSI 转义序列。
//! 参考 Zed 编辑器的 mappings 模块实现。

/// 按键映射结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMapping {
    /// 输出字节序列
    pub bytes: Vec<u8>,
}

impl KeyMapping {
    /// 创建新的按键映射
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// 创建空映射
    pub fn empty() -> Self {
        Self { bytes: Vec::new() }
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// 修饰键状态
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    /// 创建新的修饰键状态
    pub fn new(control: bool, alt: bool, shift: bool) -> Self {
        Self {
            control,
            alt,
            shift,
        }
    }
}

/// 将按键转换为终端输入字节
///
/// # Arguments
/// * `key` - 按键名称 (如 "enter", "a", "f1" 等)
/// * `modifiers` - 修饰键状态
///
/// # Returns
/// 转换后的字节序列
pub fn keystroke_to_bytes(key: &str, modifiers: Modifiers) -> KeyMapping {
    let mut bytes = Vec::new();

    // 处理特殊键
    if let Some(special_bytes) = map_special_key(key) {
        bytes.extend_from_slice(special_bytes);
        return KeyMapping::new(bytes);
    }

    // 处理功能键 F1-F12
    if let Some(fn_bytes) = map_function_key(key) {
        bytes.extend_from_slice(fn_bytes);
        return KeyMapping::new(bytes);
    }

    // 处理方向键
    if let Some(arrow_bytes) = map_arrow_key(key, modifiers) {
        bytes.extend_from_slice(&arrow_bytes);
        return KeyMapping::new(bytes);
    }

    // 处理导航键 (Home, End, PageUp, PageDown, Insert, Delete)
    if let Some(nav_bytes) = map_navigation_key(key) {
        bytes.extend_from_slice(nav_bytes);
        return KeyMapping::new(bytes);
    }

    // 处理普通字符
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        let char_bytes = map_character(c, modifiers);
        bytes.extend_from_slice(&char_bytes);
    }

    KeyMapping::new(bytes)
}

/// 映射特殊键
fn map_special_key(key: &str) -> Option<&'static [u8]> {
    match key {
        "enter" => Some(b"\r"),
        "backspace" => Some(&[0x7f]),
        "tab" => Some(b"\t"),
        "escape" => Some(&[0x1b]),
        "space" => Some(b" "),
        _ => None,
    }
}

/// 映射功能键F1-F12
fn map_function_key(key: &str) -> Option<&'static [u8]> {
    match key {
        "f1" => Some(b"\x1bOP"),
        "f2" => Some(b"\x1bOQ"),
        "f3" => Some(b"\x1bOR"),
        "f4" => Some(b"\x1bOS"),
        "f5" => Some(b"\x1b[15~"),
        "f6" => Some(b"\x1b[17~"),
        "f7" => Some(b"\x1b[18~"),
        "f8" => Some(b"\x1b[19~"),
        "f9" => Some(b"\x1b[20~"),
        "f10" => Some(b"\x1b[21~"),
        "f11" => Some(b"\x1b[23~"),
        "f12" => Some(b"\x1b[24~"),
        _ => None,
    }
}

/// 映射方向键
///支持带修饰键的方向键序列
fn map_arrow_key(key: &str, modifiers: Modifiers) -> Option<Vec<u8>> {
    let base_char = match key {
        "up" => 'A',
        "down" => 'B',
        "right" => 'C',
        "left" => 'D',
        _ => return None,
    };

    // 计算修饰键参数
    let modifier_param = calculate_modifier_param(modifiers);

    let bytes = if modifier_param > 1 {
        // 带修饰键: ESC [1 ;<modifier> <char>
        format!("\x1b[1;{}{}", modifier_param, base_char).into_bytes()
    } else {
        // 无修饰键: ESC [ <char>
        format!("\x1b[{}", base_char).into_bytes()
    };

    Some(bytes)
}

/// 映射导航键
fn map_navigation_key(key: &str) -> Option<&'static [u8]> {
    match key {
        "home" => Some(b"\x1b[H"),
        "end" => Some(b"\x1b[F"),
        "pageup" => Some(b"\x1b[5~"),
        "pagedown" => Some(b"\x1b[6~"),
        "insert" => Some(b"\x1b[2~"),
        "delete" => Some(b"\x1b[3~"),
        _ => None,
    }
}

/// 映射普通字符
fn map_character(c: char, modifiers: Modifiers) -> Vec<u8> {
    let mut bytes = Vec::new();

    if modifiers.control {
        // Ctrl+字母->控制字符 (ASCII 1-26)
        if c.is_ascii_lowercase() {
            bytes.push(c as u8 - b'a' + 1);
        } else if c.is_ascii_uppercase() {
            bytes.push(c as u8 - b'A' + 1);
        } else {
            // Ctrl + 特殊字符
            match c {
                '@' => bytes.push(0),     // Ctrl+@ = NUL
                '[' => bytes.push(0x1b),  // Ctrl+[ = ESC
                '\\' => bytes.push(0x1c), // Ctrl+\ = FS
                ']' => bytes.push(0x1d),  // Ctrl+] = GS
                '^' => bytes.push(0x1e),  // Ctrl+^ = RS
                '_' => bytes.push(0x1f),  // Ctrl+_ = US
                '?' => bytes.push(0x7f),  // Ctrl+? = DEL
                _ => {},
            }
        }
    } else if modifiers.alt {
        // Alt+字符 -> ESC + 字符
        bytes.push(0x1b);
        bytes.extend_from_slice(c.to_string().as_bytes());
    } else {
        // 普通字符
        bytes.extend_from_slice(c.to_string().as_bytes());
    }

    bytes
}

/// 计算修饰键参数
///
/// 用于 CSI 序列中的修饰键编码
/// 参考: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
fn calculate_modifier_param(modifiers: Modifiers) -> u8 {
    let mut param = 1u8;

    if modifiers.shift {
        param += 1;
    }
    if modifiers.alt {
        param += 2;
    }
    if modifiers.control {
        param += 4;
    }

    param
}

#[cfg(test)]
mod tests {
    use super::*;

    //==================== 特殊键测试 ====================

    #[test]
    fn test_enter_key() {
        let result = keystroke_to_bytes("enter", Modifiers::default());
        assert_eq!(result.bytes, b"\r");
    }

    #[test]
    fn test_backspace_key() {
        let result = keystroke_to_bytes("backspace", Modifiers::default());
        assert_eq!(result.bytes, vec![0x7f]);
    }

    #[test]
    fn test_tab_key() {
        let result = keystroke_to_bytes("tab", Modifiers::default());
        assert_eq!(result.bytes, b"\t");
    }

    #[test]
    fn test_escape_key() {
        let result = keystroke_to_bytes("escape", Modifiers::default());
        assert_eq!(result.bytes, vec![0x1b]);
    }

    #[test]
    fn test_space_key() {
        let result = keystroke_to_bytes("space", Modifiers::default());
        assert_eq!(result.bytes, b" ");
    }

    // ==================== 方向键测试 ====================

    #[test]
    fn test_arrow_up() {
        let result = keystroke_to_bytes("up", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[A");
    }

    #[test]
    fn test_arrow_down() {
        let result = keystroke_to_bytes("down", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[B");
    }

    #[test]
    fn test_arrow_right() {
        let result = keystroke_to_bytes("right", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[C");
    }

    #[test]
    fn test_arrow_left() {
        let result = keystroke_to_bytes("left", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[D");
    }

    #[test]
    fn test_arrow_with_shift() {
        let modifiers = Modifiers::new(false, false, true);
        let result = keystroke_to_bytes("up", modifiers);
        assert_eq!(result.bytes, b"\x1b[1;2A");
    }

    #[test]
    fn test_arrow_with_alt() {
        let modifiers = Modifiers::new(false, true, false);
        let result = keystroke_to_bytes("up", modifiers);
        assert_eq!(result.bytes, b"\x1b[1;3A");
    }

    #[test]
    fn test_arrow_with_ctrl() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("up", modifiers);
        assert_eq!(result.bytes, b"\x1b[1;5A");
    }

    #[test]
    fn test_arrow_with_ctrl_shift() {
        let modifiers = Modifiers::new(true, false, true);
        let result = keystroke_to_bytes("up", modifiers);
        assert_eq!(result.bytes, b"\x1b[1;6A");
    }

    // ==================== 导航键测试 ====================

    #[test]
    fn test_home_key() {
        let result = keystroke_to_bytes("home", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[H");
    }

    #[test]
    fn test_end_key() {
        let result = keystroke_to_bytes("end", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[F");
    }

    #[test]
    fn test_pageup_key() {
        let result = keystroke_to_bytes("pageup", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[5~");
    }

    #[test]
    fn test_pagedown_key() {
        let result = keystroke_to_bytes("pagedown", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[6~");
    }

    #[test]
    fn test_insert_key() {
        let result = keystroke_to_bytes("insert", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[2~");
    }

    #[test]
    fn test_delete_key() {
        let result = keystroke_to_bytes("delete", Modifiers::default());
        assert_eq!(result.bytes, b"\x1b[3~");
    }

    // ==================== 功能键测试 ====================

    #[test]
    fn test_function_keys() {
        let expected: [(&str, &[u8]); 12] = [
            ("f1", b"\x1bOP"),
            ("f2", b"\x1bOQ"),
            ("f3", b"\x1bOR"),
            ("f4", b"\x1bOS"),
            ("f5", b"\x1b[15~"),
            ("f6", b"\x1b[17~"),
            ("f7", b"\x1b[18~"),
            ("f8", b"\x1b[19~"),
            ("f9", b"\x1b[20~"),
            ("f10", b"\x1b[21~"),
            ("f11", b"\x1b[23~"),
            ("f12", b"\x1b[24~"),
        ];

        for (key, expected_bytes) in expected {
            let result = keystroke_to_bytes(key, Modifiers::default());
            assert_eq!(result.bytes, expected_bytes, "Failed for key: {}", key);
        }
    }

    // ==================== 普通字符测试 ====================

    #[test]
    fn test_lowercase_letter() {
        let result = keystroke_to_bytes("a", Modifiers::default());
        assert_eq!(result.bytes, b"a");
    }

    #[test]
    fn test_uppercase_letter() {
        let result = keystroke_to_bytes("A", Modifiers::default());
        assert_eq!(result.bytes, b"A");
    }

    #[test]
    fn test_digit() {
        let result = keystroke_to_bytes("5", Modifiers::default());
        assert_eq!(result.bytes, b"5");
    }

    // ==================== Ctrl 组合键测试 ====================

    #[test]
    fn test_ctrl_c() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("c", modifiers);
        assert_eq!(result.bytes, vec![3]); // ETX (End of Text)
    }

    #[test]
    fn test_ctrl_d() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("d", modifiers);
        assert_eq!(result.bytes, vec![4]); // EOT (End of Transmission)
    }

    #[test]
    fn test_ctrl_z() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("z", modifiers);
        assert_eq!(result.bytes, vec![26]); // SUB (Substitute)
    }

    #[test]
    fn test_ctrl_a() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("a", modifiers);
        assert_eq!(result.bytes, vec![1]); // SOH (Start of Heading)
    }

    #[test]
    fn test_ctrl_uppercase() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("C", modifiers);
        assert_eq!(result.bytes, vec![3]); // Same as Ctrl+c
    }

    #[test]
    fn test_ctrl_bracket() {
        let modifiers = Modifiers::new(true, false, false);
        let result = keystroke_to_bytes("[", modifiers);
        assert_eq!(result.bytes, vec![0x1b]); // ESC
    }

    // ==================== Alt 组合键测试 ====================

    #[test]
    fn test_alt_a() {
        let modifiers = Modifiers::new(false, true, false);
        let result = keystroke_to_bytes("a", modifiers);
        assert_eq!(result.bytes, vec![0x1b, b'a']); // ESC + a
    }

    #[test]
    fn test_alt_x() {
        let modifiers = Modifiers::new(false, true, false);
        let result = keystroke_to_bytes("x", modifiers);
        assert_eq!(result.bytes, vec![0x1b, b'x']); // ESC + x
    }

    // ==================== 修饰键参数计算测试 ====================

    #[test]
    fn test_modifier_param_none() {
        let modifiers = Modifiers::default();
        assert_eq!(calculate_modifier_param(modifiers), 1);
    }

    #[test]
    fn test_modifier_param_shift() {
        let modifiers = Modifiers::new(false, false, true);
        assert_eq!(calculate_modifier_param(modifiers), 2);
    }

    #[test]
    fn test_modifier_param_alt() {
        let modifiers = Modifiers::new(false, true, false);
        assert_eq!(calculate_modifier_param(modifiers), 3);
    }

    #[test]
    fn test_modifier_param_ctrl() {
        let modifiers = Modifiers::new(true, false, false);
        assert_eq!(calculate_modifier_param(modifiers), 5);
    }

    #[test]
    fn test_modifier_param_ctrl_alt() {
        let modifiers = Modifiers::new(true, true, false);
        assert_eq!(calculate_modifier_param(modifiers), 7);
    }

    #[test]
    fn test_modifier_param_all() {
        let modifiers = Modifiers::new(true, true, true);
        assert_eq!(calculate_modifier_param(modifiers), 8);
    }

    // ==================== 边界情况测试 ====================

    #[test]
    fn test_unknown_key() {
        let result = keystroke_to_bytes("unknown_key", Modifiers::default());
        assert!(result.is_empty());
    }

    #[test]
    fn test_empty_key() {
        let result = keystroke_to_bytes("", Modifiers::default());
        assert!(result.is_empty());
    }
}
