//! 主机列表视图
//!
//! 显示和管理 SSH 主机列表。

use crate::app::runtime;
use crate::ui::dialogs::{
    DeleteConfirmDialog, DeleteConfirmEvent, DeleteTarget, HostConnectionDialog,
};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window, div, prelude::*,
};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use zeterm_core::entities::{HostConfig, HostId};
use zeterm_storage::{Database, HostRepository, SqliteHostRepository};

/// 主机列表视图
pub struct HostListView {
    /// 数据库连接
    database: Arc<Database>,

    /// 主机仓库
    repository: Arc<SqliteHostRepository>,

    /// 主机列表（使用 Arc<RwLock> 支持异步更新）
    hosts: Arc<RwLock<Vec<HostConfig>>>,

    /// 当前选中的主机 ID
    selected_host_id: Option<HostId>,

    /// 搜索关键词
    search_query: String,

    /// 上次点击的主机和时间（用于双击检测）
    last_click: Option<(HostId, Instant)>,

    /// 展开的分组集合
    expanded_groups: Arc<RwLock<HashSet<String>>>,

    /// 当前显示右键菜单的主机
    context_menu_host: Option<HostConfig>,

    /// 焦点句柄
    focus_handle: FocusHandle,

    /// 连接对话框（新建或编辑主机）- 使用共享引用以便在回调中关闭
    connection_dialog: Arc<Mutex<Option<Entity<HostConnectionDialog>>>>,

    /// 删除确认对话框
    delete_dialog: Option<Entity<DeleteConfirmDialog>>,

    /// 待删除的主机信息（ID 和名称）
    pending_delete: Option<(HostId, String)>,
}

impl HostListView {
    /// 创建新的主机列表视图
    pub fn new(database: Arc<Database>, cx: &mut Context<Self>) -> Self {
        let repository = Arc::new(SqliteHostRepository::new(database.pool().clone()));
        let focus_handle = cx.focus_handle();

        // 初始化默认展开所有分组
        let expanded_groups = Arc::new(RwLock::new(HashSet::new()));
        let hosts = Arc::new(RwLock::new(Vec::new()));

        let view = Self {
            database,
            repository: repository.clone(),
            hosts: hosts.clone(),
            selected_host_id: None,
            search_query: String::new(),
            last_click: None,
            expanded_groups,
            context_menu_host: None,
            focus_handle,
            connection_dialog: Arc::new(Mutex::new(None)),
            delete_dialog: None,
            pending_delete: None,
        };

        // 异步加载主机列表
        Self::load_hosts_async(repository, hosts);

        view
    }

    /// 异步加载主机列表（静态方法，在独立线程中执行）
    fn load_hosts_async(
        repository: Arc<SqliteHostRepository>,
        hosts: Arc<RwLock<Vec<HostConfig>>>,
    ) {
        // 使用共享 Runtime 执行异步加载
        runtime::spawn_blocking(async move {
            tracing::info!("开始异步加载主机列表");

            match repository.list_all().await {
                Ok(loaded_hosts) => {
                    tracing::info!("成功加载 {} 个主机", loaded_hosts.len());

                    // 更新共享状态
                    if let Ok(mut hosts_guard) = hosts.write() {
                        *hosts_guard = loaded_hosts;
                    } else {
                        tracing::error!("无法获取主机列表写锁");
                    }
                },
                Err(e) => {
                    tracing::error!("加载主机列表失败: {:?}", e);
                },
            }
        });
    }

    /// 刷新主机列表
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        tracing::info!("刷新主机列表");
        Self::load_hosts_async(self.repository.clone(), self.hosts.clone());
        cx.notify();
    }

    /// 选择主机
    pub fn select_host(&mut self, host_id: HostId, cx: &mut Context<Self>) {
        self.selected_host_id = Some(host_id);
        cx.notify();
    }

    /// 获取选中的主机
    pub fn selected_host(&self) -> Option<HostConfig> {
        let hosts = self.hosts.read().ok()?;
        self.selected_host_id
            .and_then(|id| hosts.iter().find(|h| h.id == Some(id)).cloned())
    }

    /// 处理主机点击（包含双击检测）
    fn handle_host_click(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        let host_id = match host.id {
            Some(id) => id,
            None => return,
        };

        let now = Instant::now();
        let is_double_click = if let Some((last_id, last_time)) = self.last_click {
            last_id == host_id && now.duration_since(last_time).as_millis() < 500
        } else {
            false
        };

        if is_double_click {
            // 双击：触发连接
            tracing::info!("双击连接主机: {}", host.name);
            cx.emit(HostListEvent::ConnectRequested(host.clone()));
            self.last_click = None; // 重置点击状态
        } else {
            // 单击：选择主机并记录点击时间
            self.select_host(host_id, cx);
            self.last_click = Some((host_id, now));
        }
    }

    /// 切换分组展开/折叠状态
    fn toggle_group(&mut self, group_name: &str, cx: &mut Context<Self>) {
        if let Ok(mut groups) = self.expanded_groups.write() {
            if groups.contains(group_name) {
                groups.remove(group_name);
            } else {
                groups.insert(group_name.to_string());
            }
        }
        cx.notify();
    }

    /// 检查分组是否展开
    fn is_group_expanded(&self, group_name: &str) -> bool {
        self.expanded_groups
            .read()
            .map(|groups| groups.contains(group_name))
            .unwrap_or(true) // 默认展开
    }

    /// 显示右键菜单
    fn show_context_menu(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        self.context_menu_host = Some(host.clone());
        cx.notify();
    }

    /// 隐藏右键菜单
    fn hide_context_menu(&mut self, cx: &mut Context<Self>) {
        self.context_menu_host = None;
        cx.notify();
    }

    /// 处理新建主机
    fn handle_new_host(&mut self, cx: &mut Context<Self>) {
        tracing::info!("打开新建主机对话框");

        let repository = self.repository.clone();
        let hosts = self.hosts.clone();
        let dialog_ref = self.connection_dialog.clone();
        let dialog_ref_for_cancel = self.connection_dialog.clone();

        let dialog = cx.new(|cx| {
            HostConnectionDialog::new_create(cx)
                .with_on_save(move |config| {
                    Self::save_host_async(repository.clone(), hosts.clone(), config, true);
                    // 关闭对话框
                    *dialog_ref.lock() = None;
                })
                .with_on_cancel(move || {
                    tracing::info!("取消新建主机");
                    // 关闭对话框
                    *dialog_ref_for_cancel.lock() = None;
                })
        });

        // 存储对话框引用
        *self.connection_dialog.lock() = Some(dialog);

        cx.notify();
    }

    /// 处理编辑主机
    fn handle_edit_host(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        tracing::info!("打开编辑主机对话框: {}", host.name);

        let repository = self.repository.clone();
        let hosts = self.hosts.clone();
        let host_clone = host.clone();
        let dialog_ref = self.connection_dialog.clone();
        let dialog_ref_for_cancel = self.connection_dialog.clone();

        let dialog = cx.new(|cx| {
            HostConnectionDialog::new_edit(host_clone, cx)
                .with_on_save(move |config| {
                    Self::save_host_async(repository.clone(), hosts.clone(), config, false);
                    // 关闭对话框
                    *dialog_ref.lock() = None;
                })
                .with_on_cancel(move || {
                    tracing::info!("取消编辑主机");
                    // 关闭对话框
                    *dialog_ref_for_cancel.lock() = None;
                })
        });

        // 存储对话框引用
        *self.connection_dialog.lock() = Some(dialog);

        cx.notify();
    }

    /// 处理删除主机 - 显示确认对话框
    fn handle_delete_host(&mut self, host: &HostConfig, cx: &mut Context<Self>) {
        let host_id = match host.id {
            Some(id) => id,
            None => {
                tracing::warn!("无法删除没有 ID 的主机");
                return;
            },
        };

        let host_name = host.name.clone();
        tracing::info!("请求删除主机: {} (ID: {})", host_name, host_id);

        // 保存待删除的主机信息
        self.pending_delete = Some((host_id, host_name.clone()));

        // 创建删除确认对话框
        let dialog = cx.new(|cx| DeleteConfirmDialog::for_host(host_id, host_name, cx));

        // 订阅对话框事件
        cx.subscribe(&dialog, Self::on_delete_dialog_event).detach();

        self.delete_dialog = Some(dialog);
        cx.notify();
    }

    /// 处理删除确认对话框事件
    fn on_delete_dialog_event(
        &mut self,
        _dialog: Entity<DeleteConfirmDialog>,
        event: &DeleteConfirmEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            DeleteConfirmEvent::Confirmed(target) => {
                if let Some(host_id) = target.host_id() {
                    let host_name = target.host_name().unwrap_or("unknown").to_string();
                    self.do_delete_host(host_id, host_name, cx);
                }
            },
            DeleteConfirmEvent::Cancelled => {
                tracing::info!("用户取消删除操作");
            },
        }

        // 关闭对话框
        self.delete_dialog = None;
        self.pending_delete = None;
        cx.notify();
    }

    /// 执行实际的删除操作
    fn do_delete_host(&mut self, host_id: HostId, host_name: String, cx: &mut Context<Self>) {
        tracing::info!("执行删除主机: {} (ID: {})", host_name, host_id);

        let repository = self.repository.clone();
        let hosts = self.hosts.clone();

        // 使用共享 Runtime 执行异步删除
        runtime::spawn_blocking(async move {
            match repository.delete(host_id).await {
                Ok(_) => {
                    tracing::info!("成功删除主机: {}", host_name);

                    // 从内存中移除
                    if let Ok(mut hosts_guard) = hosts.write() {
                        hosts_guard.retain(|h| h.id != Some(host_id));
                    }
                },
                Err(e) => {
                    tracing::error!("删除主机失败: {:?}", e);
                },
            }
        });

        // 发出删除事件
        cx.emit(HostListEvent::HostDeleted(host_id));
        cx.notify();
    }

    /// 异步保存主机配置（静态方法）
    fn save_host_async(
        repository: Arc<SqliteHostRepository>,
        hosts: Arc<RwLock<Vec<HostConfig>>>,
        config: HostConfig,
        is_new: bool,
    ) {
        let host_name = config.name.clone();

        if is_new {
            tracing::info!("保存新主机: {}", host_name);
        } else {
            tracing::info!("更新主机: {}", host_name);
        }

        // 使用共享 Runtime 执行异步保存
        runtime::spawn_blocking(async move {
            if is_new {
                // 新建主机
                match repository.create(&config).await {
                    Ok(new_id) => {
                        tracing::info!("成功创建主机: {} (ID: {})", host_name, new_id);

                        // 创建包含新 ID 的配置
                        let mut saved_config = config.clone();
                        saved_config.id = Some(new_id);

                        // 添加到内存列表
                        if let Ok(mut hosts_guard) = hosts.write() {
                            hosts_guard.push(saved_config);
                        }
                    },
                    Err(e) => {
                        tracing::error!("创建主机失败: {:?}", e);
                    },
                }
            } else {
                // 更新主机
                match repository.update(&config).await {
                    Ok(_) => {
                        tracing::info!("成功更新主机: {}", host_name);

                        // 更新内存中的配置
                        if let Ok(mut hosts_guard) = hosts.write() {
                            if let Some(pos) = hosts_guard.iter().position(|h| h.id == config.id) {
                                hosts_guard[pos] = config.clone();
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("更新主机失败: {:?}", e);
                    },
                }
            }
        });
    }

    /// 渲染分组标题
    fn render_group_header(
        &self,
        group_name: &str,
        count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_expanded = self.is_group_expanded(group_name);
        let group_name_owned = group_name.to_string();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .cursor_pointer()
            .hover(|this| this.bg(gpui::rgb(0x374151)))
            .on_mouse_down(gpui::MouseButton::Left, {
                let group_name = group_name_owned.clone();
                cx.listener(move |this, _event, _window, cx| {
                    this.toggle_group(&group_name, cx);
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x9ca3af))
                            .child(if is_expanded { "▼" } else { "▶" }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(gpui::rgb(0xd1d5db))
                            .child(group_name_owned),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x6b7280))
                            .child(format!("({})", count)),
                    ),
            )
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
            .on_mouse_down(gpui::MouseButton::Left, {
                let host = host.clone();
                cx.listener(move |this, _event, _window, cx| {
                    this.handle_host_click(&host, cx);
                })
            })
            .on_mouse_down(gpui::MouseButton::Right, {
                let host = host.clone();
                cx.listener(move |this, _event, _window, cx| {
                    this.show_context_menu(&host, cx);
                })
            })
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

    /// 渲染右键菜单
    fn render_context_menu(&self, host: &HostConfig, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.hide_context_menu(cx);
                }),
            )
            .child(
                div()
                    .absolute()
                    .top(gpui::px(100.0))
                    .left(gpui::px(200.0))
                    .w(gpui::px(200.0))
                    .bg(gpui::rgb(0x1f2937))
                    .border_1()
                    .border_color(gpui::rgb(0x374151))
                    .rounded_md()
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                // 连接
                                div()
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(gpui::rgb(0x374151)))
                                    .on_mouse_down(gpui::MouseButton::Left, {
                                        let host = host.clone();
                                        cx.listener(move |this, _event, _window, cx| {
                                            cx.emit(HostListEvent::ConnectRequested(host.clone()));
                                            this.hide_context_menu(cx);
                                        })
                                    })
                                    .child(div().text_sm().text_color(gpui::white()).child("连接")),
                            )
                            .child(
                                // 编辑
                                div()
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(gpui::rgb(0x374151)))
                                    .on_mouse_down(gpui::MouseButton::Left, {
                                        let host = host.clone();
                                        cx.listener(move |this, _event, _window, cx| {
                                            this.handle_edit_host(&host, cx);
                                            this.hide_context_menu(cx);
                                        })
                                    })
                                    .child(div().text_sm().text_color(gpui::white()).child("编辑")),
                            )
                            .child(
                                // 删除
                                div()
                                    .px_4()
                                    .py_2()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(gpui::rgb(0xdc2626)))
                                    .on_mouse_down(gpui::MouseButton::Left, {
                                        let host = host.clone();
                                        cx.listener(move |this, _event, _window, cx| {
                                            this.handle_delete_host(&host, cx);
                                            this.hide_context_menu(cx);
                                        })
                                    })
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(gpui::rgb(0xfca5a5))
                                            .child("删除"),
                                    ),
                            ),
                    ),
            )
    }
}

impl Render for HostListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = div()
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
                // 主机列表（按分组显示）
                let mut list_div = div().flex().flex_col().flex_1().px_2().py_2().gap_1();

                // 从共享状态读取主机列表
                if let Ok(hosts) = self.hosts.read() {
                    // 按分组组织主机
                    let mut groups: BTreeMap<String, Vec<&HostConfig>> = BTreeMap::new();

                    for host in hosts.iter() {
                        let group_name = host.group.as_deref().unwrap_or("默认分组");
                        groups
                            .entry(group_name.to_string())
                            .or_insert_with(Vec::new)
                            .push(host);
                    }

                    // 渲染每个分组
                    for (group_name, group_hosts) in groups.iter() {
                        // 渲染分组标题
                        list_div = list_div.child(self.render_group_header(
                            group_name,
                            group_hosts.len(),
                            cx,
                        ));

                        // 如果分组展开，渲染主机列表
                        if self.is_group_expanded(group_name) {
                            for host in group_hosts {
                                list_div = list_div.child(
                                    div()
                                        .pl_4() // 缩进以显示层级关系
                                        .child(self.render_host_item(host, cx)),
                                );
                            }
                        }
                    }
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
                    .child(div().text_xs().text_color(gpui::rgb(0x9ca3af)).child({
                        let count = self.hosts.read().map(|h| h.len()).unwrap_or(0);
                        format!("共{} 个主机", count)
                    })),
            );

        // 如果有右键菜单，添加到根元素
        if let Some(ref host) = self.context_menu_host {
            root = root.child(self.render_context_menu(host, cx));
        }

        // 如果有连接对话框，添加到根元素（作为覆盖层）
        if let Some(ref dialog) = *self.connection_dialog.lock() {
            root = root.child(dialog.clone());
        }

        // 如果有删除确认对话框，添加到根元素（作为覆盖层）
        if let Some(ref dialog) = self.delete_dialog {
            root = root.child(dialog.clone());
        }

        root
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
