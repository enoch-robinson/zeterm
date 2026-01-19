//! 应用配置模块
//!
//! 定义 Zeterm 的主配置结构和子配置。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 应用主配置
///
/// 包含所有配置项的顶层结构。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// 终端配置
    #[serde(default)]
    pub terminal: TerminalConfig,

    /// 外观配置
    #[serde(default)]
    pub appearance: AppearanceConfig,

    /// 网络配置
    #[serde(default)]
    pub network: NetworkConfig,

    /// 快捷键配置
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            terminal: TerminalConfig::default(),
            appearance: AppearanceConfig::default(),
            network: NetworkConfig::default(),
            keybindings: KeybindingsConfig::default(),
        }
    }
}

impl AppConfig {
    /// 从 TOML 字符串加载配置
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).context("解析配置文件失败")
    }

    /// 转换为 TOML 字符串
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("序列化配置失败")
    }

    /// 从文件加载配置
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {:?}", path))?;
        Self::from_toml(&content)
    }

    /// 保存配置到文件
    pub fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let toml_str = self.to_toml()?;

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {:?}", parent))?;
        }

        std::fs::write(path, toml_str).with_context(|| format!("写入配置文件失败: {:?}", path))?;

        Ok(())
    }

    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        self.terminal.validate()?;
        self.appearance.validate()?;
        self.network.validate()?;
        Ok(())
    }
}

/// 终端配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// 默认 Shell 程序
    #[serde(default = "default_shell")]
    pub shell: String,

    /// 字体名称
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// 字体大小
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// 滚动缓冲区大小（行数）
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: u32,

    /// 光标样式
    #[serde(default)]
    pub cursor_style: CursorStyle,

    /// 光标闪烁
    #[serde(default = "default_cursor_blink")]
    pub cursor_blink: bool,

    /// 启用鼠标支持
    #[serde(default = "default_true")]
    pub enable_mouse: bool,

    /// 启用剪贴板集成
    #[serde(default = "default_true")]
    pub enable_clipboard: bool,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell: default_shell(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            scrollback_lines: default_scrollback_lines(),
            cursor_style: CursorStyle::default(),
            cursor_blink: default_cursor_blink(),
            enable_mouse: true,
            enable_clipboard: true,
        }
    }
}

impl TerminalConfig {
    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        if self.font_size <= 0.0 {
            anyhow::bail!("字体大小必须大于 0");
        }
        if self.scrollback_lines == 0 {
            anyhow::bail!("滚动缓冲区大小必须大于 0");
        }
        Ok(())
    }
}

/// 光标样式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    /// 块状光标
    Block,
    /// 竖线光标
    Beam,
    /// 下划线光标
    Underline,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// 外观配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// 主题名称
    #[serde(default = "default_theme")]
    pub theme: String,

    /// 窗口透明度 (0.0 - 1.0)
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    /// 启用模糊背景
    #[serde(default)]
    pub blur_background: bool,

    /// 显示标签栏
    #[serde(default = "default_true")]
    pub show_tab_bar: bool,

    /// 显示状态栏
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            opacity: default_opacity(),
            blur_background: false,
            show_tab_bar: true,
            show_status_bar: true,
        }
    }
}

impl AppearanceConfig {
    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.opacity) {
            anyhow::bail!("窗口透明度必须在 0.0 到 1.0 之间");
        }
        Ok(())
    }
}

/// 网络配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// 连接超时时间（秒）
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u64,

    /// 保活间隔（秒）
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval: u64,

    /// 启用自动重连
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,

    /// 最大重连次数
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connect_timeout: default_connect_timeout(),
            keepalive_interval: default_keepalive_interval(),
            auto_reconnect: true,
            max_reconnect_attempts: default_max_reconnect_attempts(),
        }
    }
}

impl NetworkConfig {
    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        if self.connect_timeout == 0 {
            anyhow::bail!("连接超时时间必须大于 0");
        }
        if self.keepalive_interval == 0 {
            anyhow::bail!("保活间隔必须大于 0");
        }
        Ok(())
    }
}

/// 快捷键配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    /// 复制快捷键
    #[serde(default = "default_copy_key")]
    pub copy: String,

    /// 粘贴快捷键
    #[serde(default = "default_paste_key")]
    pub paste: String,

    /// 新建标签页快捷键
    #[serde(default = "default_new_tab_key")]
    pub new_tab: String,

    /// 关闭标签页快捷键
    #[serde(default = "default_close_tab_key")]
    pub close_tab: String,

    /// 搜索快捷键
    #[serde(default = "default_search_key")]
    pub search: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            copy: default_copy_key(),
            paste: default_paste_key(),
            new_tab: default_new_tab_key(),
            close_tab: default_close_tab_key(),
            search: default_search_key(),
        }
    }
}

// 默认值函数
fn default_shell() -> String {
    if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

fn default_font_family() -> String {
    "JetBrains Mono".to_string()
}

fn default_font_size() -> f32 {
    14.0
}

fn default_scrollback_lines() -> u32 {
    10000
}

fn default_cursor_blink() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_opacity() -> f32 {
    1.0
}

fn default_connect_timeout() -> u64 {
    30
}

fn default_keepalive_interval() -> u64 {
    60
}

fn default_max_reconnect_attempts() -> u32 {
    3
}

fn default_copy_key() -> String {
    "Ctrl+Shift+C".to_string()
}

fn default_paste_key() -> String {
    "Ctrl+Shift+V".to_string()
}

fn default_new_tab_key() -> String {
    "Ctrl+Shift+T".to_string()
}

fn default_close_tab_key() -> String {
    "Ctrl+Shift+W".to_string()
}

fn default_search_key() -> String {
    "Ctrl+Shift+F".to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.terminal.font_family, "JetBrains Mono");
        assert_eq!(config.terminal.font_size, 14.0);
        assert_eq!(config.appearance.theme, "dark");
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let toml_str = config.to_toml().unwrap();
        let deserialized = AppConfig::from_toml(&toml_str).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        config.terminal.font_size = 0.0;
        assert!(config.validate().is_err());

        config.terminal.font_size = 14.0;
        config.appearance.opacity = 1.5;
        assert!(config.validate().is_err());
    }
}
