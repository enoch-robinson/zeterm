//! 主机配置文件模块
//!
//! 管理 hosts.toml 文件的解析、保存和密码引用解析。

use crate::entities::{AuthConfig, HostConfig};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;

/// 主机配置文件
///
/// 对应 hosts.toml 文件的完整结构。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostsConfig {
    /// 主机列表
    #[serde(default)]
    pub hosts: Vec<HostEntry>,

    /// 分组配置（可选）
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub groups: HashMap<String, GroupConfig>,
}

/// 单个主机配置条目
///
/// TOML 中的 [[hosts]] 数组元素。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEntry {
    /// 主机名称（用户自定义的显示名称）
    pub name: String,

    /// 主机地址（IP 或域名）
    pub host: String,

    /// SSH 端口
    #[serde(default = "default_ssh_port")]
    pub port: u16,

    /// 用户名
    pub username: String,

    /// 认证方式
    #[serde(default = "default_auth_type")]
    pub auth_type: AuthType,

    /// 密码引用（格式：keychain:xxx 或 env:XXX 或 plain:xxx）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_ref: Option<String>,

    /// 私钥文件路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_path: Option<PathBuf>,

    /// 私钥密码引用（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase_ref: Option<String>,

    /// 分组（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// 标签（可选）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// 描述（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 认证类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    /// 密码认证
    Password,
    /// 公钥认证
    PublicKey,
    /// SSH Agent 认证
    Agent,
}

impl Default for AuthType {
    fn default() -> Self {
        Self::Password
    }
}

/// 分组配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupConfig {
    /// 分组显示名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// 分组描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 分组颜色（十六进制，如 "#FF5733"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,

    /// 分组图标
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// 排序优先级（数字越小越靠前）
    #[serde(default)]
    pub order: i32,
}

impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            display_name: None,
            description: None,
            color: None,
            icon: None,
            order: 0,
        }
    }
}

/// 密码引用类型
#[derive(Debug, Clone, PartialEq)]
pub enum PasswordRef {
    /// Keychain/Credential Manager 引用
    /// 格式：keychain:service_name
    Keychain(String),

    /// 环境变量引用
    /// 格式：env:VAR_NAME
    Env(String),

    /// 明文密码（不推荐，仅用于测试）
    /// 格式：plain:password
    Plain(String),

    /// 空引用（无密码）
    None,
}

/// 密码引用解析错误
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum PasswordRefParseError {
    /// 空字符串
    #[error("密码引用字符串为空")]
    Empty,

    /// 无效的前缀
    #[error("无效的密码引用前缀: '{0}'，支持的格式: keychain:, env:, plain:")]
    InvalidPrefix(String),

    /// 值部分为空
    #[error("密码引用值部分为空")]
    EmptyValue,

    /// 环境变量名称无效
    #[error("环境变量名称 '{0}' 无效: {1}")]
    InvalidEnvVar(String, String),

    /// Keychain key 无效
    #[error("Keychain key '{0}' 无效: {1}")]
    InvalidKeychainKey(String, String),
}

impl PasswordRef {
    /// 解析密码引用字符串
    ///
    /// 支持的格式：
    /// - `keychain:service_name` - 从系统密钥链读取
    /// - `env:VAR_NAME` - 从环境变量读取
    /// - `plain:password` - 明文密码（不推荐）
    ///
    /// 注意：此方法向后兼容，会将无效格式静默转为 Keychain 类型。
    /// 建议使用 `parse_validated` 获取详细的错误信息。
    pub fn parse(s: &str) -> Self {
        Self::parse_validated(s).unwrap_or_else(|_| Self::Keychain(s.to_string()))
    }

    /// 解析密码引用字符串（带验证）
    ///
    /// 支持的格式：
    /// - `keychain:service_name` - 从系统密钥链读取（验证 key 不为空且只包含有效字符）
    /// - `env:VAR_NAME` - 从环境变量读取（验证变量名符合 POSIX 标准）
    /// - `plain:password` - 明文密码（不推荐）
    /// - 空字符串返回 `PasswordRef::None`
    ///
    /// # 错误
    ///
    /// - `PasswordRefParseError::Empty` - 输入字符串为空
    /// - `PasswordRefParseError::InvalidPrefix` - 使用了不支持的前缀
    /// - `PasswordRefParseError::EmptyValue` - 值部分为空
    /// - `PasswordRefParseError::InvalidEnvVar` - 环境变量名称无效
    /// - `PasswordRefParseError::InvalidKeychainKey` - Keychain key 无效
    pub fn parse_validated(s: &str) -> Result<Self, PasswordRefParseError> {
        if s.is_empty() {
            return Ok(Self::None);
        }

        if let Some(key) = s.strip_prefix("keychain:") {
            if key.is_empty() {
                return Err(PasswordRefParseError::EmptyValue);
            }
            // 验证 key 只包含有效字符（字母、数字、下划线、连字符、点）
            if !key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
            {
                return Err(PasswordRefParseError::InvalidKeychainKey(
                    key.to_string(),
                    "只能包含字母、数字、下划线、连字符和点".to_string(),
                ));
            }
            Ok(Self::Keychain(key.to_string()))
        } else if let Some(var) = s.strip_prefix("env:") {
            if var.is_empty() {
                return Err(PasswordRefParseError::EmptyValue);
            }
            // 验证环境变量名符合 POSIX 标准（字母开头，只包含字母、数字、下划线）
            if !var
                .chars()
                .next()
                .map(|c| c.is_alphabetic() || c == '_')
                .unwrap_or(false)
            {
                return Err(PasswordRefParseError::InvalidEnvVar(
                    var.to_string(),
                    "必须以字母或下划线开头".to_string(),
                ));
            }
            if !var.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(PasswordRefParseError::InvalidEnvVar(
                    var.to_string(),
                    "只能包含字母、数字和下划线".to_string(),
                ));
            }
            Ok(Self::Env(var.to_string()))
        } else if let Some(plain) = s.strip_prefix("plain:") {
            if plain.is_empty() {
                return Err(PasswordRefParseError::EmptyValue);
            }
            warn!("使用明文密码（不推荐）");
            Ok(Self::Plain(plain.to_string()))
        } else {
            // 无效的前缀
            if s.contains(':') {
                let prefix = s.split(':').next().unwrap_or("");
                Err(PasswordRefParseError::InvalidPrefix(prefix.to_string()))
            } else {
                // 无前缀的字符串，不允许在新代码中直接使用
                Err(PasswordRefParseError::InvalidPrefix(s.to_string()))
            }
        }
    }

    /// 转换为引用字符串
    pub fn to_ref_string(&self) -> Option<String> {
        match self {
            Self::Keychain(key) => Some(format!("keychain:{}", key)),
            Self::Env(var) => Some(format!("env:{}", var)),
            Self::Plain(plain) => Some(format!("plain:{}", plain)),
            Self::None => None,
        }
    }

    /// 尝试解析密码值
    ///
    /// 注意：Keychain 类型需要外部 SecretStore 实现，此处返回 None
    pub fn resolve(&self) -> Option<String> {
        match self {
            Self::Env(var) => std::env::var(var).ok(),
            Self::Plain(plain) => Some(plain.clone()),
            Self::Keychain(_) => None, // 需要 SecretStore 实现
            Self::None => None,
        }
    }

    /// 检查是否为 Keychain 类型
    pub fn is_keychain(&self) -> bool {
        matches!(self, Self::Keychain(_))
    }

    /// 获取 Keychain 键名
    pub fn keychain_key(&self) -> Option<&str> {
        match self {
            Self::Keychain(key) => Some(key),
            _ => None,
        }
    }
}

fn default_ssh_port() -> u16 {
    22
}

fn default_auth_type() -> AuthType {
    AuthType::Password
}

impl HostsConfig {
    /// 创建空的主机配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 TOML 字符串加载配置
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).context("解析 hosts.toml 失败")
    }

    /// 转换为 TOML 字符串
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("序列化 hosts.toml 失败")
    }

    /// 从文件加载配置
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            // 文件不存在时返回空配置
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取 hosts.toml 失败: {:?}", path))?;

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

        std::fs::write(path, toml_str)
            .with_context(|| format!("写入 hosts.toml 失败: {:?}", path))?;

        Ok(())
    }

    /// 验证所有主机配置
    pub fn validate(&self) -> Result<()> {
        for (i, entry) in self.hosts.iter().enumerate() {
            entry
                .validate()
                .with_context(|| format!("主机配置 #{} ({}) 验证失败", i + 1, entry.name))?;
        }
        Ok(())
    }

    /// 添加主机
    pub fn add_host(&mut self, entry: HostEntry) {
        self.hosts.push(entry);
    }

    /// 移除主机（按名称）
    pub fn remove_host(&mut self, name: &str) -> Option<HostEntry> {
        if let Some(pos) = self.hosts.iter().position(|h| h.name == name) {
            Some(self.hosts.remove(pos))
        } else {
            None
        }
    }

    /// 查找主机（按名称）
    pub fn find_host(&self, name: &str) -> Option<&HostEntry> {
        self.hosts.iter().find(|h| h.name == name)
    }

    /// 查找主机（可变引用，按名称）
    pub fn find_host_mut(&mut self, name: &str) -> Option<&mut HostEntry> {
        self.hosts.iter_mut().find(|h| h.name == name)
    }

    /// 获取指定分组的主机
    pub fn hosts_in_group(&self, group: &str) -> Vec<&HostEntry> {
        self.hosts
            .iter()
            .filter(|h| h.group.as_deref() == Some(group))
            .collect()
    }

    /// 获取未分组的主机
    pub fn ungrouped_hosts(&self) -> Vec<&HostEntry> {
        self.hosts.iter().filter(|h| h.group.is_none()).collect()
    }

    /// 获取所有分组名称
    pub fn group_names(&self) -> Vec<String> {
        let mut groups: Vec<String> = self.hosts.iter().filter_map(|h| h.group.clone()).collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// 转换为 HostConfig 列表
    pub fn to_host_configs(&self) -> Vec<HostConfig> {
        self.hosts.iter().map(|e| e.to_host_config()).collect()
    }

    /// 从 HostConfig 列表创建
    pub fn from_host_configs(configs: &[HostConfig]) -> Self {
        let hosts = configs.iter().map(HostEntry::from_host_config).collect();
        Self {
            hosts,
            groups: HashMap::new(),
        }
    }

    /// 合并另一个配置（用于导入）
    pub fn merge(&mut self, other: HostsConfig) {
        for host in other.hosts {
            // 跳过同名主机
            if !self.hosts.iter().any(|h| h.name == host.name) {
                self.hosts.push(host);
            }
        }

        // 合并分组配置
        for (name, config) in other.groups {
            self.groups.entry(name).or_insert(config);
        }
    }
}

impl HostEntry {
    /// 创建新的主机条目
    pub fn new(name: String, host: String, username: String) -> Self {
        Self {
            name,
            host,
            port: 22,
            username,
            auth_type: AuthType::Password,
            password_ref: None,
            key_path: None,
            passphrase_ref: None,
            group: None,
            tags: Vec::new(),
            description: None,
        }
    }

    /// 设置密码认证
    pub fn with_password(mut self, password_ref: String) -> Self {
        self.auth_type = AuthType::Password;
        self.password_ref = Some(password_ref);
        self
    }

    /// 设置公钥认证
    pub fn with_public_key(mut self, key_path: PathBuf, passphrase_ref: Option<String>) -> Self {
        self.auth_type = AuthType::PublicKey;
        self.key_path = Some(key_path);
        self.passphrase_ref = passphrase_ref;
        self
    }

    /// 设置 Agent 认证
    pub fn with_agent(mut self) -> Self {
        self.auth_type = AuthType::Agent;
        self
    }

    /// 设置分组
    pub fn with_group(mut self, group: String) -> Self {
        self.group = Some(group);
        self
    }

    /// 设置标签
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// 设置描述
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("主机名称不能为空");
        }
        if self.host.trim().is_empty() {
            bail!("主机地址不能为空");
        }
        if self.port == 0 {
            bail!("端口号无效");
        }
        if self.username.trim().is_empty() {
            bail!("用户名不能为空");
        }

        match self.auth_type {
            AuthType::Password => {
                if self.password_ref.is_none() {
                    bail!("密码认证需要提供密码引用");
                }
            },
            AuthType::PublicKey => {
                if self.key_path.is_none() {
                    bail!("公钥认证需要提供私钥路径");
                }
            },
            AuthType::Agent => {
                // Agent 认证无需额外验证
            },
        }

        Ok(())
    }

    /// 转换为 HostConfig
    pub fn to_host_config(&self) -> HostConfig {
        let auth_config = match self.auth_type {
            AuthType::Password => {
                AuthConfig::password(self.password_ref.clone().unwrap_or_default())
            },
            AuthType::PublicKey => AuthConfig::public_key(
                self.key_path.clone().unwrap_or_default(),
                self.passphrase_ref.clone(),
            ),
            AuthType::Agent => AuthConfig::agent(),
        };

        let mut config = HostConfig::new(
            self.name.clone(),
            self.host.clone(),
            self.username.clone(),
            auth_config,
        );

        config.port = self.port;
        config.group = self.group.clone();
        config.tags = self.tags.clone();
        config.description = self.description.clone();

        config
    }

    /// 从 HostConfig 创建
    pub fn from_host_config(config: &HostConfig) -> Self {
        let (auth_type, password_ref, key_path, passphrase_ref) = match &config.auth_config {
            AuthConfig::Password { password_ref } => {
                (AuthType::Password, Some(password_ref.clone()), None, None)
            },
            AuthConfig::PublicKey {
                key_path,
                passphrase_ref,
            } => (
                AuthType::PublicKey,
                None,
                Some(key_path.clone()),
                passphrase_ref.clone(),
            ),
            AuthConfig::Agent => (AuthType::Agent, None, None, None),
        };

        Self {
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port,
            username: config.username.clone(),
            auth_type,
            password_ref,
            key_path,
            passphrase_ref,
            group: config.group.clone(),
            tags: config.tags.clone(),
            description: config.description.clone(),
        }
    }

    /// 获取密码引用解析器
    pub fn password_reference(&self) -> PasswordRef {
        self.password_ref
            .as_ref()
            .map(|s| PasswordRef::parse(s))
            .unwrap_or(PasswordRef::None)
    }

    /// 获取私钥密码引用解析器
    pub fn passphrase_reference(&self) -> PasswordRef {
        self.passphrase_ref
            .as_ref()
            .map(|s| PasswordRef::parse(s))
            .unwrap_or(PasswordRef::None)
    }

    /// 生成 Keychain 键名
    ///
    /// 格式：zeterm:host:username@host:port
    pub fn generate_keychain_key(&self) -> String {
        format!("zeterm:host:{}@{}:{}", self.username, self.host, self.port)
    }
}

/// 主机配置管理器
///
/// 提供高层次的主机配置管理 API，包括加载、保存、同步等功能。
#[derive(Debug)]
pub struct HostsConfigManager {
    /// 配置文件路径
    config_path: PathBuf,

    /// 当前配置
    config: HostsConfig,

    /// 是否有未保存的修改
    dirty: bool,
}

impl HostsConfigManager {
    /// 创建新的管理器（使用默认路径）
    pub fn new() -> Result<Self> {
        let config_path = super::default_hosts_path()?;
        Self::with_path(config_path)
    }

    /// 使用指定路径创建管理器
    pub fn with_path(config_path: PathBuf) -> Result<Self> {
        let config = HostsConfig::load_from_file(&config_path)?;
        Ok(Self {
            config_path,
            config,
            dirty: false,
        })
    }

    /// 创建空的管理器（不从文件加载）
    pub fn empty(config_path: PathBuf) -> Self {
        Self {
            config_path,
            config: HostsConfig::new(),
            dirty: false,
        }
    }

    /// 获取配置文件路径
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// 获取当前配置的引用
    pub fn config(&self) -> &HostsConfig {
        &self.config
    }

    /// 获取当前配置的可变引用
    pub fn config_mut(&mut self) -> &mut HostsConfig {
        self.dirty = true;
        &mut self.config
    }

    /// 是否有未保存的修改
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 重新加载配置文件
    pub fn reload(&mut self) -> Result<()> {
        self.config = HostsConfig::load_from_file(&self.config_path)?;
        self.dirty = false;
        Ok(())
    }

    /// 保存配置到文件
    pub fn save(&mut self) -> Result<()> {
        self.config.save_to_file(&self.config_path)?;
        self.dirty = false;
        Ok(())
    }

    /// 如果有修改则保存
    pub fn save_if_dirty(&mut self) -> Result<bool> {
        if self.dirty {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 获取所有主机
    pub fn hosts(&self) -> &[HostEntry] {
        &self.config.hosts
    }

    /// 获取主机数量
    pub fn host_count(&self) -> usize {
        self.config.hosts.len()
    }

    /// 添加主机
    pub fn add_host(&mut self, entry: HostEntry) -> Result<()> {
        entry.validate()?;

        // 检查名称是否重复
        if self.config.find_host(&entry.name).is_some() {
            bail!("主机名称 '{}' 已存在", entry.name);
        }

        self.config.add_host(entry);
        self.dirty = true;
        Ok(())
    }

    /// 更新主机（按名称）
    pub fn update_host(&mut self, name: &str, entry: HostEntry) -> Result<()> {
        entry.validate()?;

        // 如果名称改变，检查新名称是否重复
        if name != entry.name && self.config.find_host(&entry.name).is_some() {
            bail!("主机名称 '{}' 已存在", entry.name);
        }

        // 移除旧的，添加新的
        if self.config.remove_host(name).is_none() {
            bail!("主机 '{}' 不存在", name);
        }

        self.config.add_host(entry);
        self.dirty = true;
        Ok(())
    }

    /// 移除主机（按名称）
    pub fn remove_host(&mut self, name: &str) -> Result<HostEntry> {
        match self.config.remove_host(name) {
            Some(entry) => {
                self.dirty = true;
                Ok(entry)
            },
            None => bail!("主机 '{}' 不存在", name),
        }
    }

    /// 查找主机（按名称）
    pub fn find_host(&self, name: &str) -> Option<&HostEntry> {
        self.config.find_host(name)
    }

    /// 获取指定分组的主机
    pub fn hosts_in_group(&self, group: &str) -> Vec<&HostEntry> {
        self.config.hosts_in_group(group)
    }

    /// 获取未分组的主机
    pub fn ungrouped_hosts(&self) -> Vec<&HostEntry> {
        self.config.ungrouped_hosts()
    }

    /// 获取所有分组名称
    pub fn group_names(&self) -> Vec<String> {
        self.config.group_names()
    }

    /// 导出为 HostConfig 列表
    pub fn export_host_configs(&self) -> Vec<HostConfig> {
        self.config.to_host_configs()
    }

    /// 从 HostConfig 列表导入
    pub fn import_host_configs(&mut self, configs: &[HostConfig]) {
        let imported = HostsConfig::from_host_configs(configs);
        self.config.merge(imported);
        self.dirty = true;
    }

    /// 从另一个 hosts.toml 文件导入
    pub fn import_from_file(&mut self, path: &PathBuf) -> Result<usize> {
        let imported = HostsConfig::load_from_file(path)?;
        let count_before = self.config.hosts.len();
        self.config.merge(imported);
        let count_after = self.config.hosts.len();
        let added = count_after - count_before;

        if added > 0 {
            self.dirty = true;
        }

        Ok(added)
    }

    /// 生成并写入示例配置文件
    pub fn write_example_config(&self) -> Result<()> {
        let example = generate_example_hosts_toml();

        // 确保父目录存在
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {:?}", parent))?;
        }

        std::fs::write(&self.config_path, example)
            .with_context(|| format!("写入示例配置失败: {:?}", self.config_path))?;

        Ok(())
    }

    /// 检查配置文件是否存在
    pub fn config_exists(&self) -> bool {
        self.config_path.exists()
    }

    /// 验证所有主机配置
    pub fn validate(&self) -> Result<()> {
        self.config.validate()
    }
}

impl Default for HostsConfigManager {
    fn default() -> Self {
        Self::empty(PathBuf::from("hosts.toml"))
    }
}

/// 生成示例 hosts.toml 内容
pub fn generate_example_hosts_toml() -> String {
    r##"# Zeterm 主机配置文件
# 文档：https://github.com/user/zeterm/docs/hosts-config.md

# 分组配置（可选）
[groups.production]
display_name = "生产环境"
description = "生产服务器"
color = "#FF5733"
order = 1

[groups.development]
display_name = "开发环境"
description = "开发测试服务器"
color = "#33FF57"
order = 2

# 主机列表
# 密码认证示例
[[hosts]]
name = "生产服务器 1"
host = "192.168.1.100"
port = 22
username = "admin"
auth_type = "password"
password_ref = "keychain:prod-server-1"  # 从系统密钥链读取
group = "production"
tags = ["web", "nginx"]
description = "主 Web 服务器"

# 公钥认证示例
[[hosts]]
name = "开发服务器"
host = "dev.example.com"
port = 22
username = "developer"
auth_type = "public_key"
key_path = "~/.ssh/id_ed25519"
# passphrase_ref = "keychain:dev-key-passphrase"  # 如果私钥有密码
group = "development"
tags = ["dev", "docker"]

# SSH Agent 认证示例
[[hosts]]
name = "测试服务器"
host = "test.example.com"
port = 22
username = "tester"
auth_type = "agent"
description = "使用 SSH Agent 认证"

# 环境变量密码示例（适合 CI/CD）
[[hosts]]
name = "CI 服务器"
host = "ci.example.com"
port = 22
username = "ci"
auth_type = "password"
password_ref = "env:CI_SSH_PASSWORD"  # 从环境变量读取
tags = ["ci", "automation"]
"##
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hosts_config_new() {
        let config = HostsConfig::new();
        assert!(config.hosts.is_empty());
        assert!(config.groups.is_empty());
    }

    #[test]
    fn test_host_entry_new() {
        let entry = HostEntry::new(
            "测试服务器".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
        );

        assert_eq!(entry.name, "测试服务器");
        assert_eq!(entry.host, "192.168.1.100");
        assert_eq!(entry.port, 22);
        assert_eq!(entry.username, "root");
        assert_eq!(entry.auth_type, AuthType::Password);
    }

    #[test]
    fn test_host_entry_with_password() {
        let entry = HostEntry::new("test".to_string(), "host".to_string(), "user".to_string())
            .with_password("keychain:test".to_string());

        assert_eq!(entry.auth_type, AuthType::Password);
        assert_eq!(entry.password_ref, Some("keychain:test".to_string()));
    }

    #[test]
    fn test_host_entry_with_public_key() {
        let entry = HostEntry::new("test".to_string(), "host".to_string(), "user".to_string())
            .with_public_key(
                PathBuf::from("/home/user/.ssh/id_rsa"),
                Some("keychain:passphrase".to_string()),
            );

        assert_eq!(entry.auth_type, AuthType::PublicKey);
        assert_eq!(
            entry.key_path,
            Some(PathBuf::from("/home/user/.ssh/id_rsa"))
        );
        assert_eq!(
            entry.passphrase_ref,
            Some("keychain:passphrase".to_string())
        );
    }

    #[test]
    fn test_host_entry_with_agent() {
        let entry =
            HostEntry::new("test".to_string(), "host".to_string(), "user".to_string()).with_agent();

        assert_eq!(entry.auth_type, AuthType::Agent);
    }

    #[test]
    fn test_password_ref_parse_keychain() {
        let pr = PasswordRef::parse("keychain:my-password");
        assert!(matches!(pr, PasswordRef::Keychain(key) if key == "my-password"));
    }

    #[test]
    fn test_password_ref_parse_env() {
        let pr = PasswordRef::parse("env:MY_PASSWORD");
        assert!(matches!(pr, PasswordRef::Env(var) if var == "MY_PASSWORD"));
    }

    #[test]
    fn test_password_ref_parse_plain() {
        let pr = PasswordRef::parse("plain:secret123");
        assert!(matches!(pr, PasswordRef::Plain(p) if p == "secret123"));
    }

    #[test]
    fn test_password_ref_parse_default() {
        let pr = PasswordRef::parse("some-key");
        assert!(matches!(pr, PasswordRef::Keychain(key) if key == "some-key"));
    }

    #[test]
    fn test_password_ref_parse_empty() {
        let pr = PasswordRef::parse("");
        assert!(matches!(pr, PasswordRef::None));
    }

    #[test]
    fn test_password_ref_to_string() {
        assert_eq!(
            PasswordRef::Keychain("key".to_string()).to_ref_string(),
            Some("keychain:key".to_string())
        );
        assert_eq!(
            PasswordRef::Env("VAR".to_string()).to_ref_string(),
            Some("env:VAR".to_string())
        );
        assert_eq!(
            PasswordRef::Plain("pass".to_string()).to_ref_string(),
            Some("plain:pass".to_string())
        );
        assert_eq!(PasswordRef::None.to_ref_string(), None);
    }

    #[test]
    fn test_password_ref_resolve_env() {
        // SAFETY: This test runs in a single thread and the env var is local to this test
        unsafe {
            std::env::set_var("TEST_ZETERM_PASSWORD", "test_value");
        }
        let pr = PasswordRef::Env("TEST_ZETERM_PASSWORD".to_string());
        assert_eq!(pr.resolve(), Some("test_value".to_string()));
        // SAFETY: Cleanup of test env var
        unsafe {
            std::env::remove_var("TEST_ZETERM_PASSWORD");
        }
    }

    #[test]
    fn test_password_ref_resolve_plain() {
        let pr = PasswordRef::Plain("my_secret".to_string());
        assert_eq!(pr.resolve(), Some("my_secret".to_string()));
    }

    #[test]
    fn test_hosts_config_serialization() {
        let mut config = HostsConfig::new();
        config.add_host(
            HostEntry::new(
                "测试服务器".to_string(),
                "192.168.1.100".to_string(),
                "root".to_string(),
            )
            .with_password("keychain:test".to_string())
            .with_group("production".to_string()),
        );

        let toml_str = config.to_toml().unwrap();
        let deserialized = HostsConfig::from_toml(&toml_str).unwrap();

        assert_eq!(config.hosts.len(), deserialized.hosts.len());
        assert_eq!(config.hosts[0].name, deserialized.hosts[0].name);
    }

    #[test]
    fn test_hosts_config_manager_basic() {
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("zeterm_test_hosts.toml");

        // 清理可能存在的测试文件
        let _ = std::fs::remove_file(&config_path);

        // 创建空管理器
        let mut manager = HostsConfigManager::empty(config_path.clone());
        assert_eq!(manager.host_count(), 0);
        assert!(!manager.is_dirty());

        // 添加主机
        let entry = HostEntry::new(
            "server1".to_string(),
            "host1".to_string(),
            "user1".to_string(),
        )
        .with_password("keychain:s1".to_string());
        manager.add_host(entry).unwrap();

        assert_eq!(manager.host_count(), 1);
        assert!(manager.is_dirty());

        // 保存
        manager.save().unwrap();
        assert!(!manager.is_dirty());
        assert!(config_path.exists());

        // 重新加载
        let manager2 = HostsConfigManager::with_path(config_path.clone()).unwrap();
        assert_eq!(manager2.host_count(), 1);
        assert_eq!(manager2.find_host("server1").unwrap().host, "host1");

        // 清理
        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_hosts_config_manager_duplicate_name() {
        let mut manager = HostsConfigManager::default();

        let entry1 = HostEntry::new(
            "server".to_string(),
            "host1".to_string(),
            "user1".to_string(),
        )
        .with_agent();
        manager.add_host(entry1).unwrap();

        let entry2 = HostEntry::new(
            "server".to_string(),
            "host2".to_string(),
            "user2".to_string(),
        )
        .with_agent();
        assert!(manager.add_host(entry2).is_err());
    }

    #[test]
    fn test_hosts_config_manager_update_remove() {
        let mut manager = HostsConfigManager::default();

        let entry = HostEntry::new(
            "server1".to_string(),
            "host1".to_string(),
            "user1".to_string(),
        )
        .with_agent();
        manager.add_host(entry).unwrap();

        // 更新
        let updated = HostEntry::new(
            "server1".to_string(),
            "host1-new".to_string(),
            "user1".to_string(),
        )
        .with_agent();
        manager.update_host("server1", updated).unwrap();
        assert_eq!(manager.find_host("server1").unwrap().host, "host1-new");

        // 移除
        let removed = manager.remove_host("server1").unwrap();
        assert_eq!(removed.name, "server1");
        assert_eq!(manager.host_count(), 0);
    }

    #[test]
    fn test_hosts_config_find_host() {
        let mut config = HostsConfig::new();
        config.add_host(HostEntry::new(
            "server1".to_string(),
            "host1".to_string(),
            "user1".to_string(),
        ));
        config.add_host(HostEntry::new(
            "server2".to_string(),
            "host2".to_string(),
            "user2".to_string(),
        ));

        assert!(config.find_host("server1").is_some());
        assert!(config.find_host("server2").is_some());
        assert!(config.find_host("server3").is_none());
    }

    #[test]
    fn test_hosts_config_remove_host() {
        let mut config = HostsConfig::new();
        config.add_host(HostEntry::new(
            "server1".to_string(),
            "host1".to_string(),
            "user1".to_string(),
        ));

        let removed = config.remove_host("server1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "server1");
        assert!(config.hosts.is_empty());
    }

    #[test]
    fn test_hosts_config_group_operations() {
        let mut config = HostsConfig::new();
        config.add_host(
            HostEntry::new("s1".to_string(), "h1".to_string(), "u1".to_string())
                .with_group("prod".to_string()),
        );
        config.add_host(
            HostEntry::new("s2".to_string(), "h2".to_string(), "u2".to_string())
                .with_group("prod".to_string()),
        );
        config.add_host(HostEntry::new(
            "s3".to_string(),
            "h3".to_string(),
            "u3".to_string(),
        ));

        assert_eq!(config.hosts_in_group("prod").len(), 2);
        assert_eq!(config.ungrouped_hosts().len(), 1);
        assert_eq!(config.group_names(), vec!["prod".to_string()]);
    }

    #[test]
    fn test_host_entry_validate() {
        // 有效的密码认证
        let entry = HostEntry::new("test".to_string(), "host".to_string(), "user".to_string())
            .with_password("keychain:test".to_string());
        assert!(entry.validate().is_ok());

        // 无效：空名称
        let invalid = HostEntry::new("".to_string(), "host".to_string(), "user".to_string());
        assert!(invalid.validate().is_err());

        // 无效：密码认证但无密码引用
        let invalid = HostEntry::new("test".to_string(), "host".to_string(), "user".to_string());
        assert!(invalid.validate().is_err());

        // 有效的 Agent 认证
        let agent =
            HostEntry::new("test".to_string(), "host".to_string(), "user".to_string()).with_agent();
        assert!(agent.validate().is_ok());
    }

    #[test]
    fn test_host_entry_to_host_config() {
        let entry = HostEntry::new("test".to_string(), "host".to_string(), "user".to_string())
            .with_password("keychain:test".to_string())
            .with_group("prod".to_string())
            .with_tags(vec!["web".to_string()]);

        let config = entry.to_host_config();
        assert_eq!(config.name, "test");
        assert_eq!(config.host, "host");
        assert_eq!(config.username, "user");
        assert_eq!(config.group, Some("prod".to_string()));
        assert_eq!(config.tags, vec!["web".to_string()]);
    }

    #[test]
    fn test_host_entry_from_host_config() {
        let config = HostConfig::new(
            "test".to_string(),
            "host".to_string(),
            "user".to_string(),
            AuthConfig::password("keychain:test".to_string()),
        )
        .with_group("prod".to_string());

        let entry = HostEntry::from_host_config(&config);
        assert_eq!(entry.name, "test");
        assert_eq!(entry.auth_type, AuthType::Password);
        assert_eq!(entry.password_ref, Some("keychain:test".to_string()));
        assert_eq!(entry.group, Some("prod".to_string()));
    }

    #[test]
    fn test_generate_example_hosts_toml() {
        let example = generate_example_hosts_toml();
        assert!(example.contains("[[hosts]]"));
        assert!(example.contains("keychain:"));
        assert!(example.contains("auth_type"));

        // 验证示例可以被解析
        let config = HostsConfig::from_toml(&example).unwrap();
        assert!(!config.hosts.is_empty());
    }

    #[test]
    fn test_hosts_config_merge() {
        let mut config1 = HostsConfig::new();
        config1.add_host(
            HostEntry::new(
                "server1".to_string(),
                "host1".to_string(),
                "user1".to_string(),
            )
            .with_password("keychain:s1".to_string()),
        );

        let mut config2 = HostsConfig::new();
        config2.add_host(
            HostEntry::new(
                "server1".to_string(), // 同名，应被跳过
                "host1-new".to_string(),
                "user1".to_string(),
            )
            .with_password("keychain:s1".to_string()),
        );
        config2.add_host(
            HostEntry::new(
                "server2".to_string(),
                "host2".to_string(),
                "user2".to_string(),
            )
            .with_password("keychain:s2".to_string()),
        );

        config1.merge(config2);

        assert_eq!(config1.hosts.len(), 2);
        // server1 保持原值
        assert_eq!(config1.find_host("server1").unwrap().host, "host1");
        // server2 被添加
        assert!(config1.find_host("server2").is_some());
    }

    #[test]
    fn test_hosts_config_to_from_host_configs() {
        let mut config = HostsConfig::new();
        config.add_host(
            HostEntry::new("s1".to_string(), "h1".to_string(), "u1".to_string())
                .with_password("keychain:p1".to_string()),
        );
        config.add_host(
            HostEntry::new("s2".to_string(), "h2".to_string(), "u2".to_string()).with_agent(),
        );

        let host_configs = config.to_host_configs();
        assert_eq!(host_configs.len(), 2);

        let restored = HostsConfig::from_host_configs(&host_configs);
        assert_eq!(restored.hosts.len(), 2);
        assert_eq!(restored.hosts[0].name, "s1");
        assert_eq!(restored.hosts[1].name, "s2");
    }

    #[test]
    fn test_host_entry_generate_keychain_key() {
        let entry = HostEntry::new(
            "test".to_string(),
            "192.168.1.100".to_string(),
            "admin".to_string(),
        );
        let key = entry.generate_keychain_key();
        assert_eq!(key, "zeterm:host:admin@192.168.1.100:22");
    }

    #[test]
    fn test_group_config_default() {
        let group = GroupConfig::default();
        assert!(group.display_name.is_none());
        assert!(group.color.is_none());
        assert_eq!(group.order, 0);
    }

    #[test]
    fn test_hosts_config_with_groups() {
        let toml = r##"
[groups.production]
display_name = "生产环境"
color = "#FF0000"
order = 1

[[hosts]]
name = "server1"
host = "192.168.1.1"
username = "root"
auth_type = "agent"
group = "production"
"##;

        let config = HostsConfig::from_toml(toml).unwrap();
        assert_eq!(config.groups.len(), 1);
        assert!(config.groups.contains_key("production"));
        assert_eq!(
            config.groups["production"].display_name,
            Some("生产环境".to_string())
        );
        assert_eq!(config.hosts.len(), 1);
        assert_eq!(config.hosts[0].group, Some("production".to_string()));
    }

    #[test]
    fn test_password_ref_parse_validated_empty() {
        let result = PasswordRef::parse_validated("");
        assert!(matches!(result, Ok(PasswordRef::None)));
    }

    #[test]
    fn test_password_ref_parse_validated_keychain_valid() {
        let result = PasswordRef::parse_validated("keychain:my-service");
        assert!(matches!(result, Ok(PasswordRef::Keychain(key)) if key == "my-service"));
    }

    #[test]
    fn test_password_ref_parse_validated_keychain_with_dashes() {
        let result = PasswordRef::parse_validated("keychain:my-service-name");
        assert!(matches!(result, Ok(PasswordRef::Keychain(key)) if key == "my-service-name"));
    }

    #[test]
    fn test_password_ref_parse_validated_keychain_with_dots() {
        let result = PasswordRef::parse_validated("keychain:my.service.name");
        assert!(matches!(result, Ok(PasswordRef::Keychain(key)) if key == "my.service.name"));
    }

    #[test]
    fn test_password_ref_parse_validated_keychain_empty() {
        let result = PasswordRef::parse_validated("keychain:");
        assert!(matches!(result, Err(PasswordRefParseError::EmptyValue)));
    }

    #[test]
    fn test_password_ref_parse_validated_keychain_invalid_chars() {
        let result = PasswordRef::parse_validated("keychain:my service");
        assert!(matches!(
            result,
            Err(PasswordRefParseError::InvalidKeychainKey(_, _))
        ));
    }

    #[test]
    fn test_password_ref_parse_validated_env_valid() {
        let result = PasswordRef::parse_validated("env:MY_PASSWORD");
        assert!(matches!(result, Ok(PasswordRef::Env(var)) if var == "MY_PASSWORD"));
    }

    #[test]
    fn test_password_ref_parse_validated_env_with_underscore() {
        let result = PasswordRef::parse_validated("env:MY_PASSWORD_123");
        assert!(matches!(result, Ok(PasswordRef::Env(var)) if var == "MY_PASSWORD_123"));
    }

    #[test]
    fn test_password_ref_parse_validated_env_underscore_start() {
        let result = PasswordRef::parse_validated("env:_PRIVATE_VAR");
        assert!(matches!(result, Ok(PasswordRef::Env(var)) if var == "_PRIVATE_VAR"));
    }

    #[test]
    fn test_password_ref_parse_validated_env_empty() {
        let result = PasswordRef::parse_validated("env:");
        assert!(matches!(result, Err(PasswordRefParseError::EmptyValue)));
    }

    #[test]
    fn test_password_ref_parse_validated_env_invalid_start() {
        let result = PasswordRef::parse_validated("env:1_VAR");
        assert!(matches!(
            result,
            Err(PasswordRefParseError::InvalidEnvVar(..))
        ));
    }

    #[test]
    fn test_password_ref_parse_validated_env_invalid_chars() {
        let result = PasswordRef::parse_validated("env:MY-PASSWORD");
        assert!(matches!(
            result,
            Err(PasswordRefParseError::InvalidEnvVar(..))
        ));
    }

    #[test]
    fn test_password_ref_parse_validated_plain_valid() {
        let result = PasswordRef::parse_validated("plain:secret123");
        assert!(matches!(result, Ok(PasswordRef::Plain(pwd)) if pwd == "secret123"));
    }

    #[test]
    fn test_password_ref_parse_validated_plain_empty() {
        let result = PasswordRef::parse_validated("plain:");
        assert!(matches!(result, Err(PasswordRefParseError::EmptyValue)));
    }

    #[test]
    fn test_password_ref_parse_validated_invalid_prefix() {
        let result = PasswordRef::parse_validated("vault:my-password");
        assert!(matches!(
            result,
            Err(PasswordRefParseError::InvalidPrefix(_))
        ));
    }

    #[test]
    fn test_password_ref_parse_validated_no_prefix() {
        let result = PasswordRef::parse_validated("my-password");
        assert!(matches!(
            result,
            Err(PasswordRefParseError::InvalidPrefix(_))
        ));
    }

    #[test]
    fn test_password_ref_parse_fallback() {
        // 验证 parse 方法对无效格式会回退到 Keychain
        let pr = PasswordRef::parse("invalid:format");
        assert!(matches!(pr, PasswordRef::Keychain(key) if key == "invalid:format"));

        let pr = PasswordRef::parse("simple-value");
        assert!(matches!(pr, PasswordRef::Keychain(key) if key == "simple-value"));
    }
}
