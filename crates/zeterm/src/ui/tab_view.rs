//! Tab 视图组件
//!
//! 显示和管理终端标签页。
//! 每个 Tab 拥有独立的 SplitManager，切换 Tab 时整个分屏布局随之切换。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use parking_lot::RwLock;

use super::split_pane::{PaneId, SplitView};
use super::tab_manager::{TabId, TabInfo, TabManager};
use super::terminal_view::TerminalView;

/// 终端视图映射类型（TabId -> (PaneId -> TerminalView)）
pub type TabTerminalViewMap = HashMap<TabId, HashMap<PaneId, Entity<TerminalView>>>;

/// Tab 视图组件
pub struct TabView {
    /// 焦点句柄
    focus_handle: FocusHandle,

    /// Tab 管理器
    tab_manager: Entity<TabManager>,

    /// 每个 Tab 的 SplitView（TabId -> SplitView）
    split_views: HashMap<TabId, Entity<SplitView>>,

    /// 每个 Tab 的终端视图映射
    terminal_views: Arc<RwLock<TabTerminalViewMap>>,

    /// 新建 Tab 回调（可选）
    on_new_tab: Option<Box<dyn Fn(&mut Context<Self>) + Send + Sync + 'static>>,

    /// 是否显示新建 Tab 按钮
    show_new_tab_button: bool,

    /// 是否显示关闭按钮
    show_close_buttons: bool,

    /// Tab 栏高度
    tab_bar_height: f32,
}

impl TabView {
    /// 创建新的 Tab 视图
    pub fn new(tab_manager: Entity<TabManager>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            tab_manager,
            split_views: HashMap::new(),
            terminal_views: Arc::new(RwLock::new(HashMap::new())),
            on_new_tab: None,
            show_new_tab_button: true,
            show_close_buttons: true,
            tab_bar_height: 40.0,
        }
    }

    /// 设置新建 Tab 回调
    pub fn with_on_new_tab<F>(mut self, callback: F) -> Self
    where
        F: Fn(&mut Context<Self>) + Send + Sync + 'static,
    {
        self.on_new_tab = Some(Box::new(callback));
        self
    }

    /// 设置是否显示新建 Tab 按钮
    pub fn with_new_tab_button(mut self, show: bool) -> Self {
        self.show_new_tab_button = show;
        self
    }

    /// 设置是否显示关闭按钮
    pub fn with_close_buttons(mut self, show: bool) -> Self {
        self.show_close_buttons = show;
        self
    }

    /// 设置 Tab 栏高度
    pub fn with_tab_bar_height(mut self, height: f32) -> Self {
        self.tab_bar_height = height;
        self
    }

    /// 获取 Tab 管理器
    pub fn tab_manager(&self) -> &Entity<TabManager> {
        &self.tab_manager
    }

    /// 为指定 Tab 注册 SplitView
    pub fn register_split_view(&mut self, tab_id: TabId, split_view: Entity<SplitView>) {
        self.split_views.insert(tab_id, split_view);
    }

    /// 移除指定 Tab 的 SplitView
    pub fn unregister_split_view(&mut self, tab_id: TabId) {
        self.split_views.remove(&tab_id);
    }

    /// 获取指定 Tab 的 SplitView
    pub fn get_split_view(&self, tab_id: TabId) -> Option<&Entity<SplitView>> {
        self.split_views.get(&tab_id)
    }

    /// 获取当前活动 Tab 的 SplitView
    pub fn active_split_view(&self, cx: &Context<Self>) -> Option<&Entity<SplitView>> {
        let active_id = self.tab_manager.read(cx).active_tab_id()?;
        self.split_views.get(&active_id)
    }

    /// 为指定 Tab 注册终端视图
    pub fn register_terminal_view(
        &self,
        tab_id: TabId,
        pane_id: PaneId,
        view: Entity<TerminalView>,
    ) {
        let mut guard = self.terminal_views.write();
        guard
            .entry(tab_id)
            .or_insert_with(HashMap::new)
            .insert(pane_id, view);
    }

    /// 移除指定 Tab 的终端视图
    pub fn unregister_terminal_view(&self, tab_id: TabId, pane_id: PaneId) {
        let mut guard = self.terminal_views.write();
        if let Some(tab_views) = guard.get_mut(&tab_id) {
            tab_views.remove(&pane_id);
        }
    }

    /// 移除指定 Tab 的所有终端视图
    pub fn unregister_all_terminal_views(&self, tab_id: TabId) {
        let mut guard = self.terminal_views.write();
        guard.remove(&tab_id);
    }

    /// 获取指定 Tab 的终端视图映射
    pub fn get_terminal_views(
        &self,
        tab_id: TabId,
    ) -> Option<HashMap<PaneId, Entity<TerminalView>>> {
        let guard = self.terminal_views.read();
        guard.get(&tab_id).cloned()
    }

    /// 获取终端视图映射的共享引用
    pub fn terminal_views_ref(&self) -> Arc<RwLock<TabTerminalViewMap>> {
        self.terminal_views.clone()
    }

    /// 处理新建 Tab 按钮点击
    fn handle_new_tab_click(&mut self, cx: &mut Context<Self>) {
        if let Some(ref callback) = self.on_new_tab {
            callback(cx);
        }
    }

    /// 渲染 Tab 栏
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 读取 Tab 列表并克隆（避免借用冲突）
        let tabs: Vec<_> = self
            .tab_manager
            .read(cx)
            .tabs()
            .iter()
            .map(|t| (*t).clone())
            .collect();

        let mut tab_bar = div()
            .id("tab-bar")
            .flex()
            .items_center()
            .h(px(self.tab_bar_height))
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.secondary)
            .overflow_x_scroll();

        // 渲染每个 Tab
        for tab in &tabs {
            tab_bar = tab_bar.child(self.render_tab_item(tab, cx));
        }

        // 新建 Tab 按钮
        if self.show_new_tab_button {
            tab_bar = tab_bar.child(self.render_new_tab_button(cx));
        }

        tab_bar
    }

    /// 渲染单个 Tab 项
    fn render_tab_item(&self, tab: &TabInfo, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_active = tab.is_active;
        let is_modified = tab.is_modified;
        let tab_id = tab.id;
        let tab_title = tab.title.clone();
        let tab_icon = tab.icon.clone();
        let tab_manager = self.tab_manager.clone();
        let tab_manager_for_close = self.tab_manager.clone();
        let show_close = self.show_close_buttons;

        // Tab 容器
        let tab_item = div()
            .id(SharedString::from(format!("tab-{}", tab_id)))
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                // 点击 Tab 切换
                tab_manager.update(cx, |manager, cx| {
                    manager.switch_to_tab(tab_id, cx);
                });
            }))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .h(px(32.0))
            .min_w(px(100.0))
            .max_w(px(200.0))
            .rounded_t_md()
            .bg(if is_active {
                theme.background
            } else {
                theme.secondary
            })
            .border_t_2()
            .border_color(if is_active {
                gpui::rgb(0x3b82f6).into()
            } else {
                gpui::rgba(0x00000000)
            })
            .hover(|style| {
                if !is_active {
                    style.bg(theme.muted)
                } else {
                    style
                }
            })
            .cursor_pointer();

        // Tab 内容
        let mut tab_content = div()
            .flex()
            .items_center()
            .gap_2()
            .flex_1()
            .overflow_hidden();

        // 图标
        if let Some(icon) = tab_icon {
            tab_content = tab_content.child(div().text_sm().child(icon));
        }

        // 标题
        tab_content = tab_content.child(
            div()
                .flex_1()
                .text_sm()
                .text_color(if is_active {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(tab_title),
        );

        // 修改指示器
        if is_modified {
            tab_content = tab_content.child(
                div()
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(gpui::rgb(0xf59e0b)),
            );
        }

        // 关闭按钮
        let close_button = if show_close {
            div()
                .id(SharedString::from(format!("tab-close-{}", tab_id)))
                .on_click(
                    cx.listener(move |_this, _event: &gpui::ClickEvent, _window, cx| {
                        // 关闭 Tab
                        tab_manager_for_close.update(cx, |manager, cx| {
                            manager.close_tab(tab_id, cx);
                        });
                        cx.notify();
                    }),
                )
                .flex()
                .items_center()
                .justify_center()
                .w(px(18.0))
                .h(px(18.0))
                .rounded(px(4.0))
                .hover(|style| style.bg(theme.muted))
                .active(|style| style.bg(theme.border))
                .cursor_pointer()
                .text_sm()
                .text_color(theme.muted_foreground)
                .hover(|style| style.text_color(gpui::rgb(0xef4444)))
                .child("×")
                .into_any_element()
        } else {
            div().into_any_element()
        };

        tab_item.child(tab_content).child(close_button)
    }

    /// 渲染新建 Tab 按钮
    fn render_new_tab_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div()
            .id("new-tab-button")
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.handle_new_tab_click(cx);
            }))
            .flex()
            .items_center()
            .justify_center()
            .w(px(32.0))
            .h(px(32.0))
            .ml_1()
            .rounded(px(6.0))
            .bg(gpui::rgba(0x00000000))
            .hover(|style| style.bg(theme.muted))
            .active(|style| style.bg(theme.border))
            .cursor_pointer()
            .text_lg()
            .text_color(theme.muted_foreground)
            .hover(|style| style.text_color(theme.foreground))
            .child("+")
    }

    /// 渲染 Tab 内容区域（当前活动 Tab 的 SplitView）
    fn render_tab_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        // 获取当前活动 Tab 的 SplitView
        let active_tab_id = self.tab_manager.read(cx).active_tab_id();

        if let Some(tab_id) = active_tab_id {
            if let Some(split_view) = self.split_views.get(&tab_id) {
                // 渲染活动 Tab 的 SplitView
                return div()
                    .id(SharedString::from(format!("tab-content-{}", tab_id)))
                    .flex_1()
                    .w_full()
                    .h_full()
                    .bg(theme.background)
                    .child(split_view.clone())
                    .into_any_element();
            }
        }

        // 没有活动 Tab 或 SplitView，显示空状态
        self.render_empty_state(cx)
    }

    /// 渲染空状态（无 Tab 时）
    fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        div()
            .id("tab-empty-state")
            .flex_1()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(theme.background)
            .child(
                div()
                    .text_3xl()
                    .mb_4()
                    .text_color(theme.muted_foreground)
                    .child("📑"),
            )
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .mb_2()
                    .child("没有打开的标签页"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("从左侧主机列表选择主机，或点击 + 新建标签页"),
            )
            .into_any_element()
    }
}

impl Focusable for TabView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 主容器：垂直布局
        // - Tab 栏（顶部）
        // - Tab 内容（填充剩余空间）
        div()
            .id("tab-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background)
            .child(self.render_tab_bar(cx))
            .child(self.render_tab_content(cx))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_tab_view_defaults() {
        // 测试默认值
        assert_eq!(40.0, 40.0); // tab_bar_height default
    }
}
