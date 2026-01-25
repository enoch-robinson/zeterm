//! 状态栏模块
//!
//! 显示终端连接状态、尺寸、编码等信息。

use gpui::{
    App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    /// 未连接
    #[default]
    Disconnected,
    /// 连接中
    Connecting,
    /// 已连接
    Connected,
    /// 连接错误
    Error,
}

impl ConnectionStatus {
    /// 获取状态图标
    pub fn icon(&self) -> &'static str {
        match self {
            ConnectionStatus::Disconnected => "○",
            ConnectionStatus::Connecting => "◐",
            ConnectionStatus::Connected => "●",
            ConnectionStatus::Error => "✕",
        }
    }

    /// 获取状态文本
    pub fn text(&self) -> &'static str {
        match self {
            ConnectionStatus::Disconnected => "Disconnected",
            ConnectionStatus::Connecting => "Connecting...",
            ConnectionStatus::Connected => "Connected",
            ConnectionStatus::Error => "Error",
        }
    }

    /// 获取状态颜色（返回 CSS 风格的颜色描述）
    pub fn color_name(&self) -> &'static str {
        match self {
            ConnectionStatus::Disconnected => "gray",
            ConnectionStatus::Connecting => "yellow",
            ConnectionStatus::Connected => "green",
            ConnectionStatus::Error => "red",
        }
    }
}

/// 状态栏信息
#[derive(Debug, Clone, Default)]
pub struct StatusInfo {
    /// 连接状态
    pub connection_status: ConnectionStatus,
    /// 用户名
    pub username: Option<String>,
    /// 主机名
    pub hostname: Option<String>,
    /// 终端列数
    pub cols: u16,
    /// 终端行数
    pub rows: u16,
    /// 编码
    pub encoding: String,
    /// RTT 延迟（毫秒）
    pub rtt_ms: Option<u32>,
    /// 当前工作目录
    pub cwd: Option<String>,
}

impl StatusInfo {
    /// 创建新的状态信息
    pub fn new() -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            username: None,
            hostname: None,
            cols: 80,
            rows: 24,
            encoding: "UTF-8".to_string(),
            rtt_ms: None,
            cwd: None,
        }
    }

    /// 设置连接状态
    pub fn with_connection_status(mut self, status: ConnectionStatus) -> Self {
        self.connection_status = status;
        self
    }

    /// 设置用户名和主机名
    pub fn with_user_host(
        mut self,
        username: impl Into<String>,
        hostname: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.hostname = Some(hostname.into());
        self
    }

    /// 设置终端尺寸
    pub fn with_size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }

    /// 设置编码
    pub fn with_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.encoding = encoding.into();
        self
    }

    /// 设置 RTT
    pub fn with_rtt(mut self, rtt_ms: u32) -> Self {
        self.rtt_ms = Some(rtt_ms);
        self
    }

    /// 获取用户@主机显示文本
    pub fn user_host_display(&self) -> Option<String> {
        match (&self.username, &self.hostname) {
            (Some(user), Some(host)) => Some(format!("{}@{}", user, host)),
            (None, Some(host)) => Some(host.clone()),
            (Some(user), None) => Some(user.clone()),
            (None, None) => None,
        }
    }

    /// 获取尺寸显示文本
    pub fn size_display(&self) -> String {
        format!("{}×{}", self.cols, self.rows)
    }

    /// 获取 RTT 显示文本
    pub fn rtt_display(&self) -> Option<String> {
        self.rtt_ms.map(|rtt| {
            if rtt < 1000 {
                format!("{}ms", rtt)
            } else {
                format!("{:.1}s", rtt as f64 / 1000.0)
            }
        })
    }
}

/// 状态栏组件
pub struct StatusBar {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 状态信息
    status_info: StatusInfo,
}

impl StatusBar {
    /// 创建新的状态栏
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            status_info: StatusInfo::new(),
        }
    }

    /// 更新状态信息
    pub fn update_status(&mut self, info: StatusInfo, cx: &mut Context<Self>) {
        self.status_info = info;
        cx.notify();
    }

    /// 设置连接状态
    pub fn set_connection_status(&mut self, status: ConnectionStatus, cx: &mut Context<Self>) {
        self.status_info.connection_status = status;
        cx.notify();
    }

    /// 设置用户和主机
    pub fn set_user_host(
        &mut self,
        username: Option<String>,
        hostname: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.status_info.username = username;
        self.status_info.hostname = hostname;
        cx.notify();
    }

    /// 设置终端尺寸
    pub fn set_size(&mut self, cols: u16, rows: u16, cx: &mut Context<Self>) {
        self.status_info.cols = cols;
        self.status_info.rows = rows;
        cx.notify();
    }

    /// 设置 RTT
    pub fn set_rtt(&mut self, rtt_ms: Option<u32>, cx: &mut Context<Self>) {
        self.status_info.rtt_ms = rtt_ms;
        cx.notify();
    }

    /// 获取状态信息
    pub fn status_info(&self) -> &StatusInfo {
        &self.status_info
    }

    /// 渲染状态项分隔符
    fn render_separator(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div().mx_2().text_color(theme.muted_foreground).child("|")
    }

    /// 渲染连接状态
    fn render_connection_status(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let status = &self.status_info.connection_status;

        // 根据状态选择颜色
        let status_color = match status {
            ConnectionStatus::Disconnected => theme.muted_foreground,
            ConnectionStatus::Connecting => gpui::hsla(0.14, 0.9, 0.5, 1.0), // 黄色
            ConnectionStatus::Connected => gpui::hsla(0.33, 0.7, 0.45, 1.0), // 绿色
            ConnectionStatus::Error => gpui::hsla(0.0, 0.8, 0.5, 1.0),       // 红色
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_color(status_color)
                    .child(SharedString::from(status.icon())),
            )
            .child(
                div()
                    .text_color(theme.foreground)
                    .child(SharedString::from(status.text())),
            )
    }

    /// 渲染用户@主机
    fn render_user_host(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let theme = cx.theme();
        self.status_info.user_host_display().map(|text| {
            div()
                .text_color(theme.foreground)
                .child(SharedString::from(text))
        })
    }

    /// 渲染编码信息
    fn render_encoding(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(self.status_info.encoding.clone()))
    }

    /// 渲染终端尺寸
    fn render_size(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(self.status_info.size_display()))
    }

    /// 渲染 RTT 信息
    fn render_rtt(&self, cx: &Context<Self>) -> Option<impl IntoElement> {
        let theme = cx.theme();
        self.status_info.rtt_display().map(|text| {
            let rtt = self.status_info.rtt_ms.unwrap_or(0);
            // 根据延迟选择颜色
            let color = if rtt < 100 {
                gpui::hsla(0.33, 0.7, 0.45, 1.0) // 绿色 - 好
            } else if rtt < 300 {
                gpui::hsla(0.14, 0.9, 0.5, 1.0) // 黄色 - 一般
            } else {
                gpui::hsla(0.0, 0.8, 0.5, 1.0) // 红色 - 差
            };

            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(div().text_color(theme.muted_foreground).child("RTT:"))
                .child(div().text_color(color).child(SharedString::from(text)))
        })
    }
}

impl Focusable for StatusBar {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 主容器
        div()
            .id("status-bar")
            .w_full()
            .h(px(24.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_3()
            .bg(theme.secondary)
            .border_t_1()
            .border_color(theme.border)
            .text_sm()
            .child(
                // 左侧：连接状态 + 用户@主机
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(self.render_connection_status(cx))
                    .children(self.render_user_host(cx).map(|el| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(self.render_separator(cx))
                            .child(el)
                    })),
            )
            .child(
                // 右侧：编码 + 尺寸 + RTT
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(self.render_encoding(cx))
                    .child(self.render_separator(cx))
                    .child(self.render_size(cx))
                    .children(self.render_rtt(cx).map(|el| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(self.render_separator(cx))
                            .child(el)
                    })),
            )
    }
}

/// 创建状态栏实体的便捷方法
pub fn create_status_bar(cx: &mut Context<StatusBar>) -> StatusBar {
    StatusBar::new(cx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_status_icon() {
        assert_eq!(ConnectionStatus::Disconnected.icon(), "○");
        assert_eq!(ConnectionStatus::Connecting.icon(), "◐");
        assert_eq!(ConnectionStatus::Connected.icon(), "●");
        assert_eq!(ConnectionStatus::Error.icon(), "✕");
    }

    #[test]
    fn test_status_info_user_host_display() {
        let info = StatusInfo::new().with_user_host("root", "server.example.com");
        assert_eq!(
            info.user_host_display(),
            Some("root@server.example.com".to_string())
        );

        let info2 = StatusInfo::new();
        assert_eq!(info2.user_host_display(), None);
    }

    #[test]
    fn test_status_info_size_display() {
        let info = StatusInfo::new().with_size(120, 40);
        assert_eq!(info.size_display(), "120×40");
    }

    #[test]
    fn test_status_info_rtt_display() {
        let info = StatusInfo::new().with_rtt(50);
        assert_eq!(info.rtt_display(), Some("50ms".to_string()));

        let info2 = StatusInfo::new().with_rtt(1500);
        assert_eq!(info2.rtt_display(), Some("1.5s".to_string()));

        let info3 = StatusInfo::new();
        assert_eq!(info3.rtt_display(), None);
    }

    #[test]
    fn test_status_info_builder() {
        let info = StatusInfo::new()
            .with_connection_status(ConnectionStatus::Connected)
            .with_user_host("admin", "192.168.1.1")
            .with_size(100, 30)
            .with_encoding("GBK")
            .with_rtt(25);

        assert_eq!(info.connection_status, ConnectionStatus::Connected);
        assert_eq!(info.username, Some("admin".to_string()));
        assert_eq!(info.hostname, Some("192.168.1.1".to_string()));
        assert_eq!(info.cols, 100);
        assert_eq!(info.rows, 30);
        assert_eq!(info.encoding, "GBK");
        assert_eq!(info.rtt_ms, Some(25));
    }
}
