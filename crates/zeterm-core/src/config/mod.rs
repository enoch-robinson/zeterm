//! 配置模块
//!
//! 提供应用配置的定义、加载和保存功能。

mod app_config;
mod hosts_config;
mod watcher;

pub use app_config::{
    AppConfig, AppearanceConfig, CursorStyle, KeybindingsConfig, NetworkConfig, TerminalConfig,
};

pub use hosts_config::{
    AuthType, GroupConfig, HostEntry, HostsConfig, HostsConfigManager, PasswordRef,
    generate_example_hosts_toml,
};

pub use watcher::{
    ConfigChangeCallback, ConfigChangeEvent, ConfigFileType, ConfigWatcher, ConfigWatcherConfig,
    ConfigWatcherError, ConfigWatcherResult, global_config_watcher, start_global_config_watcher,
    stop_global_config_watcher,
};

use anyhow::{Context, Result};
use std::path::PathBuf;

/// 获取默认配置目录路径
///
/// - Linux: `~/.config/zeterm/`
/// - macOS: `~/Library/Application Support/zeterm/`
/// - Windows: `%APPDATA%\zeterm\`
pub fn default_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("无法获取配置目录")?;
    Ok(config_dir.join("zeterm"))
}

/// 获取默认配置文件路径
///
/// 返回 `config.toml` 的完整路径。
pub fn default_config_path() -> Result<PathBuf> {
    Ok(default_config_dir()?.join("config.toml"))
}

/// 获取默认主机配置文件路径
///
/// 返回 `hosts.toml` 的完整路径。
pub fn default_hosts_path() -> Result<PathBuf> {
    Ok(default_config_dir()?.join("hosts.toml"))
}

/// 确保配置目录存在
pub fn ensure_config_dir() -> Result<PathBuf> {
    let config_dir = default_config_dir()?;
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .with_context(|| format!("创建配置目录失败: {:?}", config_dir))?;
    }
    Ok(config_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_dir() {
        let dir = default_config_dir().unwrap();
        assert!(dir.to_string_lossy().contains("zeterm"));
    }

    #[test]
    fn test_default_config_path() {
        let path = default_config_path().unwrap();
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_default_hosts_path() {
        let path = default_hosts_path().unwrap();
        assert!(path.to_string_lossy().ends_with("hosts.toml"));
    }
}
