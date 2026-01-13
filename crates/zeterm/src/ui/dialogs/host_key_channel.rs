//! 主机密钥确认跨线程通信模块
//!
//! 提供 SSH 连接线程和 UI 主线程之间的通信机制，
//! 用于主机密钥确认对话框的显示和响应处理。

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use parking_lot::Mutex;

/// 主机密钥确认请求
#[derive(Debug, Clone)]
pub struct HostKeyConfirmRequest {
    /// 主机名
    pub hostname: String,
    /// 端口
    pub port: u16,
    /// 密钥类型
    pub key_type: String,
    /// 密钥指纹
    pub fingerprint: String,
    /// 请求 ID（用于匹配响应）
    pub request_id: u64,
}

impl HostKeyConfirmRequest {
    /// 创建新的确认请求
    pub fn new(
        hostname: impl Into<String>,
        port: u16,
        key_type: impl Into<String>,
        fingerprint: impl Into<String>,
    ) -> Self {
        static REQUEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            hostname: hostname.into(),
            port,
            key_type: key_type.into(),
            fingerprint: fingerprint.into(),
            request_id: REQUEST_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
        }
    }
}

/// 主机密钥确认响应
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostKeyConfirmResponse {
    /// 请求 ID
    pub request_id: u64,
    /// 是否接受
    pub accepted: bool,
    /// 是否记住选择
    pub remember: bool,
}

impl HostKeyConfirmResponse {
    /// 创建接受响应
    pub fn accept(request_id: u64, remember: bool) -> Self {
        Self {
            request_id,
            accepted: true,
            remember,
        }
    }

    /// 创建拒绝响应
    pub fn reject(request_id: u64) -> Self {
        Self {
            request_id,
            accepted: false,
            remember: false,
        }
    }
}

/// 主机密钥确认通道
///
/// 用于 SSH 连接线程和 UI 主线程之间的双向通信
pub struct HostKeyConfirmChannel {
    /// 请求发送端（SSH 线程 -> UI 线程）
    request_tx: Sender<HostKeyConfirmRequest>,
    /// 请求接收端（UI 线程）
    request_rx: Arc<Mutex<Receiver<HostKeyConfirmRequest>>>,
    /// 响应发送端（UI 线程 -> SSH 线程）
    response_tx: Sender<HostKeyConfirmResponse>,
    /// 响应接收端（SSH 线程）
    response_rx: Arc<Mutex<Receiver<HostKeyConfirmResponse>>>,
}

impl HostKeyConfirmChannel {
    /// 创建新的通信通道
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();

        Self {
            request_tx,
            request_rx: Arc::new(Mutex::new(request_rx)),
            response_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
        }
    }

    /// 获取请求发送端的克隆（供 SSH 线程使用）
    pub fn request_sender(&self) -> HostKeyRequestSender {
        HostKeyRequestSender {
            tx: self.request_tx.clone(),
            response_rx: self.response_rx.clone(),
        }
    }

    /// 获取请求接收端（供 UI 线程使用）
    pub fn request_receiver(&self) -> HostKeyRequestReceiver {
        HostKeyRequestReceiver {
            rx: self.request_rx.clone(),
            response_tx: self.response_tx.clone(),
        }
    }
}

impl Default for HostKeyConfirmChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// 请求发送端（SSH 线程使用）
#[derive(Clone)]
pub struct HostKeyRequestSender {
    tx: Sender<HostKeyConfirmRequest>,
    response_rx: Arc<Mutex<Receiver<HostKeyConfirmResponse>>>,
}

impl HostKeyRequestSender {
    /// 发送确认请求并等待响应
    ///
    /// # 参数
    /// - `request`: 确认请求
    /// - `timeout`: 超时时间///
    /// # 返回
    /// - `Some(response)`: 收到响应
    /// - `None`: 超时或通道关闭
    pub fn request_and_wait(
        &self,
        request: HostKeyConfirmRequest,
        timeout: Duration,
    ) -> Option<HostKeyConfirmResponse> {
        let request_id = request.request_id;

        // 发送请求
        if self.tx.send(request).is_err() {
            return None;
        }

        // 等待响应
        let rx = self.response_rx.lock();
        match rx.recv_timeout(timeout) {
            Ok(response) if response.request_id == request_id => Some(response),
            Ok(_) => None,  // 请求 ID 不匹配
            Err(_) => None, // 超时或通道关闭
        }
    }

    /// 发送确认请求（不等待响应）
    pub fn send_request(&self, request: HostKeyConfirmRequest) -> bool {
        self.tx.send(request).is_ok()
    }
}

/// 请求接收端（UI 线程使用）
#[derive(Clone)]
pub struct HostKeyRequestReceiver {
    rx: Arc<Mutex<Receiver<HostKeyConfirmRequest>>>,
    response_tx: Sender<HostKeyConfirmResponse>,
}

impl HostKeyRequestReceiver {
    ///尝试接收请求（非阻塞）
    pub fn try_recv(&self) -> Option<HostKeyConfirmRequest> {
        let rx = self.rx.lock();
        rx.try_recv().ok()
    }

    /// 接收请求（阻塞）
    pub fn recv(&self) -> Option<HostKeyConfirmRequest> {
        let rx = self.rx.lock();
        rx.recv().ok()
    }

    /// 接收请求（带超时）
    pub fn recv_timeout(&self, timeout: Duration) -> Option<HostKeyConfirmRequest> {
        let rx = self.rx.lock();
        rx.recv_timeout(timeout).ok()
    }

    /// 发送响应
    pub fn send_response(&self, response: HostKeyConfirmResponse) -> bool {
        self.response_tx.send(response).is_ok()
    }

    /// 发送接受响应
    pub fn accept(&self, request_id: u64, remember: bool) -> bool {
        self.send_response(HostKeyConfirmResponse::accept(request_id, remember))
    }

    /// 发送拒绝响应
    pub fn reject(&self, request_id: u64) -> bool {
        self.send_response(HostKeyConfirmResponse::reject(request_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_channel_creation() {
        let channel = HostKeyConfirmChannel::new();
        let _sender = channel.request_sender();
        let _receiver = channel.request_receiver();
    }

    #[test]
    fn test_request_response() {
        let channel = HostKeyConfirmChannel::new();
        let sender = channel.request_sender();
        let receiver = channel.request_receiver();

        // 在另一个线程中发送请求
        let handle = thread::spawn(move || {
            let request =
                HostKeyConfirmRequest::new("example.com", 22, "ssh-ed25519", "SHA256:abc123");
            sender.request_and_wait(request, Duration::from_secs(5))
        });

        // 在主线程中接收请求并发送响应
        thread::sleep(Duration::from_millis(100));
        if let Some(request) = receiver.recv_timeout(Duration::from_secs(1)) {
            assert_eq!(request.hostname, "example.com");
            assert_eq!(request.port, 22);
            receiver.accept(request.request_id, true);
        }

        // 等待发送线程完成
        let response = handle.join().unwrap();
        assert!(response.is_some());
        let response = response.unwrap();
        assert!(response.accepted);
        assert!(response.remember);
    }

    #[test]
    fn test_request_timeout() {
        let channel = HostKeyConfirmChannel::new();
        let sender = channel.request_sender();

        let request = HostKeyConfirmRequest::new("example.com", 22, "ssh-ed25519", "SHA256:abc123");

        // 没有接收端响应，应该超时
        let response = sender.request_and_wait(request, Duration::from_millis(100));
        assert!(response.is_none());
    }

    #[test]
    fn test_try_recv_empty() {
        let channel = HostKeyConfirmChannel::new();
        let receiver = channel.request_receiver();

        // 没有请求，应该返回 None
        assert!(receiver.try_recv().is_none());
    }

    #[test]
    fn test_request_id_unique() {
        let req1 = HostKeyConfirmRequest::new("host1", 22, "type", "fp");
        let req2 = HostKeyConfirmRequest::new("host2", 22, "type", "fp");
        assert_ne!(req1.request_id, req2.request_id);
    }
}
