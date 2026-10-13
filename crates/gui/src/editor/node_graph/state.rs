use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphOperationHint {
    pub kind: String,
    pub details: String,
    pub field_path: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub push_undo_snapshot: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphLayoutOrientation {
    #[default]
    Vertical,
    Horizontal,
}

impl GraphLayoutOrientation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vertical => "Vertical",
            Self::Horizontal => "Horizontal",
        }
    }
}

/// A node graph representing the story structure.
#[derive(Clone, Debug)]
pub struct NodeGraph {
    pub authoring_session: AuthoringDocumentSession,
    pub view: GraphViewState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphViewState {
    #[serde(skip)]
    pub(super) modified: bool,
    #[serde(skip)]
    pub(super) drag_positions: std::collections::BTreeMap<u32, AuthoringPosition>,
    /// Currently selected node
    #[serde(skip)]
    pub selected: Option<u32>,
    /// Multi-selection used for grouping fragments and batch graph operations.
    #[serde(skip)]
    pub selected_nodes: BTreeSet<u32>,
    /// Pan offset (world-space translation)
    #[serde(default)]
    pub pan: egui::Vec2,
    /// Zoom level
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Preferred direction used by Auto Layout.
    #[serde(default)]
    pub layout_orientation: GraphLayoutOrientation,
    /// Node being edited inline
    #[serde(skip)]
    pub editing: Option<u32>,
    /// Deferred request to inspect a node's composed scene in the workbench.
    #[serde(skip)]
    pub composer_inspection_request: Option<u32>,
    /// Node being dragged (robust interaction)
    #[serde(skip)]
    pub dragging_node: Option<u32>,
    /// Node being connected (Connect To mode)
    #[serde(skip)]
    pub connecting_from: Option<(u32, usize)>,
    /// True when connection mode was started from context menu and should wait for a click.
    #[serde(skip)]
    pub connecting_sticky: bool,
    /// Graph-space marquee selection start.
    #[serde(skip)]
    pub marquee_start: Option<egui::Pos2>,
    /// Graph-space marquee selection current endpoint.
    #[serde(skip)]
    pub marquee_current: Option<egui::Pos2>,
    /// Active context menu
    #[serde(skip)]
    pub context_menu: Option<ContextMenu>,
    /// Last semantic editor operation inferred at graph level.
    #[serde(skip)]
    pub operation_hint: Option<GraphOperationHint>,
}

impl std::ops::Deref for NodeGraph {
    type Target = GraphViewState;
    fn deref(&self) -> &Self::Target {
        &self.view
    }
}
impl std::ops::DerefMut for NodeGraph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.view
    }
}
impl Serialize for NodeGraph {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct StoredGraph<'a> {
            #[serde(flatten)]
            authoring: &'a AuthoringGraph,
            #[serde(flatten)]
            view: &'a GraphViewState,
        }
        StoredGraph {
            authoring: self.authoring_graph(),
            view: &self.view,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for NodeGraph {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct StoredGraph {
            #[serde(flatten)]
            authoring: AuthoringGraph,
            #[serde(flatten)]
            view: GraphViewState,
        }
        let stored = StoredGraph::deserialize(deserializer)?;
        Ok(Self {
            authoring_session: AuthoringDocumentSession::new(AuthoringDocument::new(
                stored.authoring,
            )),
            view: stored.view,
        })
    }
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self {
            modified: false,
            drag_positions: std::collections::BTreeMap::new(),
            selected: None,
            selected_nodes: BTreeSet::new(),
            pan: egui::Vec2::ZERO,
            zoom: ZOOM_DEFAULT,
            layout_orientation: GraphLayoutOrientation::Vertical,
            editing: None,
            composer_inspection_request: None,
            dragging_node: None,
            connecting_from: None,
            connecting_sticky: false,
            marquee_start: None,
            marquee_current: None,
            context_menu: None,
            operation_hint: None,
        }
    }
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self::from_authoring_graph(AuthoringGraph::new())
    }
}
