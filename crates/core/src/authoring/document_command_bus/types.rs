use serde::Serialize;
use std::collections::BTreeMap;

use super::super::{
    composer::{BackgroundFit, LayerOverride},
    AuthoringCommand, AuthoringDelta, AuthoringReportFingerprint, NodeGraph, OperationKind,
    OperationLogEntry, VerificationRun,
};

#[derive(Clone, Debug)]
pub enum AuthoringDocumentCommand {
    Graph(AuthoringCommand),
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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum AuthoringDocumentDelta {
    Graph(Box<AuthoringDelta>),
    GraphReplaced {
        before: Box<NodeGraph>,
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
