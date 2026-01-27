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

    pub fn cleanup_tab(&mut self, tab_id: TabId) {
        let pane_ids: Vec<_> = self.pane_ids_for_tab(tab_id);
        for pane_id in pane_ids {
            self.terminal_panes.remove(&pane_id);
        }
    }
}
