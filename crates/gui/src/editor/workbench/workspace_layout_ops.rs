use super::layout::{WorkspacePanelId, WorkspacePanelRect};
use super::*;

impl EditorWorkbench {
    pub(crate) fn sync_workspace_layout_from_flags(&mut self) {
        let flags = self.workspace_panel_flags();
        sync_layout_flags(&mut self.workspace_layout, flags);
    }

    pub(super) fn workspace_layout_from_current_flags(&self) -> super::layout::WorkspaceLayout {
        let mut layout = self.workspace_layout.clone();
        sync_layout_flags(&mut layout, self.workspace_panel_flags());
        layout
    }

    fn workspace_panel_flags(&self) -> WorkspacePanelFlags {
        WorkspacePanelFlags {
            show_graph: self.show_graph,
            show_inspector: self.show_inspector,
            show_timeline: self.show_timeline,
            show_asset_browser: self.show_asset_browser,
            show_validation: self.show_validation,
            validation_collapsed: self.validation_collapsed,
            node_editor_window_open: self.node_editor_window_open,
        }
    }

    pub(super) fn apply_workspace_layout_flags(&mut self) {
        self.workspace_layout.normalize();
        self.show_asset_browser = self
            .workspace_layout
            .panel(WorkspacePanelId::AssetBrowser)
            .visible;
        self.show_graph = self.workspace_layout.panel(WorkspacePanelId::Graph).visible;
        self.show_inspector = self
            .workspace_layout
            .panel(WorkspacePanelId::Inspector)
            .visible;
        self.show_validation = self
            .workspace_layout
            .panel(WorkspacePanelId::Validation)
            .visible;
        self.validation_collapsed = self
            .workspace_layout
            .panel(WorkspacePanelId::Validation)
            .collapsed;
        self.show_timeline = self
            .workspace_layout
            .panel(WorkspacePanelId::Timeline)
            .visible;
        self.node_editor_window_open = self
            .workspace_layout
            .panel(WorkspacePanelId::NodeEditor)
            .visible;
    }
}

#[derive(Clone, Copy, Debug)]
struct WorkspacePanelFlags {
    show_graph: bool,
    show_inspector: bool,
    show_timeline: bool,
    show_asset_browser: bool,
    show_validation: bool,
    validation_collapsed: bool,
    node_editor_window_open: bool,
}

fn sync_layout_flags(layout: &mut super::layout::WorkspaceLayout, flags: WorkspacePanelFlags) {
    layout.normalize();
    layout.set_visible(WorkspacePanelId::AssetBrowser, flags.show_asset_browser);
    layout.set_visible(WorkspacePanelId::Graph, flags.show_graph);
    layout.set_visible(WorkspacePanelId::Inspector, flags.show_inspector);
    layout.set_visible(WorkspacePanelId::Validation, flags.show_validation);
    layout.set_collapsed(WorkspacePanelId::Validation, flags.validation_collapsed);
    layout.set_visible(WorkspacePanelId::Timeline, flags.show_timeline);
    layout.set_visible(WorkspacePanelId::NodeEditor, flags.node_editor_window_open);
    if flags.node_editor_window_open
        && layout
            .panel(WorkspacePanelId::NodeEditor)
            .floating_rect
            .is_none()
    {
        layout.set_floating_rect(
            WorkspacePanelId::NodeEditor,
            WorkspacePanelRect {
                x: 80.0,
                y: 80.0,
                w: 1000.0,
                h: 700.0,
            },
        );
    }
}
