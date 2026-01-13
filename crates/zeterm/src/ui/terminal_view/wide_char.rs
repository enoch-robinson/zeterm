//! 宽字符处理模块
//!
//! 处理终端中的宽字符（如中日韩字符），这些字符占用两个单元格宽度。
//! 参考 Unicode East Asian Width 属性。

/// 字符宽度类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharWidth {
    /// 零宽度字符（如组合字符）
    Zero,
    /// 窄字符（占用 1 列）
    Narrow,
    /// 宽字符（占用 2 列）
    Wide,
}

impl CharWidth {
    /// 获取列数
    pub fn columns(&self) -> usize {
        match self {
            CharWidth::Zero => 0,
            CharWidth::Narrow => 1,
            CharWidth::Wide => 2,
        }
    }
}

/// 判断字符是否为宽字符
///
/// 宽字符包括：
/// - CJK 统一表意文字
/// - CJK 兼容表意文字
/// - 全角字符
/// - 部分特殊符号
pub fn is_wide_char(c: char) -> bool {
    let cp = c as u32;

    // CJK 统一表意文字
    if (0x4E00..=0x9FFF).contains(&cp) {
        return true;
    }

    // CJK 扩展 A
    if (0x3400..=0x4DBF).contains(&cp) {
        return true;
    }

    // CJK 扩展 B-F
    if (0x20000..=0x2A6DF).contains(&cp) {
        return true;
    }
    if (0x2A700..=0x2B73F).contains(&cp) {
        return true;
    }
    if (0x2B740..=0x2B81F).contains(&cp) {
        return true;
    }
    if (0x2B820..=0x2CEAF).contains(&cp) {
        return true;
    }
    if (0x2CEB0..=0x2EBEF).contains(&cp) {
        return true;
    }

    // CJK 兼容表意文字
    if (0xF900..=0xFAFF).contains(&cp) {
        return true;
    }

    // 全角 ASCII和标点
    if (0xFF01..=0xFF60).contains(&cp) {
        return true;
    }

    // 全角字母
    if (0xFFE0..=0xFFE6).contains(&cp) {
        return true;
    }

    // 日文平假名
    if (0x3040..=0x309F).contains(&cp) {
        return true;
    }

    // 日文片假名
    if (0x30A0..=0x30FF).contains(&cp) {
        return true;
    }

    // 韩文音节
    if (0xAC00..=0xD7AF).contains(&cp) {
        return true;
    }

    // 韩文字母
    if (0x1100..=0x11FF).contains(&cp) {
        return true;
    }

    // CJK 符号和标点
    if (0x3000..=0x303F).contains(&cp) {
        return true;
    }

    // 注音符号
    if (0x3100..=0x312F).contains(&cp) {
        return true;
    }

    // Emoji 字符（大部分 Emoji 占用 2 列）
    if is_emoji(c) {
        return true;
    }

    false
}

/// 判断字符是否为 Emoji
///
/// Emoji 字符包括：
/// - 基本 Emoji (表情符号)
/// - Emoji 修饰符
/// - Emoji 组件
/// - 各种符号 Emoji
pub fn is_emoji(c: char) -> bool {
    let cp = c as u32;

    // Miscellaneous Symbols (☀ ☁ ☂等)
    if (0x2600..=0x26FF).contains(&cp) {
        return true;
    }

    // Dingbats (✂ ✈ ✉ 等)
    if (0x2700..=0x27BF).contains(&cp) {
        return true;
    }

    // Emoticons (😀 😁 😂 等)
    if (0x1F600..=0x1F64F).contains(&cp) {
        return true;
    }

    // Miscellaneous Symbols and Pictographs (🌀 🌁 🌂 等)
    if (0x1F300..=0x1F5FF).contains(&cp) {
        return true;
    }

    // Transport and Map Symbols (🚀 🚁 🚂 等)
    if (0x1F680..=0x1F6FF).contains(&cp) {
        return true;
    }

    // Supplemental Symbols and Pictographs (🤐 🤑 🤒 等)
    if (0x1F900..=0x1F9FF).contains(&cp) {
        return true;
    }

    // Symbols and Pictographs Extended-A (🥰 🥱 等)
    if (0x1FA00..=0x1FA6F).contains(&cp) {
        return true;
    }

    // Symbols and Pictographs Extended-B
    if (0x1FA70..=0x1FAFF).contains(&cp) {
        return true;
    }

    // Regional Indicator Symbols (🇦🇧 等，用于国旗)
    if (0x1F1E0..=0x1F1FF).contains(&cp) {
        return true;
    }

    // Mahjong Tiles (🀀 🀁 等)
    if (0x1F000..=0x1F02F).contains(&cp) {
        return true;
    }

    // Domino Tiles (🁠 🁡 等)
    if (0x1F030..=0x1F09F).contains(&cp) {
        return true;
    }

    // Playing Cards (🂠 🂡 等)
    if (0x1F0A0..=0x1F0FF).contains(&cp) {
        return true;
    }

    // Chess Symbols
    if (0x1FA00..=0x1FA0F).contains(&cp) {
        return true;
    }

    false
}

/// 判断字符是否为零宽度字符
///
/// 零宽度字符包括：
/// - 组合字符
/// - 零宽空格
/// - 控制字符
pub fn is_zero_width_char(c: char) -> bool {
    let cp = c as u32;

    // 控制字符
    if cp < 0x20 {
        return true;
    }

    // DEL
    if cp == 0x7F {
        return true;
    }

    // 零宽空格
    if cp == 0x200B {
        return true;
    }

    // 零宽非连接符
    if cp == 0x200C {
        return true;
    }

    // 零宽连接符
    if cp == 0x200D {
        return true;
    }

    // 组合字符 (Combining Diacritical Marks)
    if (0x0300..=0x036F).contains(&cp) {
        return true;
    }

    // 组合字符扩展
    if (0x1AB0..=0x1AFF).contains(&cp) {
        return true;
    }
    if (0x1DC0..=0x1DFF).contains(&cp) {
        return true;
    }
    if (0x20D0..=0x20FF).contains(&cp) {
        return true;
    }
    if (0xFE20..=0xFE2F).contains(&cp) {
        return true;
    }

    false
}

/// 获取字符的显示宽度
pub fn char_width(c: char) -> CharWidth {
    if is_zero_width_char(c) {
        CharWidth::Zero
    } else if is_wide_char(c) {
        CharWidth::Wide
    } else {
        CharWidth::Narrow
    }
}

/// 计算字符串的显示宽度（列数）
pub fn string_width(s: &str) -> usize {
    s.chars().map(|c| char_width(c).columns()).sum()
}

///宽字符单元格信息
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellContent {
    /// 空单元格
    Empty,
    /// 窄字符
    Narrow(char),
    /// 宽字符的第一个单元格
    WideFirst(char),
    /// 宽字符的第二个单元格（占位符）
    WidePlaceholder,
}

impl CellContent {
    /// 是否为占位符
    pub fn is_placeholder(&self) -> bool {
        matches!(self, CellContent::WidePlaceholder)
    }

    /// 是否为宽字符的第一个单元格
    pub fn is_wide_first(&self) -> bool {
        matches!(self, CellContent::WideFirst(_))
    }

    /// 获取字符（如果有）
    pub fn char(&self) -> Option<char> {
        match self {
            CellContent::Narrow(c) | CellContent::WideFirst(c) => Some(*c),
            _ => None,
        }
    }
}

/// 将字符串转换为单元格内容列表
///
/// 宽字符会生成两个单元格：WideFirst 和 WidePlaceholder
pub fn string_to_cells(s: &str) -> Vec<CellContent> {
    let mut cells = Vec::new();

    for c in s.chars() {
        let width = char_width(c);
        match width {
            CharWidth::Zero => {
                // 零宽度字符不占用单元格
            },
            CharWidth::Narrow => {
                cells.push(CellContent::Narrow(c));
            },
            CharWidth::Wide => {
                cells.push(CellContent::WideFirst(c));
                cells.push(CellContent::WidePlaceholder);
            },
        }
    }

    cells
}

/// 从列位置获取实际字符索引
///
/// 考虑宽字符占用两列的情况
pub fn column_to_char_index(s: &str, column: usize) -> Option<usize> {
    let mut current_col = 0;

    for (idx, c) in s.chars().enumerate() {
        if current_col == column {
            return Some(idx);
        }

        let width = char_width(c).columns();
        current_col += width;

        // 如果目标列在宽字符的第二列
        if current_col > column {
            return Some(idx);
        }
    }

    None
}

/// 从字符索引获取列位置
pub fn char_index_to_column(s: &str, char_index: usize) -> usize {
    s.chars()
        .take(char_index)
        .map(|c| char_width(c).columns())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    //==================== 宽字符判断测试 ====================

    #[test]
    fn test_is_wide_char_cjk() {
        // 中文
        assert!(is_wide_char('中'));
        assert!(is_wide_char('文'));
        assert!(is_wide_char('字'));

        // 日文汉字
        assert!(is_wide_char('日'));
        assert!(is_wide_char('本'));

        // 韩文
        assert!(is_wide_char('한'));
        assert!(is_wide_char('글'));
    }

    #[test]
    fn test_is_wide_char_japanese() {
        // 平假名
        assert!(is_wide_char('あ'));
        assert!(is_wide_char('い'));
        assert!(is_wide_char('う'));

        // 片假名
        assert!(is_wide_char('ア'));
        assert!(is_wide_char('イ'));
        assert!(is_wide_char('ウ'));
    }

    #[test]
    fn test_is_wide_char_fullwidth() {
        // 全角 ASCII
        assert!(is_wide_char('Ａ'));
        assert!(is_wide_char('Ｂ'));
        assert!(is_wide_char('１'));
        assert!(is_wide_char('！'));
    }

    #[test]
    fn test_is_wide_char_narrow() {
        // ASCII
        assert!(!is_wide_char('a'));
        assert!(!is_wide_char('Z'));
        assert!(!is_wide_char('0'));
        assert!(!is_wide_char('!'));
        assert!(!is_wide_char(' '));
    }

    // ==================== 零宽度字符测试 ====================

    #[test]
    fn test_is_zero_width_control() {
        assert!(is_zero_width_char('\0'));
        assert!(is_zero_width_char('\n'));
        assert!(is_zero_width_char('\r'));
        assert!(is_zero_width_char('\t'));
    }

    #[test]
    fn test_is_zero_width_special() {
        assert!(is_zero_width_char('\u{200B}')); // 零宽空格
        assert!(is_zero_width_char('\u{200C}')); // 零宽非连接符
        assert!(is_zero_width_char('\u{200D}')); // 零宽连接符
    }

    #[test]
    fn test_is_zero_width_combining() {
        assert!(is_zero_width_char('\u{0300}')); // 组合重音符
        assert!(is_zero_width_char('\u{0301}')); // 组合锐音符
    }

    // ==================== 字符宽度测试 ====================

    #[test]
    fn test_char_width() {
        assert_eq!(char_width('a'), CharWidth::Narrow);
        assert_eq!(char_width('中'), CharWidth::Wide);
        assert_eq!(char_width('\n'), CharWidth::Zero);
    }

    #[test]
    fn test_char_width_columns() {
        assert_eq!(CharWidth::Zero.columns(), 0);
        assert_eq!(CharWidth::Narrow.columns(), 1);
        assert_eq!(CharWidth::Wide.columns(), 2);
    }

    // ==================== 字符串宽度测试 ====================

    #[test]
    fn test_string_width_ascii() {
        assert_eq!(string_width("hello"), 5);
        assert_eq!(string_width(""), 0);
        assert_eq!(string_width(" "), 1);
    }

    #[test]
    fn test_string_width_cjk() {
        assert_eq!(string_width("中文"), 4);
        assert_eq!(string_width("日本語"), 6);
        assert_eq!(string_width("한글"), 4);
    }

    #[test]
    fn test_string_width_mixed() {
        assert_eq!(string_width("hello中文"), 9); // 5 + 4
        assert_eq!(string_width("a中b"), 4); // 1 + 2+ 1
        assert_eq!(string_width("中a文"), 5); // 2 + 1 + 2
    }

    // ==================== 单元格内容测试 ====================

    #[test]
    fn test_cell_content_methods() {
        let narrow = CellContent::Narrow('a');
        let wide_first = CellContent::WideFirst('中');
        let placeholder = CellContent::WidePlaceholder;
        let empty = CellContent::Empty;

        assert!(!narrow.is_placeholder());
        assert!(!wide_first.is_placeholder());
        assert!(placeholder.is_placeholder());
        assert!(!empty.is_placeholder());

        assert!(!narrow.is_wide_first());
        assert!(wide_first.is_wide_first());

        assert_eq!(narrow.char(), Some('a'));
        assert_eq!(wide_first.char(), Some('中'));
        assert_eq!(placeholder.char(), None);
        assert_eq!(empty.char(), None);
    }

    // ==================== 字符串转单元格测试 ====================

    #[test]
    fn test_string_to_cells_ascii() {
        let cells = string_to_cells("abc");
        assert_eq!(cells.len(), 3);
        assert_eq!(cells[0], CellContent::Narrow('a'));
        assert_eq!(cells[1], CellContent::Narrow('b'));
        assert_eq!(cells[2], CellContent::Narrow('c'));
    }

    #[test]
    fn test_string_to_cells_cjk() {
        let cells = string_to_cells("中文");
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], CellContent::WideFirst('中'));
        assert_eq!(cells[1], CellContent::WidePlaceholder);
        assert_eq!(cells[2], CellContent::WideFirst('文'));
        assert_eq!(cells[3], CellContent::WidePlaceholder);
    }

    #[test]
    fn test_string_to_cells_mixed() {
        let cells = string_to_cells("a中b");
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], CellContent::Narrow('a'));
        assert_eq!(cells[1], CellContent::WideFirst('中'));
        assert_eq!(cells[2], CellContent::WidePlaceholder);
        assert_eq!(cells[3], CellContent::Narrow('b'));
    }

    #[test]
    fn test_string_to_cells_empty() {
        let cells = string_to_cells("");
        assert!(cells.is_empty());
    }

    // ==================== 列位置转换测试 ====================

    #[test]
    fn test_column_to_char_index_ascii() {
        let s = "hello";
        assert_eq!(column_to_char_index(s, 0), Some(0));
        assert_eq!(column_to_char_index(s, 2), Some(2));
        assert_eq!(column_to_char_index(s, 4), Some(4));
        assert_eq!(column_to_char_index(s, 5), None);
    }

    #[test]
    fn test_column_to_char_index_cjk() {
        let s = "中文";
        assert_eq!(column_to_char_index(s, 0), Some(0)); // '中' 的第一列
        assert_eq!(column_to_char_index(s, 1), Some(0)); // '中' 的第二列
        assert_eq!(column_to_char_index(s, 2), Some(1)); // '文' 的第一列
        assert_eq!(column_to_char_index(s, 3), Some(1)); // '文' 的第二列
        assert_eq!(column_to_char_index(s, 4), None);
    }

    #[test]
    fn test_column_to_char_index_mixed() {
        let s = "a中b";
        assert_eq!(column_to_char_index(s, 0), Some(0)); // 'a'
        assert_eq!(column_to_char_index(s, 1), Some(1)); // '中' 第一列
        assert_eq!(column_to_char_index(s, 2), Some(1)); // '中' 第二列
        assert_eq!(column_to_char_index(s, 3), Some(2)); // 'b'
    }

    #[test]
    fn test_char_index_to_column() {
        let s = "a中b";
        assert_eq!(char_index_to_column(s, 0), 0); // 'a' 在列0
        assert_eq!(char_index_to_column(s, 1), 1); // '中' 在列 1
        assert_eq!(char_index_to_column(s, 2), 3); // 'b' 在列 3
    }

    // ==================== 边界情况测试 ====================

    #[test]
    fn test_empty_string() {
        assert_eq!(string_width(""), 0);
        assert!(string_to_cells("").is_empty());
        assert_eq!(column_to_char_index("", 0), None);
    }

    #[test]
    fn test_single_wide_char() {
        let s = "中";
        assert_eq!(string_width(s), 2);
        let cells = string_to_cells(s);
        assert_eq!(cells.len(), 2);
    }
}
