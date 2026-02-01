//! 主机配置实体定义
//!
//! 定义主机连接配置相关的数据结构。

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 主机 ID
///
/// 唯一标识一个主机配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(pub i64);

impl HostId {
    /// 创建新的主机 ID
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// 获取内部值
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for HostId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<i64> for HostId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

/// 主机配置
///
/// 存储 SSH 主机的连接配置信息。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostConfig {
    /// 主机 ID（数据库主键，新建时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<HostId>,

    /// 主机名称（用户自定义的显示名称）
    pub name: String,

    /// 主机地址（IP 或域名）
    pub host: String,

    /// SSH 端口
    #[serde(default = "default_ssh_port")]
    pub port: u16,

    /// 用户名
    pub username: String,

    /// 认证配置
    pub auth_config: AuthConfig,

    /// 分组（可选，用于组织主机）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,

    /// 标签（可选，用于搜索和过滤）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// 描述（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 创建时间（Unix 时间戳，秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,

    /// 更新时间（Unix 时间戳，秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
}

fn default_ssh_port() -> u16 {
    22
}

impl HostConfig {
    /// 创建新的主机配置
    pub fn new(name: String, host: String, username: String, auth_config: AuthConfig) -> Self {
        Self {
            id: None,
            name,
            host,
            port: 22,
            username,
            auth_config,
            group: None,
            tags: Vec::new(),
            description: None,
            created_at: None,
            updated_at: None,
        }
    }

    /// 验证配置是否有效
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

        self.auth_config.validate()?;

        Ok(())
    }

    /// 获取连接地址字符串（格式：username@host:port）
    pub fn connection_string(&self) -> String {
        format!("{}@{}:{}", self.username, self.host, self.port)
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
}

/// 认证配置
///
/// 定义 SSH 连接的认证方式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// 密码认证
    Password {
        /// 密码引用（格式：keychain:xxx 或 env:XXX）
        /// 不直接存储明文密码
        password_ref: String,
    },

    /// 公钥认证
    PublicKey {
        /// 私钥文件路径
        key_path: PathBuf,

        /// 私钥密码引用（可选，用于加密的私钥）
        #[serde(skip_serializing_if = "Option::is_none")]
        passphrase_ref: Option<String>,
    },

    /// SSH Agent 认证
    Agent,
}

impl AuthConfig {
    /// 创建密码认证配置
    pub fn password(password_ref: String) -> Self {
        Self::Password { password_ref }
    }

    /// 创建公钥认证配置
    pub fn public_key(key_path: PathBuf, passphrase_ref: Option<String>) -> Self {
        Self::PublicKey {
            key_path,
            passphrase_ref,
        }
    }

    /// 创建 Agent 认证配置
    pub fn agent() -> Self {
        Self::Agent
    }

    /// 验证认证配置是否有效
    pub fn validate(&self) -> Result<()> {
        match self {
            AuthConfig::Password { password_ref } => {
                if password_ref.trim().is_empty() {
                    bail!("密码引用不能为空");
                }
                Ok(())
            },
            AuthConfig::PublicKey {
                key_path,
                passphrase_ref,
            } => {
                if !key_path.exists() {
                    bail!("私钥文件不存在: {:?}", key_path);
                }
                if let Some(passphrase) = passphrase_ref {
                    if passphrase.trim().is_empty() {
                        bail!("私钥密码引用不能为空");
                    }
                }
                Ok(())
            },
            AuthConfig::Agent => Ok(()),
        }
    }

    /// 获取认证类型的显示名称
    pub fn type_name(&self) -> &'static str {
        match self {
            AuthConfig::Password { .. } => "密码",
            AuthConfig::PublicKey { .. } => "公钥",
            AuthConfig::Agent => "SSH Agent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_config_new() {
        let config = HostConfig::new(
            "测试服务器".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::password("keychain:test".to_string()),
        );

        assert_eq!(config.name, "测试服务器");
        assert_eq!(config.host, "192.168.1.100");
        assert_eq!(config.port, 22);
        assert_eq!(config.username, "root");
        assert!(config.id.is_none());
    }

    #[test]
    fn test_host_config_validate() {
        let mut config = HostConfig::new(
            "测试".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::password("keychain:test".to_string()),
        );

        assert!(config.validate().is_ok());

        config.name = "".to_string();
        assert!(config.validate().is_err());

        config.name = "测试".to_string();
        config.host = "".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_host_config_connection_string() {
        let config = HostConfig::new(
            "测试".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::agent(),
        );

        assert_eq!(config.connection_string(), "root@192.168.1.100:22");
    }

    #[test]
    fn test_auth_config_password() {
        let auth = AuthConfig::password("keychain:test".to_string());
        assert_eq!(auth.type_name(), "密码");
        assert!(auth.validate().is_ok());
    }

    #[test]
    fn test_auth_config_agent() {
        let auth = AuthConfig::agent();
        assert_eq!(auth.type_name(), "SSH Agent");
        assert!(auth.validate().is_ok());
    }

    #[test]
    fn test_host_config_serialization() {
        let config = HostConfig::new(
            "测试".to_string(),
            "192.168.1.100".to_string(),
            "root".to_string(),
            AuthConfig::password("keychain:test".to_string()),
        )
        .with_group("生产环境".to_string())
        .with_tags(vec!["web".to_string(), "database".to_string()]);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: HostConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, deserialized);
    }
}
