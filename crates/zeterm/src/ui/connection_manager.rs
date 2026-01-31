//! 连接管理器
//!
//! 负责管理 SSH 连接的建立、配置转换和状态更新。
//! 将 MainWindow 中的连接逻辑提取出来，提高代码可维护性。

use std::sync::Arc;

use gpui::{Context, Entity};
use tracing::{debug, error, info};
use zeterm_core::ConnectionState;
use zeterm_core::config::PasswordRef;
use zeterm_core::entities::{AuthConfig, HostConfig};
use zeterm_core::errors::ConnectionError;
use zeterm_ssh::{AuthMethod, SshConfig, SshConnection};
use zeterm_storage::{KeyringSecretStore, SecretHelper};

use crate::app::runtime;
use crate::app::session::SessionCoordinator;
use crate::ui::status_bar::{ConnectionStatus, StatusBar};
use crate::ui::tab_manager::{TabId, TabManager};

pub struct ConnectionManager;

impl ConnectionManager {
    /// 连接到主机
    ///
    /// 执行完整的 SSH 连接流程：
    /// 1. 创建 SSH Tab
    /// 2. 更新状态栏为"连接中"
    /// 3. 创建 SessionCoordinator
    /// 4. 添加终端面板（显示"正在连接..."）
    /// 5. 异步建立 SSH 连接并启动数据泵
    pub fn connect_to_host<T>(
        host: HostConfig,
        tab_id: TabId,
        coordinator: Arc<SessionCoordinator>,
        cx: &mut Context<T>,
        tab_manager: &Entity<TabManager>,
        status_bar: &Entity<StatusBar>,
        on_error: Option<Box<dyn Fn(String) + Send + 'static>>,
    ) -> Option<()>
    where
        T: 'static,
    {
        // 1. 激活新创建的 tab
        tab_manager.update(cx, |manager, cx| {
            manager.switch_to_tab(tab_id, cx);
        });

        // 2. 更新状态栏为"连接中"
        Self::update_connection_status(ConnectionStatus::Connecting, status_bar, cx);
        Self::update_user_host(
            Some(host.username.clone()),
            Some(host.host.clone()),
            status_bar,
            cx,
        );

        // 3-5. 异步执行 SSH 连接
        let host_clone = host.clone();
        let coordinator_clone = coordinator.clone();

        runtime::spawn(async move {
            info!(
                "开始异步 SSH 连接: {}@{}",
                host_clone.username, host_clone.host
            );

            match Self::do_ssh_connect(&host_clone, coordinator_clone.clone()).await {
                Ok(stream) => {
                    info!("SSH 连接成功: {}@{}", host_clone.username, host_clone.host);

                    // 启动数据泵
                    coordinator_clone
                        .start_data_pump(stream, move || {
                            debug!("数据泵收到新数据");
                        })
                        .await;

                    info!("数据泵已停止: {}@{}", host_clone.username, host_clone.host);
                },
                Err(e) => {
                    let error_msg = format!("SSH 连接失败: {} - {}", host_clone.name, e);
                    error!("{}", error_msg);

                    // 调用错误回调
                    if let Some(ref callback) = on_error {
                        callback(error_msg);
                    }
                },
            }
        });

        Some(())
    }

    /// 执行 SSH 连接（异步）
    ///
    /// 1. 转换 HostConfig 为 SshConfig
    /// 2. 创建 SshConnection 并连接
    /// 3. 设置连接到 SessionCoordinator
    /// 4. 返回数据流用于启动数据泵
    async fn do_ssh_connect(
        host: &HostConfig,
        coordinator: Arc<SessionCoordinator>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<Vec<u8>, ConnectionError>>,
        ConnectionError,
    > {
        // 1. 转换配置
        let ssh_config = Self::convert_to_ssh_config(host).await?;

        // 2. 创建 SSH 连接
        let ssh_conn = SshConnection::new(ssh_config);

        // 3. 建立连接
        info!("正在建立 SSH 连接...");
        ssh_conn.connect().await?;
        info!("SSH 握手和认证完成");

        // 4. 设置到 SessionCoordinator 并获取数据流
        let stream = coordinator.set_connection(Box::new(ssh_conn)).await;

        Ok(stream)
    }

    /// 将 HostConfig 转换为 SshConfig
    ///
    /// 处理认证配置的转换，包括从密钥链解析密码
    async fn convert_to_ssh_config(host: &HostConfig) -> Result<SshConfig, ConnectionError> {
        // 创建密钥助手用于解析密码引用
        let secret_helper = SecretHelper::<KeyringSecretStore>::default();

        // 转换认证方式
        let auth_method = match &host.auth_config {
            AuthConfig::Password { password_ref } => {
                debug!("解析密码认证: {}", password_ref);
                let parsed_ref = PasswordRef::parse(password_ref);
                let password = secret_helper
                    .resolve_password_ref(&parsed_ref)
                    .map_err(|e| ConnectionError::Configuration(format!("密码解析失败: {}", e)))?
                    .ok_or_else(|| {
                        ConnectionError::Authentication("无法获取密码，请检查密码配置".into())
                    })?;
                AuthMethod::Password(password)
            },
            AuthConfig::PublicKey {
                key_path,
                passphrase_ref,
            } => {
                debug!("解析公钥认证: {:?}", key_path);
                let passphrase = if let Some(pp_ref) = passphrase_ref {
                    let parsed_ref = PasswordRef::parse(pp_ref);
                    secret_helper
                        .resolve_password_ref(&parsed_ref)
                        .map_err(|e| {
                            ConnectionError::Configuration(format!("私钥密码解析失败: {}", e))
                        })?
                } else {
                    None
                };
                AuthMethod::PublicKey {
                    key_path: key_path.clone(),
                    passphrase,
                }
            },
            AuthConfig::Agent => {
                debug!("使用 SSH Agent 认证");
                AuthMethod::Agent
            },
        };

        // 构建 SshConfig
        let ssh_config = SshConfig::new(&host.host, &host.username)
            .with_port(host.port)
            .with_terminal_size(80, 24); // 默认终端大小，后续会通过 resize 调整

        // 设置认证方式（需要使用内部字段，因为 SshConfig 没有 with_auth_method）
        let mut ssh_config = ssh_config;
        ssh_config.auth_method = auth_method;

        debug!(
            "SshConfig 创建完成: {}@{}:{}",
            host.username, host.host, host.port
        );

        Ok(ssh_config)
    }

    /// 更新状态栏连接状态
    fn update_connection_status<T>(
        status: ConnectionStatus,
        status_bar: &Entity<StatusBar>,
        cx: &mut Context<T>,
    ) {
        status_bar.update(cx, |bar, cx| {
            bar.set_connection_status(status, cx);
        });
    }

    /// 更新状态栏用户和主机信息
    fn update_user_host<T>(
        username: Option<String>,
        hostname: Option<String>,
        status_bar: &Entity<StatusBar>,
        cx: &mut Context<T>,
    ) {
        status_bar.update(cx, |bar, cx| {
            bar.set_user_host(username, hostname, cx);
        });
    }

    /// 同步状态栏从协调器状态
    pub fn sync_status_bar_from_coordinator<T>(
        coordinator: &SessionCoordinator,
        status_bar: &Entity<StatusBar>,
        cx: &mut Context<T>,
    ) {
        let actual_state = coordinator.connection_state();
        let expected_status = match actual_state {
            ConnectionState::Idle | ConnectionState::Disconnected { .. } => {
                ConnectionStatus::Disconnected
            },
            ConnectionState::Connected { .. } => ConnectionStatus::Connected,
            ConnectionState::Connecting { .. }
            | ConnectionState::Authenticating
            | ConnectionState::Reconnecting { .. }
            | ConnectionState::Disconnecting => ConnectionStatus::Connecting,
        };

        status_bar.update(cx, |bar, cx| {
            let current_status = bar.status_info().connection_status;
            if current_status != expected_status {
                bar.set_connection_status(expected_status, cx);
                debug!(
                    "状态栏从 {:?} 更新为 {:?} (coordinator state: {:?})",
                    current_status, expected_status, actual_state
                );
            }
        });
    }
}
