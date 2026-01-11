//! SSH Handler模块
//!
//! 实现 russh 的 client::Handler trait，处理 SSH 连接事件。

use std::sync::Arc;

use parking_lot::Mutex;
use russh::ChannelId;
use russh::client::{Handler, Session};
use russh::keys::PublicKey;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::config::HostKeyVerification;

/// SSH 数据接收器类型
pub type DataSender = mpsc::UnboundedSender<Vec<u8>>;
pub type DataReceiver = mpsc::UnboundedReceiver<Vec<u8>>;

/// SSH Handler 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerState {
    /// 初始状态
    Initial,
    /// 已验证主机密钥
    HostKeyVerified,
    /// 已认证
    Authenticated,
    /// 会话已建立
    SessionEstablished,
    /// 已断开
    Disconnected,
}

/// SSH 事件处理器
///
/// 实现 russh 的 Handler trait，处理 SSH 连接的各种事件。
pub struct SshHandler {
    /// 数据发送通道
    data_sender: DataSender,
    /// 主机密钥验证策略
    pub host_key_verification: HostKeyVerification,
    /// 当前状态
    state: Arc<Mutex<HandlerState>>,
    /// 服务器主机名（用于主机密钥验证）
    server_host: String,
    /// 服务器端口
    server_port: u16,
}

impl SshHandler {
    /// 创建新的 SSH Handler
    pub fn new(
        data_sender: DataSender,
        host_key_verification: HostKeyVerification,
        server_host: String,
        server_port: u16,
    ) -> Self {
        Self {
            data_sender,
            host_key_verification,
            state: Arc::new(Mutex::new(HandlerState::Initial)),
            server_host,
            server_port,
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> HandlerState {
        *self.state.lock()
    }

    /// 设置状态
    pub fn set_state(&self, new_state: HandlerState) {
        let mut state = self.state.lock();
        debug!("SSH handler state: {:?} -> {:?}", *state, new_state);
        *state = new_state;
    }

    /// 验证主机密钥
    fn verify_host_key(&self, _server_public_key: &PublicKey) -> bool {
        match &self.host_key_verification {
            HostKeyVerification::AutoAccept => {
                warn!(
                    "Auto-accepting host key for {}:{} (insecure)",
                    self.server_host, self.server_port
                );
                true
            },
            HostKeyVerification::Strict => {
                warn!("Strict host key verification not yet implemented, rejecting");
                false
            },
            HostKeyVerification::AskOnFirstConnect => {
                warn!("Interactive host key verification not yet implemented, auto-accepting");
                true
            },
            HostKeyVerification::KnownHostsFile(_path) => {
                warn!("Known hosts file verification not yet implemented, rejecting");
                false
            },
        }
    }
}

impl Handler for SshHandler {
    type Error = russh::Error;

    /// 检查服务器公钥
    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        info!(
            "Checking server key for {}:{}",
            self.server_host, self.server_port
        );

        let accepted = self.verify_host_key(server_public_key);

        if accepted {
            self.set_state(HandlerState::HostKeyVerified);
            info!("Server key accepted");
        } else {
            error!("Server key rejected");
        }

        Ok(accepted)
    }

    /// 处理通道数据
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!("Received {} bytes on channel {:?}", data.len(), channel);

        if let Err(e) = self.data_sender.send(data.to_vec()) {
            error!("Failed to send data to receiver: {}", e);
        }

        Ok(())
    }

    /// 处理扩展数据（stderr）
    async fn extended_data(
        &mut self,
        channel: ChannelId,
        ext: u32,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        debug!(
            "Received {} bytes of extended data (ext={}) on channel {:?}",
            data.len(),
            ext,
            channel
        );

        // 将stderr 数据也发送到主数据流
        if let Err(e) = self.data_sender.send(data.to_vec()) {
            error!("Failed to send extended data to receiver: {}", e);
        }

        Ok(())
    }

    /// 处理 EOF
    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Channel {:?} received EOF", channel);
        Ok(())
    }

    /// 处理通道关闭
    async fn channel_close(
        &mut self,
        channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!("Channel {:?} closed", channel);
        self.set_state(HandlerState::Disconnected);
        Ok(())
    }

    /// 处理通道打开确认
    async fn channel_open_confirmation(
        &mut self,
        channel: ChannelId,
        max_packet_size: u32,
        window_size: u32,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        info!(
            "Channel {:?} opened: max_packet_size={}, window_size={}",
            channel, max_packet_size, window_size
        );
        self.set_state(HandlerState::SessionEstablished);
        Ok(())
    }

    /// 处理通道打开失败
    async fn channel_open_failure(
        &mut self,
        channel: ChannelId,
        reason: russh::ChannelOpenFailure,
        description: &str,
        _language: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        error!(
            "Channel {:?} open failed: {:?} - {}",
            channel, reason, description
        );
        Ok(())
    }
}

/// 创建数据通道
pub fn create_data_channel() -> (DataSender, DataReceiver) {
    mpsc::unbounded_channel()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_state_initial() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AutoAccept,
            "localhost".to_string(),
            22,
        );
        assert_eq!(handler.state(), HandlerState::Initial);
    }

    #[test]
    fn test_handler_state_transition() {
        let (sender, _receiver) = create_data_channel();
        let handler = SshHandler::new(
            sender,
            HostKeyVerification::AutoAccept,
            "localhost".to_string(),
            22,
        );

        handler.set_state(HandlerState::HostKeyVerified);
        assert_eq!(handler.state(), HandlerState::HostKeyVerified);

        handler.set_state(HandlerState::Authenticated);
        assert_eq!(handler.state(), HandlerState::Authenticated);
    }

    #[test]
    fn test_create_data_channel() {
        let (sender, mut receiver) = create_data_channel();

        // 发送数据
        sender.send(vec![1, 2, 3]).unwrap();

        // 接收数据
        let data = receiver.try_recv().unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }
}

