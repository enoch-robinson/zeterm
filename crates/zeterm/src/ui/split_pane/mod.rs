//! 分屏面板组件
//!
//! 支持水平和垂直分屏布局，用于多个终端视图的并排显示。

use gpui::{Context, EventEmitter};
use uuid::Uuid;
use zeterm_core::entities::HostConfig;

/// 面板 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(Uuid);

impl PaneId {
    /// 创建新的面板 ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PaneContent {
    fn default() -> Self {
        PaneContent::Terminal {
            pane_id: PaneId::new(),
            title: String::new(),
            focused: false,
        }
    }
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 分屏方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// 水平分屏（左右排列）
    Horizontal,
    /// 垂直分屏（上下排列）
    Vertical,
}

impl SplitDirection {
    /// 获取方向描述
    pub fn description(&self) -> &'static str {
        match self {
            SplitDirection::Horizontal => "水平分屏",
            SplitDirection::Vertical => "垂直分屏",
        }
    }

    /// 切换方向
    pub fn toggle(&self) -> Self {
        match self {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        }
    }
}

/// 面板内容
///
/// 可以是一个终端视图，或者是另一个分屏容器（递归）
#[derive(Debug, Clone)]
pub enum PaneContent {
    /// 终端视图
    Terminal {
        /// 终端视图实体 ID（在 MainWindow 中维护）
        pane_id: PaneId,
        /// 标题
        title: String,
        /// 是否有焦点
        focused: bool,
    },
    /// 分屏容器
    Split {
        /// 分屏方向
        direction: SplitDirection,
        /// 第一个面板
        first: Box<Pane>,
        /// 第二个面板
        second: Box<Pane>,
        /// 分割比例（0.0 - 1.0，first 占据的比例）
        ratio: f32,
    },
}

/// 面板
///
/// 表示分屏布局中的一个节点，可以是终端或子分屏
#[derive(Debug, Clone, Default)]
pub struct Pane {
    /// 面板 ID
    pub id: PaneId,
    /// 面板内容
    pub content: PaneContent,
}

impl Pane {
    /// 创建新的终端面板
    pub fn new_terminal(title: impl Into<String>) -> Self {
        Self {
            id: PaneId::new(),
            content: PaneContent::Terminal {
                pane_id: PaneId::new(), // 实际的 TerminalView 引用在外部维护
                title: title.into(),
                focused: false,
            },
        }
    }

    /// 创建带主机配置的终端面板
    pub fn new_ssh(host_config: &HostConfig) -> Self {
        let title = format!("{}@{}", host_config.username, host_config.host);
        Self {
            id: PaneId::new(),
            content: PaneContent::Terminal {
                pane_id: PaneId::new(),
                title,
                focused: false,
            },
        }
    }

    /// 创建分屏面板
    pub fn new_split(direction: SplitDirection, first: Pane, second: Pane, ratio: f32) -> Self {
        Self {
            id: PaneId::new(),
            content: PaneContent::Split {
                direction,
                first: Box::new(first),
                second: Box::new(second),
                ratio: ratio.clamp(0.1, 0.9),
            },
        }
    }

    /// 设置焦点
    pub fn set_focused(&mut self, focused: bool) {
        match &mut self.content {
            PaneContent::Terminal { focused: f, .. } => {
                *f = focused;
            },
            PaneContent::Split { first, second, .. } => {
                if focused {
                    // 递归设置：如果需要焦点，默认设置到第一个面板
                    first.set_focused(true);
                    second.set_focused(false);
                } else {
                    first.set_focused(false);
                    second.set_focused(false);
                }
            },
        }
    }

    /// 获取焦点状态
    pub fn is_focused(&self) -> bool {
        match &self.content {
            PaneContent::Terminal { focused, .. } => *focused,
            PaneContent::Split { first, .. } => first.is_focused(),
        }
    }

    /// 获取所有终端面板 ID
    pub fn collect_terminal_panes(&self) -> Vec<PaneId> {
        let mut panes = Vec::new();
        self.collect_terminal_panes_recursive(&mut panes);
        panes
    }

    fn collect_terminal_panes_recursive(&self, panes: &mut Vec<PaneId>) {
        match &self.content {
            PaneContent::Terminal { pane_id, .. } => {
                panes.push(*pane_id);
            },
            PaneContent::Split { first, second, .. } => {
                first.collect_terminal_panes_recursive(panes);
                second.collect_terminal_panes_recursive(panes);
            },
        }
    }
}

/// 分屏管理器
///
/// 管理整个分屏布局和焦点状态
pub struct SplitManager {
    /// 根面板
    root: Option<Pane>,
    /// 焦点面板 ID
    focused_pane: Option<PaneId>,
    /// 面板数量（用于自动生成 ID）
    pane_count: usize,
}

impl SplitManager {
    /// 创建新的分屏管理器
    pub fn new() -> Self {
        Self {
            root: None,
            focused_pane: None,
            pane_count: 0,
        }
    }

    /// 设置根面板
    pub fn set_root(&mut self, pane: Pane, cx: &mut Context<Self>) {
        self.root = Some(pane);
        self.pane_count = 1;
        cx.emit(SplitManagerEvent::LayoutChanged);
        cx.notify();
    }

    /// 水平分屏（左右）
    pub fn split_horizontal(&mut self, pane_id: PaneId, cx: &mut Context<Self>) -> Option<PaneId> {
        self.split_pane(pane_id, SplitDirection::Horizontal, 0.5, cx)
    }

    /// 垂直分屏（上下）
    pub fn split_vertical(&mut self, pane_id: PaneId, cx: &mut Context<Self>) -> Option<PaneId> {
        self.split_pane(pane_id, SplitDirection::Vertical, 0.5, cx)
    }

    /// 使用已创建好的 Pane 进行分屏
    ///
    /// 与 split_horizontal/split_vertical 不同，此方法接受外部创建的 Pane，
    /// 允许调用方在分屏前完成 TerminalView 等资源的创建和关联。
    ///
    /// # Arguments
    /// * `target_pane_id` - 要分屏的目标面板 ID
    /// * `new_pane` - 已创建好的新面板
    /// * `direction` - 分屏方向
    /// * `ratio` - 分屏比例（0.1 - 0.9）
    pub fn split_with_pane(
        &mut self,
        target_pane_id: PaneId,
        new_pane: Pane,
        direction: SplitDirection,
        ratio: f32,
        cx: &mut Context<Self>,
    ) -> Option<PaneId> {
        let root = self.root.as_mut()?;
        let new_pane_id = new_pane.id;

        // 查找目标面板并替换为分屏结构
        if let Some(target_pane) = Self::find_and_remove_pane(root, target_pane_id) {
            let split_pane =
                Pane::new_split(direction, target_pane, new_pane, ratio.clamp(0.1, 0.9));
            *root = split_pane;
            self.pane_count += 1;
            self.focused_pane = Some(new_pane_id);

            cx.emit(SplitManagerEvent::PaneSplit(new_pane_id));
            cx.emit(SplitManagerEvent::LayoutChanged);
            cx.notify();

            Some(new_pane_id)
        } else {
            None
        }
    }

    /// 分屏指定面板
    fn split_pane(
        &mut self,
        pane_id: PaneId,
        direction: SplitDirection,
        ratio: f32,
        cx: &mut Context<Self>,
    ) -> Option<PaneId> {
        let root = self.root.as_mut()?;

        // 查找并分割目标面板
        let (old_pane, new_pane) = if let Some(pane) = Self::find_and_remove_pane(root, pane_id) {
            let new_pane = Pane::new_terminal(format!("Terminal {}", self.pane_count + 1));
            let split_pane = Pane::new_split(direction, pane, new_pane.clone(), ratio);
            self.pane_count += 1;
            (split_pane, new_pane.id)
        } else {
            return None;
        };

        *root = old_pane;
        self.focused_pane = Some(new_pane);
        cx.emit(SplitManagerEvent::PaneSplit(new_pane));
        cx.emit(SplitManagerEvent::LayoutChanged);
        cx.notify();

        Some(new_pane)
    }

    /// 查找并移除面板
    fn find_and_remove_pane(root: &mut Pane, pane_id: PaneId) -> Option<Pane> {
        if root.id == pane_id {
            // 找到了，但我们需要替换它
            // 返回一个新的终端面板作为占位符
            let replacement = Pane::new_terminal("Terminal");
            let original = std::mem::replace(root, replacement);
            Some(original)
        } else {
            match &mut root.content {
                PaneContent::Split { first, second, .. } => {
                    // 先在第一个面板中查找
                    if let Some(pane) = Self::find_and_remove_pane(first, pane_id) {
                        return Some(pane);
                    }
                    // 然后在第二个面板中查找
                    Self::find_and_remove_pane(second, pane_id)
                },
                PaneContent::Terminal { .. } => None,
            }
        }
    }

    /// 关闭面板
    pub fn close_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) -> bool {
        if self.root.is_none() {
            return false;
        }

        // 如果只有一个面板，不允许关闭
        if self.pane_count <= 1 {
            return false;
        }

        // 查找并移除面板
        let root = self.root.as_mut().unwrap();
        if Self::remove_pane_recursive(root, pane_id) {
            self.pane_count -= 1;
            // 重新设置焦点
            self.focus_next_pane(cx);
            cx.emit(SplitManagerEvent::PaneClosed(pane_id));
            cx.emit(SplitManagerEvent::LayoutChanged);
            cx.notify();
            true
        } else {
            false
        }
    }

    /// 递归移除面板
    fn remove_pane_recursive(pane: &mut Pane, pane_id: PaneId) -> bool {
        if pane.id == pane_id {
            return true;
        }

        match &mut pane.content {
            PaneContent::Terminal { .. } => false,
            PaneContent::Split { first, second, .. } => {
                if Self::remove_pane_recursive(first, pane_id) {
                    // 如果第一个子面板被移除，用第二个替换当前分屏
                    let replacement = std::mem::take(second.as_mut());
                    *pane = replacement;
                    true
                } else if Self::remove_pane_recursive(second, pane_id) {
                    // 如果第二个子面板被移除，用第一个替换当前分屏
                    let replacement = std::mem::take(first.as_mut());
                    *pane = replacement;
                    true
                } else {
                    false
                }
            },
        }
    }

    /// 切换焦点到指定面板
    pub fn focus_pane(&mut self, pane_id: PaneId, cx: &mut Context<Self>) -> bool {
        if let Some(root) = &mut self.root {
            if Self::set_focus_recursive(root, pane_id, true) {
                self.focused_pane = Some(pane_id);
                cx.emit(SplitManagerEvent::FocusChanged(pane_id));
                cx.notify();
                return true;
            }
        }
        false
    }

    /// 递归设置焦点
    fn set_focus_recursive(pane: &mut Pane, pane_id: PaneId, focused: bool) -> bool {
        if pane.id == pane_id {
            pane.set_focused(focused);
            true
        } else {
            match &mut pane.content {
                PaneContent::Terminal { .. } => false,
                PaneContent::Split { first, second, .. } => {
                    let found = Self::set_focus_recursive(first, pane_id, focused)
                        || Self::set_focus_recursive(second, pane_id, focused);
                    if found && !focused {
                        // 清除所有子面板的焦点
                        first.set_focused(false);
                        second.set_focused(false);
                    }
                    found
                },
            }
        }
    }

    /// 焦点移动到下一个面板
    pub fn focus_next_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(root) = &self.root {
            let panes = root.collect_terminal_panes();
            if let Some(current) = self.focused_pane {
                if let Some(pos) = panes.iter().position(|&id| id == current) {
                    let next = panes[(pos + 1) % panes.len()];
                    self.focus_pane(next, cx);
                    return;
                }
            }
            // 如果没有当前焦点，聚焦到第一个面板
            if let Some(&first) = panes.first() {
                self.focus_pane(first, cx);
            }
        }
    }

    /// 焦点移动到上一个面板
    pub fn focus_prev_pane(&mut self, cx: &mut Context<Self>) {
        if let Some(root) = &self.root {
            let panes = root.collect_terminal_panes();
            if let Some(current) = self.focused_pane {
                if let Some(pos) = panes.iter().position(|&id| id == current) {
                    let prev_idx = if pos == 0 { panes.len() - 1 } else { pos - 1 };
                    let prev = panes[prev_idx];
                    self.focus_pane(prev, cx);
                    return;
                }
            }
            if let Some(&first) = panes.first() {
                self.focus_pane(first, cx);
            }
        }
    }

    /// 获取根面板
    pub fn root(&self) -> Option<&Pane> {
        self.root.as_ref()
    }

    /// 获取焦点面板 ID
    pub fn focused_pane(&self) -> Option<PaneId> {
        self.focused_pane
    }

    /// 获取面板数量
    pub fn pane_count(&self) -> usize {
        self.pane_count
    }

    /// 是否有面板
    pub fn has_panes(&self) -> bool {
        self.root.is_some()
    }

    /// 调整分屏大小（增量）
    pub fn adjust_split_ratio(&mut self, pane_id: PaneId, delta: f32, cx: &mut Context<Self>) {
        if let Some(root) = &mut self.root {
            if Self::adjust_ratio_recursive(root, pane_id, delta) {
                cx.emit(SplitManagerEvent::LayoutChanged);
                cx.notify();
            }
        }
    }

    /// 设置分屏比例（绝对值）
    pub fn set_split_ratio(&mut self, pane_id: PaneId, new_ratio: f32, cx: &mut Context<Self>) {
        if let Some(root) = &mut self.root {
            if Self::set_ratio_recursive(root, pane_id, new_ratio) {
                cx.emit(SplitManagerEvent::LayoutChanged);
                cx.notify();
            }
        }
    }

    /// 获取分屏比例
    pub fn get_split_ratio(&self, pane_id: PaneId) -> Option<f32> {
        self.root
            .as_ref()
            .and_then(|root| Self::get_ratio_recursive(root, pane_id))
    }

    /// 递归获取分屏比例
    fn get_ratio_recursive(pane: &Pane, pane_id: PaneId) -> Option<f32> {
        match &pane.content {
            PaneContent::Terminal { .. } => None,
            PaneContent::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if pane.id == pane_id {
                    Some(*ratio)
                } else {
                    // 递归查找子面板
                    Self::get_ratio_recursive(first, pane_id)
                        .or_else(|| Self::get_ratio_recursive(second, pane_id))
                }
            },
        }
    }

    /// 递归调整分屏比例（增量）
    fn adjust_ratio_recursive(pane: &mut Pane, pane_id: PaneId, delta: f32) -> bool {
        match &mut pane.content {
            PaneContent::Terminal { .. } => false,
            PaneContent::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if pane.id == pane_id {
                    *ratio = (*ratio + delta).clamp(0.1, 0.9);
                    true
                } else {
                    // 递归查找子面板
                    Self::adjust_ratio_recursive(first, pane_id, delta)
                        || Self::adjust_ratio_recursive(second, pane_id, delta)
                }
            },
        }
    }

    /// 递归设置分屏比例（绝对值）
    fn set_ratio_recursive(pane: &mut Pane, pane_id: PaneId, new_ratio: f32) -> bool {
        match &mut pane.content {
            PaneContent::Terminal { .. } => false,
            PaneContent::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if pane.id == pane_id {
                    *ratio = new_ratio.clamp(0.1, 0.9);
                    true
                } else {
                    // 递归查找子面板
                    Self::set_ratio_recursive(first, pane_id, new_ratio)
                        || Self::set_ratio_recursive(second, pane_id, new_ratio)
                }
            },
        }
    }
}

impl Default for SplitManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 分屏管理器事件
#[derive(Clone, Debug)]
pub enum SplitManagerEvent {
    /// 面板已分割
    PaneSplit(PaneId),
    /// 面板已关闭
    PaneClosed(PaneId),
    /// 焦点已改变
    FocusChanged(PaneId),
    /// 布局已改变
    LayoutChanged,
}

impl EventEmitter<SplitManagerEvent> for SplitManager {}

mod split_view;

pub use split_view::SplitView;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_id_creation() {
        let id1 = PaneId::new();
        let id2 = PaneId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_split_direction_toggle() {
        assert_eq!(
            SplitDirection::Horizontal.toggle(),
            SplitDirection::Vertical
        );
        assert_eq!(
            SplitDirection::Vertical.toggle(),
            SplitDirection::Horizontal
        );
    }

    #[test]
    fn test_pane_creation() {
        let pane = Pane::new_terminal("Test Terminal");
        assert!(matches!(pane.content, PaneContent::Terminal { .. }));
    }
}
