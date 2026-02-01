//! 终端面板管理器

use crate::app::session::SessionCoordinator;
use crate::ui::split_pane::PaneId;
use crate::ui::tab_manager::TabId;
use crate::ui::terminal_view::TerminalView;
use gpui::Entity;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct TerminalPaneData {
    pub terminal_view: Entity<TerminalView>,
    pub coordinator: Arc<SessionCoordinator>,
    pub tab_id: TabId,
}

pub struct TerminalPaneManager {
    terminal_panes: HashMap<PaneId, TerminalPaneData>,
}

impl TerminalPaneManager {
    pub fn new() -> Self {
        Self {
            terminal_panes: HashMap::new(),
        }
    }

    pub fn add_pane(&mut self, pane_id: PaneId, data: TerminalPaneData) {
        self.terminal_panes.insert(pane_id, data);
    }

    pub fn remove_pane(&mut self, pane_id: PaneId) -> Option<TerminalPaneData> {
        self.terminal_panes.remove(&pane_id)
    }

    pub fn get_terminal_view(&self, pane_id: PaneId) -> Option<&Entity<TerminalView>> {
        self.terminal_panes.get(&pane_id).map(|d| &d.terminal_view)
    }

    pub fn get_coordinator(&self, pane_id: PaneId) -> Option<&Arc<SessionCoordinator>> {
        self.terminal_panes.get(&pane_id).map(|d| &d.coordinator)
    }

    pub fn is_connected(&self) -> bool {
        !self.terminal_panes.is_empty()
    }

    pub fn pane_ids_for_tab(&self, tab_id: TabId) -> Vec<PaneId> {
        self.terminal_panes
            .iter()
            .filter(|(_, d)| d.tab_id == tab_id)
            .map(|(id, _)| *id)
            .collect()
    }

    /// 获取指定 Tab 的所有 coordinator
    pub fn get_coordinators_for_tab(&self, tab_id: TabId) -> Vec<Arc<SessionCoordinator>> {
        self.terminal_panes
            .iter()
            .filter(|(_, d)| d.tab_id == tab_id)
            .map(|(_, d)| d.coordinator.clone())
            .collect()
    }

    /// 清理 Tab 相关资源
    ///
    /// 返回被移除的 pane 数据，调用者负责关闭连接
    pub fn cleanup_tab(&mut self, tab_id: TabId) -> Vec<TerminalPaneData> {
        let pane_ids: Vec<_> = self.pane_ids_for_tab(tab_id);
        let mut removed = Vec::new();
        for pane_id in pane_ids {
            if let Some(data) = self.terminal_panes.remove(&pane_id) {
                removed.push(data);
            }
        }
        removed
    }
}
