//! 超链接模块
//!
//! 提供终端内容中的超链接检测和处理功能，包括：
//! - URL 检测（http、https、ftp、file等协议）
//! - 超链接范围计算
//! - 悬停提示支持

use gpui::Hsla;

//============================================================================
// 超链接类型
// ============================================================================

/// 超链接类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperlinkType {
    /// HTTP/HTTPS URL
    Http,
    /// FTP URL
    Ftp,
    ///文件路径
    File,
    /// 邮件地址
    Email,
    /// 其他 URL
    Other,
}

impl HyperlinkType {
    /// 从URL 字符串推断类型
    pub fn from_url(url: &str) -> Self {
        let lower = url.to_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") {
            HyperlinkType::Http
        } else if lower.starts_with("ftp://") {
            HyperlinkType::Ftp
        } else if lower.starts_with("file://") {
            HyperlinkType::File
        } else if lower.starts_with("mailto:") || lower.contains('@') {
            HyperlinkType::Email
        } else {
            HyperlinkType::Other
        }
    }
}

// ============================================================================
// 超链接数据结构
// ============================================================================

/// 超链接
#[derive(Debug, Clone)]
pub struct Hyperlink {
    /// URL 字符串
    pub url: String,
    /// 超链接类型
    pub link_type: HyperlinkType,
    /// 起始行
    pub start_line: i32,
    /// 起始列
    pub start_col: i32,
    /// 结束行
    pub end_line: i32,
    /// 结束列
    pub end_col: i32,
}

impl Hyperlink {
    /// 创建新的超链接
    pub fn new(url: String, start_line: i32, start_col: i32, end_line: i32, end_col: i32) -> Self {
        let link_type = HyperlinkType::from_url(&url);
        Self {
            url,
            link_type,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// 检查指定位置是否在超链接范围内
    pub fn contains_point(&self, line: i32, col: i32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }

        if line == self.start_line && col < self.start_col {
            return false;
        }

        if line == self.end_line && col > self.end_col {
            return false;
        }

        true
    }

    /// 检查是否在指定行
    pub fn is_on_line(&self, line: i32) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    /// 获取指定行的范围
    pub fn get_line_range(&self, line: i32, max_col: i32) -> Option<(i32, i32)> {
        if !self.is_on_line(line) {
            return None;
        }

        let start = if line == self.start_line {
            self.start_col
        } else {
            0
        };

        let end = if line == self.end_line {
            self.end_col
        } else {
            max_col
        };

        Some((start, end))
    }
}

// ============================================================================
// 超链接配置
// ============================================================================

/// 超链接配置
#[derive(Debug, Clone)]
pub struct HyperlinkConfig {
    /// 是否启用超链接检测
    pub enabled: bool,
    /// 超链接下划线颜色
    pub underline_color: Hsla,
    /// 悬停时的下划线颜色
    pub hover_underline_color: Hsla,
    /// 是否显示悬停提示
    pub show_tooltip: bool,
}

impl Default for HyperlinkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            //蓝色下划线
            underline_color: Hsla {
                h: 210.0 / 360.0,
                s: 0.8,
                l: 0.6,
                a: 0.8,
            },
            // 悬停时更亮的蓝色
            hover_underline_color: Hsla {
                h: 210.0 / 360.0,
                s: 0.9,
                l: 0.7,
                a: 1.0,
            },
            show_tooltip: true,
        }
    }
}

// ============================================================================
// URL 检测
// ============================================================================

/// URL 检测模式
const URL_PATTERNS: &[&str] = &[
    // HTTP/HTTPS URLs
    "https?://[\\w\\-._~:/?#\\[\\]@!$&'()*+,;=%]+",
    // FTP URLs
    "ftp://[\\w\\-._~:/?#\\[\\]@!$&'()*+,;=%]+",
    // File URLs
    "file://[\\w\\-._~:/?#\\[\\]@!$&'()*+,;=%]+",
    // Email addresses
    "[\\w.+-]+@[\\w.-]+\\.[a-zA-Z]{2,}",
];

/// 简单的 URL 检测（不使用正则表达式）
///
/// 检测常见的 URL 模式：
/// - http:// 或 https:// 开头的URL
/// - ftp:// 开头的 URL
/// - file:// 开头的 URL
/// - 邮件地址（包含 @）
pub fn detect_urls_simple(text: &str, line: i32) -> Vec<Hyperlink> {
    let mut links = Vec::new();

    // 检测 http/https/ftp/file URLs
    for protocol in &["http://", "https://", "ftp://", "file://"] {
        let mut start = 0;
        while let Some(pos) = text[start..].find(protocol) {
            let abs_start = start + pos;
            let url_end = find_url_end(&text[abs_start..]);
            let url = &text[abs_start..abs_start + url_end];

            if is_valid_url(url) {
                links.push(Hyperlink::new(
                    url.to_string(),
                    line,
                    abs_start as i32,
                    line,
                    (abs_start + url_end - 1) as i32,
                ));
            }

            start = abs_start + url_end;
        }
    }

    // 检测邮件地址
    let mut chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' && i > 0 {
            // 向前查找用户名部分
            let mut user_start = i;
            while user_start > 0 && is_email_char(chars[user_start - 1]) {
                user_start -= 1;
            }

            // 向后查找域名部分
            let mut domain_end = i + 1;
            while domain_end < chars.len() && is_email_char(chars[domain_end]) {
                domain_end += 1;
            }

            // 验证邮件地址
            if user_start < i && domain_end > i + 1 {
                let email: String = chars[user_start..domain_end].iter().collect();
                if is_valid_email(&email) {
                    links.push(Hyperlink::new(
                        format!("mailto:{}", email),
                        line,
                        user_start as i32,
                        line,
                        (domain_end - 1) as i32,
                    ));
                }
            }

            i = domain_end;
        } else {
            i += 1;
        }
    }

    links
}

/// 查找 URL 结束位置
fn find_url_end(text: &str) -> usize {
    let mut end = 0;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;

    for (i, c) in text.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => {
                if paren_depth > 0 {
                    paren_depth -= 1;
                } else {
                    break;
                }
            },
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth > 0 {
                    bracket_depth -= 1;
                } else {
                    break;
                }
            },
            // URL 有效字符
            'a'..='z'
            | 'A'..='Z'
            | '0'..='9'
            | '-'
            | '.'
            | '_'
            | '~'
            | ':'
            | '/'
            | '?'
            | '#'
            | '@'
            | '!'
            | '$'
            | '&'
            | '\''
            | '*'
            | '+'
            | ','
            | ';'
            | '='
            | '%' => {},
            //遇到其他字符，结束
            _ => break,
        }
        end = i + c.len_utf8();
    }

    // 去除尾部标点
    while end > 0 {
        let last_char = text[..end].chars().last().unwrap();
        if matches!(last_char, '.' | ',' | ';' | ':' | '!' | '?') {
            end -= last_char.len_utf8();
        } else {
            break;
        }
    }

    end
}

/// 检查字符是否为邮件地址有效字符
fn is_email_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | '+')
}

/// 验证 URL 是否有效
fn is_valid_url(url: &str) -> bool {
    // 至少包含协议和一些内容
    if url.len() < 10 {
        return false;
    }

    // 检查是否包含域名或路径
    let after_protocol = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        return false;
    };

    // 域名至少包含一个点或者是 localhost
    after_protocol.contains('.') || after_protocol.starts_with("localhost")
}

/// 验证邮件地址是否有效
fn is_valid_email(email: &str) -> bool {
    // 基本格式检查
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let user = parts[0];
    let domain = parts[1];

    // 用户名和域名不能为空
    if user.is_empty() || domain.is_empty() {
        return false;
    }

    // 域名必须包含点
    if !domain.contains('.') {
        return false;
    }

    // 域名不能以点开头或结尾
    if domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }

    true
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http_url() {
        let text = "Visit https://example.com for more info";
        let links = detect_urls_simple(text, 0);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].link_type, HyperlinkType::Http);
    }

    #[test]
    fn test_detect_url_with_path() {
        let text = "Check https://example.com/path/to/page?query=1";
        let links = detect_urls_simple(text, 0);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/path/to/page?query=1");
    }

    #[test]
    fn test_detect_email() {
        let text = "Contact us at support@example.com";
        let links = detect_urls_simple(text, 0);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "mailto:support@example.com");
        assert_eq!(links[0].link_type, HyperlinkType::Email);
    }

    #[test]
    fn test_detect_multiple_urls() {
        let text = "Visit https://a.com and https://b.com";
        let links = detect_urls_simple(text, 0);

        assert_eq!(links.len(), 2);
    }

    #[test]
    fn test_url_with_trailing_punctuation() {
        let text = "See https://example.com.";
        let links = detect_urls_simple(text, 0);

        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
    }

    #[test]
    fn test_hyperlink_contains_point() {
        let link = Hyperlink::new("https://example.com".to_string(), 0, 5, 0, 23);

        assert!(link.contains_point(0, 5));
        assert!(link.contains_point(0, 15));
        assert!(link.contains_point(0, 23));
        assert!(!link.contains_point(0, 4));
        assert!(!link.contains_point(0, 24));
        assert!(!link.contains_point(1, 10));
    }

    #[test]
    fn test_hyperlink_type_detection() {
        assert_eq!(
            HyperlinkType::from_url("https://example.com"),
            HyperlinkType::Http
        );
        assert_eq!(
            HyperlinkType::from_url("ftp://files.example.com"),
            HyperlinkType::Ftp
        );
        assert_eq!(
            HyperlinkType::from_url("file:///home/user/file.txt"),
            HyperlinkType::File
        );
        assert_eq!(
            HyperlinkType::from_url("mailto:user@example.com"),
            HyperlinkType::Email
        );
    }
}
