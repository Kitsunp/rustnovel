use super::*;

pub const WORKSPACE_LAYOUT_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_DOCK_REFERENCE_WIDTH: f32 = 1280.0;
pub const TOOLBAR_EXPANDED_MIN_WIDTH: f32 = 2400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorToolbarMode {
    Grouped,
    Expanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationReportPlacement {
    Inspector,
    FloatingWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkspacePanelId {
    AssetBrowser,
    Graph,
    Composer,
    Inspector,
    Validation,
    Timeline,
    NodeEditor,
}

impl WorkspacePanelId {
    pub const ALL: [Self; 7] = [
        Self::AssetBrowser,
        Self::Graph,
        Self::Composer,
        Self::Inspector,
        Self::Validation,
        Self::Timeline,
        Self::NodeEditor,
    ];

    fn default_placement(self) -> WorkspacePanelPlacement {
        match self {
            Self::AssetBrowser | Self::Graph => WorkspacePanelPlacement::Left,
            Self::Composer => WorkspacePanelPlacement::Center,
            Self::Inspector => WorkspacePanelPlacement::Right,
            Self::Validation | Self::Timeline => WorkspacePanelPlacement::Bottom,
            Self::NodeEditor => WorkspacePanelPlacement::Floating,
        }
    }

    fn visible_by_default(self) -> bool {
        !matches!(self, Self::Validation | Self::NodeEditor)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspacePanelPlacement {
    Left,
    Center,
    Right,
    Bottom,
    Floating,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePanelState {
    pub id: WorkspacePanelId,
    pub placement: WorkspacePanelPlacement,
    pub visible: bool,
    pub collapsed: bool,
    pub floating_rect: Option<WorkspacePanelRect>,
    pub split_ratio: f32,
    pub min_size: f32,
    pub max_size: f32,
    pub last_size: Option<f32>,
    pub z_order: i32,
}

impl WorkspacePanelState {
    fn default_for(id: WorkspacePanelId, z_order: i32) -> Self {
        let placement = id.default_placement();
        let (split_ratio, min_size, max_size) = match id {
            WorkspacePanelId::AssetBrowser => (0.12, 92.0, 420.0),
            WorkspacePanelId::Graph => (0.30, 150.0, 760.0),
            WorkspacePanelId::Composer => (0.58, 320.0, 4096.0),
            WorkspacePanelId::Inspector => (0.20, 150.0, 520.0),
            WorkspacePanelId::Validation => (0.30, 34.0, 720.0),
            WorkspacePanelId::Timeline => (0.26, 72.0, 420.0),
            WorkspacePanelId::NodeEditor => (0.50, 320.0, 4096.0),
        };
        Self {
            id,
            placement,
            visible: id.visible_by_default(),
            collapsed: false,
            floating_rect: None,
            split_ratio,
            min_size,
            max_size,
            last_size: None,
            z_order,
        }
    }
}

fn workspace_layout_schema_version() -> u32 {
    WORKSPACE_LAYOUT_SCHEMA_VERSION
}

pub fn editor_toolbar_mode(available_width: f32) -> EditorToolbarMode {
    if available_width >= TOOLBAR_EXPANDED_MIN_WIDTH {
        EditorToolbarMode::Expanded
    } else {
        EditorToolbarMode::Grouped
    }
}

pub fn validation_report_placement(inspector_visible: bool) -> ValidationReportPlacement {
    if inspector_visible {
        ValidationReportPlacement::Inspector
    } else {
        ValidationReportPlacement::FloatingWindow
    }
}

pub fn validation_report_body_height(available_height: f32, issue_count: usize) -> f32 {
    if issue_count == 0 {
        return 28.0;
    }
    (available_height * 0.42).clamp(120.0, 360.0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceLayout {
    #[serde(default = "workspace_layout_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub panels: BTreeMap<WorkspacePanelId, WorkspacePanelState>,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        let mut panels = BTreeMap::new();
        for (idx, id) in WorkspacePanelId::ALL.into_iter().enumerate() {
            panels.insert(id, WorkspacePanelState::default_for(id, idx as i32));
        }
        Self {
            schema_version: WORKSPACE_LAYOUT_SCHEMA_VERSION,
            panels,
        }
    }
}

impl WorkspaceLayout {
    pub fn normalize(&mut self) {
        self.schema_version = WORKSPACE_LAYOUT_SCHEMA_VERSION;
        for (idx, id) in WorkspacePanelId::ALL.into_iter().enumerate() {
            self.panels
                .entry(id)
                .or_insert_with(|| WorkspacePanelState::default_for(id, idx as i32));
        }
    }

    pub fn panel(&self, id: WorkspacePanelId) -> WorkspacePanelState {
        self.panels
            .get(&id)
            .cloned()
            .unwrap_or_else(|| WorkspacePanelState::default_for(id, id as i32))
    }

    pub fn set_visible(&mut self, id: WorkspacePanelId, visible: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.visible = visible;
        }
    }

    pub fn set_collapsed(&mut self, id: WorkspacePanelId, collapsed: bool) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.collapsed = collapsed;
        }
    }

    pub fn set_floating_rect(&mut self, id: WorkspacePanelId, rect: WorkspacePanelRect) {
        self.normalize();
        if let Some(panel) = self.panels.get_mut(&id) {
            panel.placement = WorkspacePanelPlacement::Floating;
            panel.floating_rect = Some(rect);
            panel.visible = true;
        }
    }
}
