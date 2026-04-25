use super::{GraphConnection, LintCode, LintIssue, NodeGraph, StoryNode, ValidationPhase};

pub fn validate(graph: &NodeGraph) -> Vec<LintIssue> {
    validate_with_asset_probe(graph, |_| true)
}

pub fn validate_with_asset_probe<F>(graph: &NodeGraph, asset_exists: F) -> Vec<LintIssue>
where
    F: Fn(&str) -> bool,
{
    let mut issues = Vec::new();
    let start_nodes = graph
        .nodes()
        .filter_map(|(id, node, _)| matches!(node, StoryNode::Start).then_some(*id))
        .collect::<Vec<_>>();
    match start_nodes.len() {
        0 => issues.push(LintIssue::error(
            None,
            ValidationPhase::Graph,
            LintCode::MissingStart,
            "Missing Start node",
        )),
        1 => {}
        count => issues.push(LintIssue::warning(
            None,
            ValidationPhase::Graph,
            LintCode::MultipleStart,
            format!("Multiple Start nodes found ({count})"),
        )),
    }
    let flow = graph.flow_analysis(&start_nodes);
    for (id, node, _) in graph.nodes() {
        if !flow.reachable.contains(id) {
            issues.push(LintIssue::warning(
                Some(*id),
                ValidationPhase::Graph,
                LintCode::UnreachableNode,
                "Unreachable node",
            ));
        }
        validate_node(graph, *id, node, &asset_exists, &mut issues);
    }
    for node_id in flow.reachable_cycle_nodes {
        issues.push(LintIssue::warning(
            Some(node_id),
            ValidationPhase::Graph,
            LintCode::PotentialLoop,
            "Potential execution loop detected on reachable route",
        ));
    }
    issues
}

fn validate_node<F>(
    graph: &NodeGraph,
    id: u32,
    node: &StoryNode,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    if !node.is_marker() && !node.export_supported() {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::ContractUnsupportedExport,
            "Node is not export-compatible",
        ));
    }
    match node {
        StoryNode::Dialogue { speaker, .. } if speaker.trim().is_empty() => {
            issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::EmptySpeakerName,
                "Dialogue speaker is empty",
            ));
        }
        StoryNode::Choice { options, .. } => validate_choice(graph, id, options, issues),
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => validate_scene(id, background, music, characters, asset_exists, issues),
        StoryNode::Jump { target } | StoryNode::JumpIf { target, .. }
            if target.trim().is_empty() =>
        {
            issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::EmptyJumpTarget,
                "Jump target is empty",
            ));
        }
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            ..
        } => validate_audio(
            id,
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            asset_exists,
            issues,
        ),
        StoryNode::Transition {
            kind, duration_ms, ..
        } => validate_transition(id, kind, *duration_ms, issues),
        StoryNode::CharacterPlacement { name, scale, .. } => {
            validate_character(id, name, scale, issues)
        }
        _ => {}
    }
    if !matches!(node, StoryNode::End) && !graph.connections().any(|conn| conn.from == id) {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::DeadEnd,
            "Node has no outgoing transition",
        ));
    }
}

fn validate_scene<F>(
    id: u32,
    background: &Option<String>,
    music: &Option<String>,
    characters: &[crate::CharacterPlacementRaw],
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    validate_asset(id, background, "background", asset_exists, issues);
    validate_asset(id, music, "music", asset_exists, issues);
    if characters
        .iter()
        .any(|character| character.name.trim().is_empty())
    {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Scene has an empty character name",
        ));
    }
}

fn validate_choice(graph: &NodeGraph, id: u32, options: &[String], issues: &mut Vec<LintIssue>) {
    if options.is_empty() {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::ChoiceNoOptions,
            "Choice has no options",
        ));
    }
    let outgoing = graph
        .connections()
        .filter(|conn| conn.from == id)
        .collect::<Vec<&GraphConnection>>();
    for idx in 0..options.len() {
        if !outgoing.iter().any(|conn| conn.from_port == idx) {
            issues.push(LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::ChoiceOptionUnlinked,
                format!("Choice option {idx} is unlinked"),
            ));
        }
    }
    for conn in outgoing {
        if conn.from_port >= options.len() {
            issues.push(
                LintIssue::warning(
                    Some(id),
                    ValidationPhase::Graph,
                    LintCode::ChoicePortOutOfRange,
                    "Choice connection port is out of range",
                )
                .with_edge(Some(conn.from), Some(conn.to)),
            );
        }
    }
}

fn validate_audio<F>(
    id: u32,
    channel: &str,
    action: &str,
    asset: &Option<String>,
    volume: &Option<f32>,
    fade_duration_ms: &Option<u64>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    if !matches!(channel, "bgm" | "sfx" | "voice") {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioChannel,
            "Invalid audio channel",
        ));
    }
    if !matches!(action, "play" | "stop" | "fade_out") {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioAction,
            "Invalid audio action",
        ));
    }
    if volume.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioVolume,
            "Invalid audio volume",
        ));
    }
    if matches!(action, "stop" | "fade_out") && fade_duration_ms.unwrap_or(0) == 0 {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidAudioFade,
            "Missing audio fade duration",
        ));
    }
    validate_asset(id, asset, "audio", asset_exists, issues);
}

fn validate_transition(id: u32, kind: &str, duration_ms: u32, issues: &mut Vec<LintIssue>) {
    if duration_ms == 0 {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidTransitionDuration,
            "Transition duration should be > 0 ms",
        ));
    }
    if !matches!(kind, "fade" | "fade_black" | "dissolve" | "cut") {
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidTransitionKind,
            "Unknown transition kind",
        ));
    }
}

fn validate_character(id: u32, name: &str, scale: &Option<f32>, issues: &mut Vec<LintIssue>) {
    if name.trim().is_empty() {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::EmptyCharacterName,
            "Character name is empty",
        ));
    }
    if scale.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        issues.push(LintIssue::error(
            Some(id),
            ValidationPhase::Graph,
            LintCode::InvalidCharacterScale,
            "Character scale is invalid",
        ));
    }
}

fn validate_asset<F>(
    id: u32,
    value: &Option<String>,
    label: &str,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    let Some(path) = value else {
        return;
    };
    if path.trim().is_empty() {
        let code = if label == "background" {
            LintCode::SceneBackgroundEmpty
        } else {
            LintCode::AudioAssetEmpty
        };
        issues.push(LintIssue::warning(
            Some(id),
            ValidationPhase::Graph,
            code,
            "Asset path is empty",
        ));
    } else if is_unsafe_asset_ref(path) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::UnsafeAssetPath,
                "Asset path is unsafe",
            )
            .with_asset_path(Some(path.clone())),
        );
    } else if !asset_exists(path) {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::AssetReferenceMissing,
                "Asset reference does not exist",
            )
            .with_asset_path(Some(path.clone())),
        );
    }
}

fn is_unsafe_asset_ref(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    path.starts_with('/')
        || path.starts_with('\\')
        || lower.contains("://")
        || path.split(['/', '\\']).any(|part| part == "..")
}
