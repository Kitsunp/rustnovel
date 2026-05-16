use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldPath {
    pub value: String,
}

impl FieldPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn stable_key(&self) -> String {
        normalize_trace_part(&self.value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target_kind", rename_all = "snake_case")]
pub enum DiagnosticTarget {
    Graph,
    Node {
        node_id: u32,
    },
    Edge {
        from: u32,
        from_port: usize,
        to: Option<u32>,
    },
    ChoiceOption {
        node_id: u32,
        option_index: usize,
    },
    JumpTarget {
        node_id: u32,
        target: String,
    },
    AssetRef {
        node_id: Option<u32>,
        field_path: FieldPath,
        asset_path: String,
    },
    SceneProfile {
        profile_id: String,
    },
    SceneLayer {
        profile_id: Option<String>,
        layer_index: usize,
        layer_name: Option<String>,
    },
    Character {
        node_id: Option<u32>,
        name: String,
        field_path: Option<FieldPath>,
    },
    AudioChannel {
        node_id: Option<u32>,
        channel: String,
    },
    Transition {
        node_id: Option<u32>,
        kind: String,
    },
    RuntimeEvent {
        event_ip: u32,
    },
    Fragment {
        fragment_id: String,
    },
    Generic {
        field_path: Option<FieldPath>,
    },
}

impl DiagnosticTarget {
    pub fn stable_key(&self) -> String {
        match self {
            Self::Graph => "graph".to_string(),
            Self::Node { node_id } => format!("node_{node_id}"),
            Self::Edge {
                from,
                from_port,
                to,
            } => format!(
                "edge_{from}_{from_port}_{}",
                to.map(|id| id.to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            Self::ChoiceOption {
                node_id,
                option_index,
            } => format!("choice_{node_id}_option_{option_index}"),
            Self::JumpTarget { node_id, target } => {
                format!("jump_{node_id}_{}", normalize_trace_part(target))
            }
            Self::AssetRef {
                node_id,
                field_path,
                asset_path,
            } => format!(
                "asset_{}_{}_{}",
                node_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "global".to_string()),
                field_path.stable_key(),
                normalize_trace_part(asset_path)
            ),
            Self::SceneProfile { profile_id } => {
                format!("profile_{}", normalize_trace_part(profile_id))
            }
            Self::SceneLayer {
                profile_id,
                layer_index,
                layer_name,
            } => format!(
                "layer_{}_{}_{}",
                profile_id
                    .as_deref()
                    .map(normalize_trace_part)
                    .unwrap_or_else(|| "global".to_string()),
                layer_index,
                layer_name
                    .as_deref()
                    .map(normalize_trace_part)
                    .unwrap_or_else(|| "unnamed".to_string())
            ),
            Self::Character {
                node_id,
                name,
                field_path,
            } => format!(
                "character_{}_{}_{}",
                node_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "global".to_string()),
                normalize_trace_part(name),
                field_path
                    .as_ref()
                    .map(FieldPath::stable_key)
                    .unwrap_or_else(|| "na".to_string())
            ),
            Self::AudioChannel { node_id, channel } => format!(
                "audio_{}_{}",
                node_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "global".to_string()),
                normalize_trace_part(channel)
            ),
            Self::Transition { node_id, kind } => format!(
                "transition_{}_{}",
                node_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "global".to_string()),
                normalize_trace_part(kind)
            ),
            Self::RuntimeEvent { event_ip } => format!("event_ip_{event_ip}"),
            Self::Fragment { fragment_id } => {
                format!("fragment_{}", normalize_trace_part(fragment_id))
            }
            Self::Generic { field_path } => field_path
                .as_ref()
                .map(|path| format!("generic_{}", path.stable_key()))
                .unwrap_or_else(|| "generic".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticValueKind {
    LabelRef,
    AssetRef,
    VariableRef,
    CharacterRef,
    PluginRef,
    AudioChannelRef,
    TransitionKind,
    Text,
    Number,
}

impl SemanticValueKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LabelRef => "label_ref",
            Self::AssetRef => "asset_ref",
            Self::VariableRef => "variable_ref",
            Self::CharacterRef => "character_ref",
            Self::PluginRef => "plugin_ref",
            Self::AudioChannelRef => "audio_channel_ref",
            Self::TransitionKind => "transition_kind",
            Self::Text => "text",
            Self::Number => "number",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticValue {
    pub kind: SemanticValueKind,
    pub raw_value: String,
    pub normalized_value: String,
    pub owner_path: FieldPath,
    pub introduced_by_operation: Option<String>,
}

impl SemanticValue {
    pub fn new(
        kind: SemanticValueKind,
        raw_value: impl Into<String>,
        owner_path: impl Into<String>,
    ) -> Self {
        let raw_value = raw_value.into();
        Self {
            kind,
            normalized_value: normalize_semantic_value(&raw_value),
            raw_value,
            owner_path: FieldPath::new(owner_path),
            introduced_by_operation: None,
        }
    }

    pub fn stable_key(&self) -> String {
        format!(
            "{}_{}_{}",
            self.kind.label(),
            self.owner_path.stable_key(),
            normalize_trace_part(&self.normalized_value)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceAtomKind {
    OperationApplied,
    FieldChanged,
    ValueRead,
    ResolverLookup,
    RuleEvaluated,
    Failure,
    RuntimeConsequence,
    FixSuggested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceAtom {
    pub atom_id: String,
    pub kind: TraceAtomKind,
    pub summary: String,
    pub target: Option<DiagnosticTarget>,
    pub field_path: Option<FieldPath>,
    pub semantic_value: Option<SemanticValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceRelation {
    Produced,
    ConsumedBy,
    ResolvedBy,
    FailedAt,
    Caused,
    Affects,
    CanBeFixedBy,
    FollowedBy,
    Suggested,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEdge {
    pub from: String,
    pub to: String,
    pub relation: TraceRelation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTrace {
    pub trace_id: String,
    pub atoms: Vec<TraceAtom>,
    pub edges: Vec<TraceEdge>,
}

impl EvidenceTrace {
    pub fn for_issue(
        code: super::super::LintCode,
        target: Option<DiagnosticTarget>,
        field_path: Option<FieldPath>,
        semantic_values: &[SemanticValue],
        failure_summary: impl Into<String>,
    ) -> Self {
        let trace_id = format!(
            "trace:{}:{}:{}",
            code.label(),
            target
                .as_ref()
                .map(DiagnosticTarget::stable_key)
                .unwrap_or_else(|| "global".to_string()),
            field_path
                .as_ref()
                .map(FieldPath::stable_key)
                .unwrap_or_else(|| "na".to_string())
        );
        let mut atoms = Vec::new();
        let mut edges = Vec::new();
        let operation_id = "operation_applied".to_string();
        let field_changed_id = "field_changed".to_string();
        let resolver_id = "resolver_lookup".to_string();
        let rule_id = "rule_evaluated".to_string();
        let failure_id = "failure".to_string();
        let consequence_id = "runtime_consequence".to_string();
        let fix_id = "fix_suggested".to_string();
        atoms.push(TraceAtom {
            atom_id: operation_id.clone(),
            kind: TraceAtomKind::OperationApplied,
            summary: "Current authoring document state was inspected".to_string(),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        atoms.push(TraceAtom {
            atom_id: field_changed_id.clone(),
            kind: TraceAtomKind::FieldChanged,
            summary: "Relevant authoring field participates in validation".to_string(),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        edges.push(TraceEdge {
            from: operation_id.clone(),
            to: field_changed_id.clone(),
            relation: TraceRelation::FollowedBy,
        });
        for (index, value) in semantic_values.iter().enumerate() {
            let atom_id = format!("value_{index}");
            atoms.push(TraceAtom {
                atom_id: atom_id.clone(),
                kind: TraceAtomKind::ValueRead,
                summary: format!("Read {} '{}'", value.kind.label(), value.normalized_value),
                target: target.clone(),
                field_path: Some(value.owner_path.clone()),
                semantic_value: Some(value.clone()),
            });
            edges.push(TraceEdge {
                from: field_changed_id.clone(),
                to: atom_id.clone(),
                relation: TraceRelation::Produced,
            });
            edges.push(TraceEdge {
                from: atom_id,
                to: resolver_id.clone(),
                relation: TraceRelation::ConsumedBy,
            });
        }
        if semantic_values.is_empty() {
            edges.push(TraceEdge {
                from: field_changed_id.clone(),
                to: resolver_id.clone(),
                relation: TraceRelation::ConsumedBy,
            });
        }
        atoms.push(TraceAtom {
            atom_id: resolver_id.clone(),
            kind: TraceAtomKind::ResolverLookup,
            summary: "Resolver looked up the referenced semantic value".to_string(),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        atoms.push(TraceAtom {
            atom_id: rule_id.clone(),
            kind: TraceAtomKind::RuleEvaluated,
            summary: format!(
                "Validation rule {} evaluated the resolved value",
                code.label()
            ),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        edges.push(TraceEdge {
            from: resolver_id,
            to: rule_id.clone(),
            relation: TraceRelation::ConsumedBy,
        });
        atoms.push(TraceAtom {
            atom_id: failure_id.clone(),
            kind: TraceAtomKind::Failure,
            summary: failure_summary.into(),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        edges.push(TraceEdge {
            from: rule_id,
            to: failure_id.clone(),
            relation: TraceRelation::FailedAt,
        });
        atoms.push(TraceAtom {
            atom_id: consequence_id.clone(),
            kind: TraceAtomKind::RuntimeConsequence,
            summary: "If exported unchanged, runtime/preview behavior may diverge or fail"
                .to_string(),
            target: target.clone(),
            field_path: field_path.clone(),
            semantic_value: None,
        });
        edges.push(TraceEdge {
            from: failure_id.clone(),
            to: consequence_id,
            relation: TraceRelation::Affects,
        });
        atoms.push(TraceAtom {
            atom_id: fix_id.clone(),
            kind: TraceAtomKind::FixSuggested,
            summary: "A manual edit or quick-fix can resolve the diagnostic".to_string(),
            target,
            field_path,
            semantic_value: None,
        });
        edges.push(TraceEdge {
            from: failure_id,
            to: fix_id,
            relation: TraceRelation::CanBeFixedBy,
        });
        Self {
            trace_id,
            atoms,
            edges,
        }
    }
}

fn normalize_semantic_value(value: &str) -> String {
    value.trim().replace('\\', "/")
}

fn normalize_trace_part(value: &str) -> String {
    let normalized = normalize_semantic_value(value);
    let mut out = normalized
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}
