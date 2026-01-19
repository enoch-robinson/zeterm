//! 主机列表视图
//!
//! 显示和管理 SSH 主机列表。

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, prelude::*,
};
use std::sync::Arc;
use zeterm_core::entities::{HostConfig, HostId};
use zeterm_storage::{Database, HostRepository, SqliteHostRepository};

/// 主机列表视图
pub struct HostListView {
    /// 数据库连接
    database: Arc<Database>,

    /// 主机仓库
    repository: Arc<SqliteHostRepository>,

    /// 主机列表
    hosts: Vec<HostConfig>,

    /// 当前选中的主机 ID
    selected_host_id: Option<HostId>,

    /// 搜索关键词
    search_query: String,

    ///焦点句柄
    focus_handle: FocusHandle,
}

impl HostListView {
    /// 创建新的主机列表视图
    pub fn new(database: Arc<Database>, cx: &mut Context<Self>) -> Self {
        let repository = Arc::new(SqliteHostRepository::new(database.pool().clone()));
        let focus_handle = cx.focus_handle();

        let mut view = Self {
            database,
            repository,
            hosts: Vec::new(),
            selected_host_id: None,
            search_query: String::new(),
            focus_handle,
        };

        // 异步加载主机列表
        view.load_hosts(cx);

        view
    }

    /// 加载主机列表
    fn load_hosts(&mut self, cx: &mut Context<Self>) {
        // TODO: 实现异步加载
        // 当前使用同步实现以避免 GPUI 异步模式的复杂性
        // 需要研究 GPUI 的正确异步模式 (参考 Zed 的 terminal_view.rs)
        // 或使用 cx.spawn() 配合正确的生命周期和类型注解

        tracing::info!("加载主机列表（同步模式）");

        // 暂时使用空列表，实际加载需要异步实现
        // 可以考虑使用 tokio::spawn 或其他异步运行时
        cx.notify();
    }

    /// 刷新主机列表
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_hosts(cx);
    }

    /// 选择主机
    pub fn select_host(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.selected_host_id = Some(host_id);
        cx.notify();
    }

    /// 获取选中的主机
    pub fn selected_host(&self) -> Option<&HostConfig> {
        self.selected_host_id
            .and_then(|id| self.hosts.iter().find(|h| h.id == Some(id)))
    }

    /// 处理主机双击（连接）
    fn handle_host_double_click(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        tracing::info!("双击连接主机: {}", host.name);
        // TODO: 触发连接事件
        cx.emit(HostListEvent::ConnectRequested(host.clone()));
    }

    /// 处理新建主机
    fn handle_new_host(&mut self, cx: &mut Context<Self>) {
        tracing::info!("新建主机");
        // TODO: 显示新建主机对话框
        cx.emit(HostListEvent::NewHostRequested);
    }

    /// 处理编辑主机
    fn handle_edit_host(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        tracing::info!("编辑主机: {}", host.name);
        // TODO: 显示编辑主机对话框
        cx.emit(HostListEvent::EditHostRequested(host.clone()));
    }

    /// 处理删除主机
    fn handle_delete_host(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        let host_id = match host.id {
            Some(id) => id,
            None => {
                tracing::warn!("无法删除没有 ID 的主机");
                return;
            },
        };

        let host_name = host.name.clone();

        tracing::info!("删除主机: {} (ID: {})", host_name, host_id);

        // TODO: 实现异步删除
        // 当前使用同步实现以避免 GPUI 异步模式的复杂性
        // 需要研究 GPUI 的正确异步模式来:
        // 1. 异步调用 repository.delete(host_id)
        // 2. 更新视图状态 (重新加载主机列表)
        // 3. 触发 HostDeleted 事件
        // 参考: Zed 编辑器的 terminal_view.rs 实现

        // 暂时只记录日志，实际删除需要异步实现
        tracing::warn!("删除功能暂未实现（需要异步支持）");
        cx.notify();
    }

    /// 渲染主机列表项
    fn render_host_item(&self, host: &HostConfig, cx: &mut Context<Self>) -> impl IntoElement {
        let host_id = host.id;
        let is_selected = self.selected_host_id == host_id;

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .when(is_selected, |this| this.bg(gpui::rgb(0x2563eb)))
            .hover(|this| this.bg(gpui::rgb(0x3b82f6)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    if let Some(id) = host_id {
                        this.select_host(id, cx);
                    }
                }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(gpui::white())
                            .child(host.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x9ca3af))
                            .child(format!("{}@{}:{}", host.username, host.host, host.port)),
                    ),
            )
    }
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::rgb(0x1f2937))
            .child(
                // 工具栏
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(gpui::rgb(0x374151))
                    .child(div().text_sm().text_color(gpui::white()).child("主机列表"))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(gpui::rgb(0x3b82f6))
                                    .cursor_pointer()
                                    .hover(|this| this.bg(gpui::rgb(0x2563eb)))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _event, _window, cx| {
                                            this.handle_new_host(cx);
                                        }),
                                    )
                                    .child(div().text_xs().text_color(gpui::white()).child("新建")),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(gpui::rgb(0x374151))
                                    .cursor_pointer()
                                    .hover(|this| this.bg(gpui::rgb(0x4b5563)))
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _event, _window, cx| {
                                            this.refresh(cx);
                                        }),
                                    )
                                    .child(div().text_xs().text_color(gpui::white()).child("刷新")),
                            ),
                    ),
            )
            .child({
                // 主机列表
                let mut list_div = div().flex().flex_col().flex_1().px_2().py_2().gap_1();

                for host in &self.hosts {
                    list_div = list_div.child(self.render_host_item(host, cx));
                }

                list_div
            })
            .child(
                // 状态栏
                div()
                    .flex()
                    .items_center()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(gpui::rgb(0x374151))
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x9ca3af))
                            .child(format!("共{} 个主机", self.hosts.len())),
                    ),
            )
    }
}

impl Focusable for HostListView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// 主机列表事件
#[derive(Clone, Debug)]
pub enum HostListEvent {
    /// 请求连接主机
    ConnectRequested(HostConfig),

    /// 请求新建主机
    NewHostRequested,

    /// 请求编辑主机
    EditHostRequested(HostConfig),

    /// 主机已删除
    HostDeleted(HostId),
}

impl EventEmitter<HostListEvent> for HostListView {}
