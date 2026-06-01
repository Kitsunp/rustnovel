use super::{DiagnosticTarget, EvidenceTrace, FieldPath, SemanticValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
    Info,
}

impl LintSeverity {
    pub const ALL: &'static [Self] = &[Self::Error, Self::Warning, Self::Info];

    pub fn label(self) -> &'static str {
        match self {
            LintSeverity::Error => "error",
            LintSeverity::Warning => "warning",
            LintSeverity::Info => "info",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|severity| severity.label().eq_ignore_ascii_case(value.trim()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationPhase {
    Graph,
    Compile,
    Runtime,
    DryRun,
}

impl ValidationPhase {
    pub const ALL: &'static [Self] = &[Self::Graph, Self::Compile, Self::Runtime, Self::DryRun];

    pub fn label(self) -> &'static str {
        match self {
            ValidationPhase::Graph => "GRAPH",
            ValidationPhase::Compile => "COMPILE",
            ValidationPhase::Runtime => "RUNTIME",
            ValidationPhase::DryRun => "DRYRUN",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|phase| phase.label().eq_ignore_ascii_case(value.trim()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintCode {
    MissingStart,
    MultipleStart,
    UnreachableNode,
    PotentialLoop,
    DeadEnd,
    ChoiceNoOptions,
    ChoiceOptionUnlinked,
    ChoicePortOutOfRange,
    AudioAssetMissing,
    AudioAssetEmpty,
    AssetReferenceMissing,
    SceneBackgroundEmpty,
    UnsafeAssetPath,
    InvalidAudioChannel,
    InvalidAudioAction,
    InvalidAudioVolume,
    InvalidAudioFade,
    InvalidCharacterScale,
    InvalidTransitionDuration,
    InvalidTransitionKind,
    EmptyCharacterName,
    EmptySpeakerName,
    EmptyJumpTarget,
    MissingJumpTarget,
    EmptyStateKey,
    InvalidLayoutPosition,
    PlaceholderChoiceOption,
    ContractUnsupportedExport,
    GenericEventUnchecked,
    PhaseTraceOk,
    PhaseTraceFailed,
    CompileError,
    RuntimeInitError,
    DryRunUnreachableCompiled,
    DryRunStepLimit,
    DryRunRuntimeError,
    DryRunParityMismatch,
    DryRunExtCallBlocked,
    DryRunFinished,
    FragmentPortStale,
    FragmentNodeMissing,
    FragmentOwnershipConflict,
    FragmentEmpty,
    SubgraphCallInvalid,
    FragmentRecursion,
    FragmentLabelCollision,
}

impl LintCode {
    pub const ALL: &'static [Self] = &[
        Self::MissingStart,
        Self::MultipleStart,
        Self::UnreachableNode,
        Self::PotentialLoop,
        Self::DeadEnd,
        Self::ChoiceNoOptions,
        Self::ChoiceOptionUnlinked,
        Self::ChoicePortOutOfRange,
        Self::AudioAssetMissing,
        Self::AudioAssetEmpty,
        Self::AssetReferenceMissing,
        Self::SceneBackgroundEmpty,
        Self::UnsafeAssetPath,
        Self::InvalidAudioChannel,
        Self::InvalidAudioAction,
        Self::InvalidAudioVolume,
        Self::InvalidAudioFade,
        Self::InvalidCharacterScale,
        Self::InvalidTransitionDuration,
        Self::InvalidTransitionKind,
        Self::EmptyCharacterName,
        Self::EmptySpeakerName,
        Self::EmptyJumpTarget,
        Self::MissingJumpTarget,
        Self::EmptyStateKey,
        Self::InvalidLayoutPosition,
        Self::PlaceholderChoiceOption,
        Self::ContractUnsupportedExport,
        Self::GenericEventUnchecked,
        Self::PhaseTraceOk,
        Self::PhaseTraceFailed,
        Self::CompileError,
        Self::RuntimeInitError,
        Self::DryRunUnreachableCompiled,
        Self::DryRunStepLimit,
        Self::DryRunRuntimeError,
        Self::DryRunParityMismatch,
        Self::DryRunExtCallBlocked,
        Self::DryRunFinished,
        Self::FragmentPortStale,
        Self::FragmentNodeMissing,
        Self::FragmentOwnershipConflict,
        Self::FragmentEmpty,
        Self::SubgraphCallInvalid,
        Self::FragmentRecursion,
        Self::FragmentLabelCollision,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LintCode::MissingStart => "VAL_START_MISSING",
            LintCode::MultipleStart => "VAL_START_MULTIPLE",
            LintCode::UnreachableNode => "VAL_UNREACHABLE",
            LintCode::PotentialLoop => "VAL_POTENTIAL_LOOP",
            LintCode::DeadEnd => "VAL_DEAD_END",
            LintCode::ChoiceNoOptions => "VAL_CHOICE_EMPTY",
            LintCode::ChoiceOptionUnlinked => "VAL_CHOICE_UNLINKED",
            LintCode::ChoicePortOutOfRange => "VAL_CHOICE_PORT_OOB",
            LintCode::AudioAssetMissing => "VAL_AUDIO_MISSING",
            LintCode::AudioAssetEmpty => "VAL_AUDIO_EMPTY",
            LintCode::AssetReferenceMissing => "VAL_ASSET_NOT_FOUND",
            LintCode::SceneBackgroundEmpty => "VAL_SCENE_BG_EMPTY",
            LintCode::UnsafeAssetPath => "VAL_ASSET_UNSAFE_PATH",
            LintCode::InvalidAudioChannel => "VAL_AUDIO_CHANNEL_INVALID",
            LintCode::InvalidAudioAction => "VAL_AUDIO_ACTION_INVALID",
            LintCode::InvalidAudioVolume => "VAL_AUDIO_VOLUME_INVALID",
            LintCode::InvalidAudioFade => "VAL_AUDIO_FADE_INVALID",
            LintCode::InvalidCharacterScale => "VAL_SCALE_INVALID",
            LintCode::InvalidTransitionDuration => "VAL_TRANSITION_DURATION",
            LintCode::InvalidTransitionKind => "VAL_TRANSITION_KIND_INVALID",
            LintCode::EmptyCharacterName => "VAL_CHARACTER_NAME_EMPTY",
            LintCode::EmptySpeakerName => "VAL_SPEAKER_EMPTY",
            LintCode::EmptyJumpTarget => "VAL_JUMP_EMPTY",
            LintCode::MissingJumpTarget => "VAL_JUMP_TARGET_MISSING",
            LintCode::EmptyStateKey => "VAL_STATE_KEY_EMPTY",
            LintCode::InvalidLayoutPosition => "VAL_LAYOUT_POSITION_INVALID",
            LintCode::PlaceholderChoiceOption => "VAL_CHOICE_PLACEHOLDER",
            LintCode::ContractUnsupportedExport => "VAL_CONTRACT_EXPORT_UNSUPPORTED",
            LintCode::GenericEventUnchecked => "VAL_GENERIC_UNCHECKED",
            LintCode::PhaseTraceOk => "TRACE_PHASE_OK",
            LintCode::PhaseTraceFailed => "TRACE_PHASE_FAILED",
            LintCode::CompileError => "CMP_SCRIPT_ERROR",
            LintCode::RuntimeInitError => "CMP_RUNTIME_INIT",
            LintCode::DryRunUnreachableCompiled => "DRY_UNREACHABLE",
            LintCode::DryRunStepLimit => "DRY_STEP_LIMIT",
            LintCode::DryRunRuntimeError => "DRY_RUNTIME_ERROR",
            LintCode::DryRunParityMismatch => "DRY_PARITY_MISMATCH",
            LintCode::DryRunExtCallBlocked => "DRY_EXTCALL_BLOCKED",
            LintCode::DryRunFinished => "DRY_FINISHED",
            LintCode::FragmentPortStale => "VAL_FRAGMENT_PORT_STALE",
            LintCode::FragmentNodeMissing => "VAL_FRAGMENT_NODE_MISSING",
            LintCode::FragmentOwnershipConflict => "VAL_FRAGMENT_OWNERSHIP_CONFLICT",
            LintCode::FragmentEmpty => "VAL_FRAGMENT_EMPTY",
            LintCode::SubgraphCallInvalid => "VAL_SUBGRAPH_CALL_INVALID",
            LintCode::FragmentRecursion => "VAL_FRAGMENT_RECURSION",
            LintCode::FragmentLabelCollision => "VAL_FRAGMENT_LABEL_COLLISION",
        }
    }

    pub fn from_label(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.label().eq_ignore_ascii_case(value.trim()))
    }
}

#[derive(Debug, Clone)]
pub struct LintIssue {
    pub node_id: Option<u32>,
    pub event_ip: Option<u32>,
    pub edge_from: Option<u32>,
    pub edge_to: Option<u32>,
    pub blocked_by: Option<String>,
    pub asset_path: Option<String>,
    pub target: Option<DiagnosticTarget>,
    pub field_path: Option<FieldPath>,
    pub semantic_values: Vec<SemanticValue>,
    pub evidence_trace: Option<EvidenceTrace>,
    pub operation_id: Option<String>,
    pub severity: LintSeverity,
    pub phase: ValidationPhase,
    pub code: LintCode,
    pub message: String,
}

impl LintIssue {
    pub fn diagnostic_id(&self) -> String {
        const RULE_VERSION: &str = "authoring-diagnostic-v2";
        let mut parts = vec![
            RULE_VERSION.to_string(),
            self.phase.label().to_string(),
            self.code.label().to_string(),
            self.node_id
                .map(|id| format!("node={id}"))
                .unwrap_or_else(|| "scope=global".to_string()),
        ];
        if let Some(event_ip) = self.event_ip {
            parts.push(format!("ip={event_ip}"));
        }
        match (self.edge_from, self.edge_to) {
            (Some(from), Some(to)) => parts.push(format!("edge={from}>{to}")),
            (Some(from), None) => parts.push(format!("edge_from={from}")),
            (None, Some(to)) => parts.push(format!("edge_to={to}")),
            (None, None) => {}
        }
        if let Some(asset_path) = self.asset_path.as_deref() {
            parts.push(format!("asset={}", normalize_diagnostic_part(asset_path)));
        }
        if let Some(blocked_by) = self.blocked_by.as_deref() {
            parts.push(format!(
                "blocked_by={}",
                normalize_diagnostic_part(blocked_by)
            ));
        }
        if let Some(target) = &self.target {
            parts.push(format!(
                "target={}",
                normalize_diagnostic_part(&target.stable_key())
            ));
        }
        if let Some(field_path) = &self.field_path {
            parts.push(format!(
                "field={}",
                normalize_diagnostic_part(&field_path.value)
            ));
        }
        if !self.semantic_values.is_empty() {
            let semantic = self
                .semantic_values
                .iter()
                .map(|value| normalize_diagnostic_part(&value.stable_key()))
                .collect::<Vec<_>>()
                .join("_");
            parts.push(format!("semantic={semantic}"));
        }
        parts.join(":")
    }

    pub fn new(
        node_id: Option<u32>,
        severity: LintSeverity,
        phase: ValidationPhase,
        code: LintCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            node_id,
            event_ip: None,
            edge_from: None,
            edge_to: None,
            blocked_by: None,
            asset_path: None,
            target: node_id.map(|node_id| DiagnosticTarget::Node { node_id }),
            field_path: None,
            semantic_values: Vec::new(),
            evidence_trace: None,
            operation_id: None,
            severity,
            phase,
            code,
            message: message.into(),
        }
    }

    pub fn error(
        node_id: Option<u32>,
        phase: ValidationPhase,
        code: LintCode,
        message: impl Into<String>,
    ) -> Self {
        Self::new(node_id, LintSeverity::Error, phase, code, message)
    }

    pub fn warning(
        node_id: Option<u32>,
        phase: ValidationPhase,
        code: LintCode,
        message: impl Into<String>,
    ) -> Self {
        Self::new(node_id, LintSeverity::Warning, phase, code, message)
    }

    pub fn info(
        node_id: Option<u32>,
        phase: ValidationPhase,
        code: LintCode,
        message: impl Into<String>,
    ) -> Self {
        Self::new(node_id, LintSeverity::Info, phase, code, message)
    }

    pub fn with_event_ip(mut self, event_ip: Option<u32>) -> Self {
        self.event_ip = event_ip;
        self
    }

    pub fn with_edge(mut self, edge_from: Option<u32>, edge_to: Option<u32>) -> Self {
        self.edge_from = edge_from;
        self.edge_to = edge_to;
        self
    }

    pub fn with_blocked_by(mut self, blocked_by: impl Into<String>) -> Self {
        self.blocked_by = Some(blocked_by.into());
        self
    }

    pub fn with_operation_id(mut self, operation_id: impl Into<String>) -> Self {
        self.operation_id = Some(operation_id.into());
        self
    }

    pub fn with_asset_path(mut self, asset_path: Option<String>) -> Self {
        self.asset_path = asset_path;
        self
    }

    pub fn with_target(mut self, target: DiagnosticTarget) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_field_path(mut self, field_path: impl Into<String>) -> Self {
        self.field_path = Some(FieldPath::new(field_path));
        self
    }

    pub fn with_semantic_value(mut self, semantic_value: SemanticValue) -> Self {
        self.semantic_values.push(semantic_value);
        self
    }

    pub fn with_evidence_trace(mut self) -> Self {
        self.evidence_trace = Some(EvidenceTrace::for_issue(
            self.code,
            self.target.clone(),
            self.field_path.clone(),
            &self.semantic_values,
            self.message.clone(),
        ));
        self
    }
}

fn normalize_diagnostic_part(value: &str) -> String {
    let out = value
        .trim()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let mut compact = out;
    while compact.contains("__") {
        compact = compact.replace("__", "_");
    }
    compact.trim_matches('_').to_string()
}
