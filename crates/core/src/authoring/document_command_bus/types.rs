use super::super::{
    composer::{BackgroundFit, LayerOverride},
    AuthoringCommand, AuthoringDelta, AuthoringReportFingerprint, OperationLogEntry,
    VerificationRun,
};

#[derive(Clone, Debug)]
pub enum AuthoringDocumentCommand {
    Graph(AuthoringCommand),
    SetLayerVisible { object_id: String, visible: bool },
    SetLayerLocked { object_id: String, locked: bool },
    SetBackgroundFitOverride { node_id: u32, fit: BackgroundFit },
    ClearBackgroundFitOverride { node_id: u32 },
    RevertLast,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AuthoringDocumentDelta {
    Graph(Box<AuthoringDelta>),
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
    Reverted {
        reverted: Box<AuthoringDocumentDelta>,
    },
}

#[derive(Clone, Debug)]
pub struct AuthoringDocumentCommandOutcome {
    pub delta: AuthoringDocumentDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
    pub before_fingerprint: AuthoringReportFingerprint,
    pub after_fingerprint: AuthoringReportFingerprint,
}
