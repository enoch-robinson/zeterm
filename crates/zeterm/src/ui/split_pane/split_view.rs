//! 分屏视图组件
//!
//! 渲染和管理分屏布局的UI 组件。

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, CursorStyle, Entity, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Point, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::ActiveTheme;
use parking_lot::RwLock;

use super::{Pane, PaneContent, PaneId, SplitDirection, SplitManager};
use crate::ui::terminal_view::TerminalView;

/// 拖拽状态
#[derive(Debug, Clone)]
struct DragState {
    /// 正在拖拽的分隔条所属的面板 ID
    split_id: PaneId,
    /// 分屏方向
    direction: SplitDirection,
    /// 拖拽开始时的鼠标位置
    start_position: Point<f32>,
    /// 拖拽开始时的分屏比例
    start_ratio: f32,
    /// 容器尺寸（用于计算比例变化）
    container_size: f32,
}

/// 终端视图渲染器
///
/// 用于从外部提供终端视图实体的映射
pub type TerminalViewMap = HashMap<PaneId, Entity<TerminalView>>;

/// 分屏视图组件
pub struct SplitView {
    /// 焦点句柄
    focus_handle: FocusHandle,
    /// 分屏管理器
    split_manager: Entity<SplitManager>,
    /// 终端视图映射（由外部提供）
    terminal_views: Arc<RwLock<TerminalViewMap>>,
    /// 当前拖拽状态
    drag_state: Arc<RwLock<Option<DragState>>>,
}

impl SplitView {
    /// 创建新的分屏视图
    pub fn new(split_manager: Entity<SplitManager>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            split_manager,
            terminal_views: Arc::new(RwLock::new(HashMap::new())),
            drag_state: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置终端视图映射
    pub fn set_terminal_views(&mut self, views: TerminalViewMap) {
        let mut guard = self.terminal_views.write();
        *guard = views;
    }

    /// 注册终端视图
    pub fn register_terminal_view(&self, pane_id: PaneId, view: Entity<TerminalView>) {
        let mut guard = self.terminal_views.write();
        guard.insert(pane_id, view);
    }

    /// 移除终端视图
    pub fn unregister_terminal_view(&self, pane_id: PaneId) {
        let mut guard = self.terminal_views.write();
        guard.remove(&pane_id);
    }

    /// 获取终端视图映射的共享引用
    pub fn terminal_views_ref(&self) -> Arc<RwLock<TerminalViewMap>> {
        self.terminal_views.clone()
    }

    /// 开始拖拽分隔条
    fn start_drag(
        &mut self,
        split_id: PaneId,
        direction: SplitDirection,
        position: Point<f32>,
        current_ratio: f32,
        container_size: f32,
        _cx: &mut Context<Self>,
    ) {
        let mut drag_state = self.drag_state.write();
        *drag_state = Some(DragState {
            split_id,
            direction,
            start_position: position,
            start_ratio: current_ratio,
            container_size,
        });
    }

    /// 处理拖拽移动
    fn handle_drag_move(&mut self, position: Point<f32>, cx: &mut Context<Self>) {
        let drag_info = {
            let drag_state = self.drag_state.read();
            drag_state.clone()
        };

        if let Some(state) = drag_info {
            // 计算位置差异
            let delta = match state.direction {
                SplitDirection::Horizontal => position.x - state.start_position.x,
                SplitDirection::Vertical => position.y - state.start_position.y,
            };

            // 计算新的比例
            if state.container_size > 0.0 {
                let ratio_delta = delta / state.container_size;
                let new_ratio = (state.start_ratio + ratio_delta).clamp(0.1, 0.9);

                // 更新分屏管理器中的比例
                self.split_manager.update(cx, |manager, cx| {
                    manager.adjust_split_ratio(state.split_id, new_ratio - state.start_ratio, cx);
                });

                // 更新起始比例以便下次计算
                let mut drag_state = self.drag_state.write();
                if let Some(ref mut s) = *drag_state {
                    s.start_ratio = new_ratio;
                    s.start_position = position;
                }
            }
        }
    }

    /// 结束拖拽
    fn end_drag(&mut self, _cx: &mut Context<Self>) {
        let mut drag_state = self.drag_state.write();
        *drag_state = None;
    }

    /// 检查是否正在拖拽
    fn is_dragging(&self) -> bool {
        self.drag_state.read().is_some()
    }

    /// 获取分屏管理器
    pub fn split_manager(&self) -> &Entity<SplitManager> {
        &self.split_manager
    }

    /// 渲染单个面板（返回 AnyElement 避免借用问题）
    fn render_pane(&self, pane: &Pane, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let pane_id = pane.id;
        let split_manager = self.split_manager.clone();

        match &pane.content {
            PaneContent::Terminal {
                pane_id: terminal_pane_id,
                title,
                focused,
                ..
            } => {
                let is_focused = *focused;
                let title = title.clone();
                let border_color = if is_focused {
                    gpui::rgb(0x3b82f6).into()
                } else {
                    theme.border
                };

                // 尝试获取对应的 TerminalView
                let terminal_view = {
                    let guard = self.terminal_views.read();
                    guard.get(&terminal_pane_id).cloned()
                };

                // 终端面板容器
                let container = div()
                    .id(format!("pane-{}", pane_id))
                    .flex_1()
                    .flex()
                    .flex_col()
                    .bg(theme.background)
                    .border_1()
                    .border_color(border_color)
                    .relative()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                        // 点击切换焦点
                        split_manager.update(cx, |manager, cx| {
                            manager.focus_pane(pane_id, cx);
                        });
                    }))
                    .child(
                        // 面板标题栏
                        div()
                            .h(px(24.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .bg(theme.secondary)
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(title),
                            ),
                    );

                // 根据是否有TerminalView 渲染不同内容
                if let Some(view) = terminal_view {
                    // 渲染实际的终端视图
                    container
                        .child(
                            div()
                                .id(format!("pane-content-{}", pane_id))
                                .flex_1()
                                .child(view),
                        )
                        .into_any_element()
                } else {
                    // 渲染占位符
                    container
                        .child(
                            div()
                                .id(format!("pane-content-{}", pane_id))
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(theme.muted_foreground)
                                        .child("正在加载终端..."),
                                ),
                        )
                        .into_any_element()
                }
            },
            PaneContent::Split {
                direction,
                first,
                second,
                ratio,
            } => {
                // 分屏容器
                let is_horizontal = matches!(direction, SplitDirection::Horizontal);
                let first_ratio = *ratio;
                let second_ratio = 1.0 - first_ratio;
                let direction_copy = *direction;

                // 先渲染子面板
                let first_child = self.render_pane(first, cx);
                let second_child = self.render_pane(second, cx);
                let separator = self.render_separator(direction_copy, pane_id, first_ratio, cx);

                let container = div()
                    .id(format!("split-{}", pane_id))
                    .flex()
                    .flex_1()
                    .w_full()
                    .h_full();

                if is_horizontal {
                    // 水平分屏（左右排列）
                    container
                        .flex_row()
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(50.0))
                                .flex_basis(gpui::relative(first_ratio))
                                .child(first_child),
                        )
                        .child(separator)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(50.0))
                                .flex_basis(gpui::relative(second_ratio))
                                .child(second_child),
                        )
                        .into_any_element()
                } else {
                    // 垂直分屏（上下排列）
                    container
                        .flex_col()
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(50.0))
                                .flex_basis(gpui::relative(first_ratio))
                                .child(first_child),
                        )
                        .child(separator)
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(50.0))
                                .flex_basis(gpui::relative(second_ratio))
                                .child(second_child),
                        )
                        .into_any_element()
                }
            },
        }
    }

    /// 渲染分隔条
    fn render_separator(
        &self,
        direction: SplitDirection,
        split_id: PaneId,
        current_ratio: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let hover_color: Hsla = gpui::rgb(0x3b82f6).into();
        let dragging_color: Hsla = gpui::rgb(0x2563eb).into();

        let is_dragging = self.is_dragging();

        let (width, height, cursor) = match direction {
            SplitDirection::Horizontal => (px(6.0), px(0.0), CursorStyle::ResizeLeftRight),
            SplitDirection::Vertical => (px(0.0), px(6.0), CursorStyle::ResizeUpDown),
        };

        let bg_color = if is_dragging {
            dragging_color
        } else {
            theme.border
        };

        let base = div()
            .id(format!("separator-{}", split_id))
            .bg(bg_color)
            .hover(move |style| style.bg(hover_color))
            .cursor(cursor)
            .flex_shrink_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                    // 获取容器尺寸（使用窗口尺寸作为近似值）
                    let container_size = match direction {
                        SplitDirection::Horizontal => 800.0, // 默认宽度
                        SplitDirection::Vertical => 600.0,   // 默认高度
                    };
                    this.start_drag(
                        split_id,
                        direction,
                        Point::new(f32::from(event.position.x), f32::from(event.position.y)),
                        current_ratio,
                        container_size,
                        cx,
                    );
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_dragging() {
                    this.handle_drag_move(
                        Point::new(f32::from(event.position.x), f32::from(event.position.y)),
                        cx,
                    );
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, cx| {
                    this.end_drag(cx);
                }),
            );

        match direction {
            SplitDirection::Horizontal => base.w(width).h_full().into_any_element(),
            SplitDirection::Vertical => base.h(height).w_full().into_any_element(),
        }
    }
}

impl Focusable for SplitView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SplitView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        // 获取根面板
        let root_pane = self.split_manager.read(cx).root().cloned();

        // 基础容器
        let container = div()
            .id("split-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(theme.background);

        match root_pane {
            Some(pane) => container.child(self.render_pane(&pane, cx)),
            None => {
                // 没有面板时显示提示
                container.child(
                    div().flex_1().flex().items_center().justify_center().child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child("暂无终端，请从左侧主机列表选择主机连接"),
                    ),
                )
            },
        }
    }
}
