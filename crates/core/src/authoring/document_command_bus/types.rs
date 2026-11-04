use std::collections::BTreeMap;
use std::fmt;

use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use super::super::{
    composer::{BackgroundFit, LayerOverride},
    AuthoringCommand, AuthoringDelta, AuthoringPosition, AuthoringReportFingerprint, NodeGraph,
    OperationKind, OperationLogEntry, VerificationRun,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthoringDocumentCommand {
    Graph(AuthoringCommand),
    /// Moves nodes as one atomic, undoable layout operation.
    MoveNodes {
        #[serde(deserialize_with = "deserialize_node_positions")]
        positions: BTreeMap<u32, AuthoringPosition>,
    },
    /// Records a completed graph edit from a snapshot-based authoring client.
    /// The graph retains shared node entries; logs are produced by the session.
    ReplaceGraph {
        graph: Box<NodeGraph>,
        kind: OperationKind,
        details: String,
        field_path: Option<String>,
        before_value: Option<String>,
        after_value: Option<String>,
    },
    SetLayerVisible {
        object_id: String,
        visible: bool,
    },
    SetLayerLocked {
        object_id: String,
        locked: bool,
    },
    SetBackgroundFitOverride {
        node_id: u32,
        fit: BackgroundFit,
    },
    ClearBackgroundFitOverride {
        node_id: u32,
    },
    ResetComposerOverrides,
    RevertLast,
    UndoLast,
    RedoLast,
}

fn deserialize_node_positions<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<u32, AuthoringPosition>, D::Error>
where
    D: Deserializer<'de>,
{
    struct NodePositions;

    impl<'de> Visitor<'de> for NodePositions {
        type Value = BTreeMap<u32, AuthoringPosition>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an object keyed by decimal node ID")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut positions = BTreeMap::new();
            // Internally tagged commands expose JSON object keys as strings.
            while let Some((key, position)) = map.next_entry::<String, AuthoringPosition>()? {
                let id = key
                    .parse::<u32>()
                    .map_err(|_| M::Error::custom(format!("invalid node ID '{key}'")))?;
                if positions.insert(id, position).is_some() {
                    return Err(M::Error::custom(format!("duplicate node ID {id}")));
                }
            }
            Ok(positions)
        }
    }

    deserializer.deserialize_map(NodePositions)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum AuthoringDocumentDelta {
    Graph(Box<AuthoringDelta>),
    GraphReplaced {
        before: Box<NodeGraph>,
    },
    NodesMoved {
        before: BTreeMap<u32, AuthoringPosition>,
        validation_dirty: bool,
    },
    LayerVisibleChanged {
        object_id: String,
        before: Option<LayerOverride>,
        after: Option<LayerOverride>,
    },
    LayerLockedChanged {
        object_id: String,
        before: Option<LayerOverride>,
        after: Option<LayerOverride>,
    },
    BackgroundFitChanged {
        node_id: u32,
        before: Option<BackgroundFit>,
        after: Option<BackgroundFit>,
    },
    BackgroundFitCleared {
        node_id: u32,
        before: Option<BackgroundFit>,
    },
    ComposerOverridesReset {
        before_layers: BTreeMap<String, LayerOverride>,
        before_background_fits: BTreeMap<String, BackgroundFit>,
    },
    Reverted {
        reverted: Box<AuthoringDocumentDelta>,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AuthoringDirtyFlags {
    pub graph_dirty: bool,
    pub layout_dirty: bool,
    pub assets_dirty: bool,
    pub document_dirty: bool,
    pub validation_dirty: bool,
    pub runtime_export_dirty: bool,
}

impl AuthoringDirtyFlags {
    pub fn is_clean(&self) -> bool {
        !self.graph_dirty
            && !self.layout_dirty
            && !self.assets_dirty
            && !self.document_dirty
            && !self.validation_dirty
            && !self.runtime_export_dirty
    }

    pub(crate) fn include(&mut self, other: Self) {
        self.graph_dirty |= other.graph_dirty;
        self.layout_dirty |= other.layout_dirty;
        self.assets_dirty |= other.assets_dirty;
        self.document_dirty |= other.document_dirty;
        self.validation_dirty |= other.validation_dirty;
        self.runtime_export_dirty |= other.runtime_export_dirty;
    }

    pub(crate) fn for_delta(delta: &AuthoringDocumentDelta) -> Self {
        match delta {
            AuthoringDocumentDelta::NodesMoved {
                validation_dirty, ..
            } => Self {
                layout_dirty: true,
                document_dirty: true,
                validation_dirty: *validation_dirty,
                ..Self::default()
            },
            AuthoringDocumentDelta::LayerVisibleChanged { .. }
            | AuthoringDocumentDelta::LayerLockedChanged { .. }
            | AuthoringDocumentDelta::BackgroundFitChanged { .. }
            | AuthoringDocumentDelta::BackgroundFitCleared { .. }
            | AuthoringDocumentDelta::ComposerOverridesReset { .. } => Self {
                layout_dirty: true,
                document_dirty: true,
                ..Self::default()
            },
            AuthoringDocumentDelta::Graph(_) | AuthoringDocumentDelta::GraphReplaced { .. } => {
                Self {
                    graph_dirty: true,
                    layout_dirty: true,
                    assets_dirty: true,
                    document_dirty: true,
                    validation_dirty: true,
                    runtime_export_dirty: true,
                }
            }
            AuthoringDocumentDelta::Reverted { reverted } => Self::for_delta(reverted),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthoringDocumentCommandOutcome {
    pub delta: AuthoringDocumentDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
    pub before_fingerprint: AuthoringReportFingerprint,
    pub after_fingerprint: AuthoringReportFingerprint,
}
