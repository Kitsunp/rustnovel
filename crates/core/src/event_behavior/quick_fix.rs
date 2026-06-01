use super::*;

pub(super) fn suggest_node_quick_fixes(
    ctx: &QuickFixCtx<'_>,
    kind: NodeKind,
    issue: &LintIssue,
) -> Vec<BehaviorQuickFix> {
    let mut fixes = Vec::new();
    match (kind, issue.code) {
        (NodeKind::Dialogue, LintCode::EmptySpeakerName) => fixes.push(behavior_fix(
            "dialogue_fill_speaker",
            "Rellenar speaker",
            "Fill speaker",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Choice, LintCode::ChoiceNoOptions) => {
            fixes.push(behavior_fix(
                "choice_add_default_option",
                "Agregar opcion placeholder",
                "Add placeholder option",
                BehaviorQuickFixRisk::Review,
                false,
            ));
            fixes.push(behavior_fix(
                "choice_add_default_option_to_end",
                "Agregar opcion y conectar a End",
                "Add option and connect to End",
                BehaviorQuickFixRisk::Safe,
                true,
            ));
        }
        (NodeKind::Choice, LintCode::ChoiceOptionUnlinked) => fixes.push(behavior_fix(
            "choice_link_unlinked_to_end",
            "Conectar opciones sin salida",
            "Connect unlinked options",
            BehaviorQuickFixRisk::Review,
            true,
        )),
        (NodeKind::Choice, LintCode::ChoicePortOutOfRange) => fixes.push(behavior_fix(
            "choice_expand_options_to_ports",
            "Sincronizar opciones con puertos",
            "Sync options with ports",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Jump, LintCode::EmptyJumpTarget)
        | (NodeKind::JumpIf, LintCode::EmptyJumpTarget)
            if existing_jump_target(ctx.graph()).is_some() =>
        {
            fixes.push(behavior_fix(
                "jump_set_existing_target",
                "Usar destino existente",
                "Use existing target",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Transition, LintCode::InvalidTransitionKind) => fixes.push(behavior_fix(
            "transition_set_fade",
            "Usar fade",
            "Use fade",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Transition, LintCode::InvalidTransitionDuration) => fixes.push(behavior_fix(
            "transition_set_default_duration",
            "Usar duracion por defecto",
            "Use default duration",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Scene, LintCode::SceneBackgroundEmpty) => fixes.push(behavior_fix(
            "scene_clear_empty_background",
            "Limpiar background vacio",
            "Clear empty background",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::Scene, LintCode::AudioAssetEmpty) if scene_music_is_empty(ctx, issue) => {
            fixes.push(behavior_fix(
                "scene_clear_empty_music",
                "Limpiar musica vacia",
                "Clear empty music",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::AudioAssetEmpty) if audio_asset_is_empty(ctx, issue) => {
            fixes.push(behavior_fix(
                "audio_clear_empty_asset",
                "Limpiar asset de audio vacio",
                "Clear empty audio asset",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::AudioAssetMissing)
            if audio_play_is_missing_asset(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_missing_asset_to_stop",
                "Normalizar play sin asset a stop",
                "Normalize play without asset to stop",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Scene, LintCode::AssetReferenceMissing)
        | (NodeKind::ScenePatch, LintCode::AssetReferenceMissing)
        | (NodeKind::AudioAction, LintCode::AssetReferenceMissing)
            if has_clearable_asset_field(ctx.graph(), issue) =>
        {
            fixes.push(behavior_fix(
                "clear_missing_asset_reference",
                "Limpiar asset inexistente",
                "Clear missing asset",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::Scene, LintCode::UnsafeAssetPath)
        | (NodeKind::ScenePatch, LintCode::UnsafeAssetPath)
        | (NodeKind::AudioAction, LintCode::UnsafeAssetPath)
            if has_clearable_asset_field(ctx.graph(), issue) =>
        {
            fixes.push(behavior_fix(
                "clear_unsafe_asset_reference",
                "Limpiar asset inseguro",
                "Clear unsafe asset",
                BehaviorQuickFixRisk::Review,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioChannel)
            if audio_channel_can_normalize(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_normalize_channel",
                "Normalizar alias de canal",
                "Normalize channel alias",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioAction)
            if audio_action_can_normalize(ctx, issue) =>
        {
            fixes.push(behavior_fix(
                "audio_normalize_action",
                "Normalizar alias de accion",
                "Normalize action alias",
                BehaviorQuickFixRisk::Safe,
                false,
            ));
        }
        (NodeKind::AudioAction, LintCode::InvalidAudioVolume) => fixes.push(behavior_fix(
            "audio_clamp_volume",
            "Ajustar volumen",
            "Clamp volume",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::AudioAction, LintCode::InvalidAudioFade) => fixes.push(behavior_fix(
            "audio_set_default_fade",
            "Usar fade por defecto",
            "Use default fade",
            BehaviorQuickFixRisk::Safe,
            false,
        )),
        (NodeKind::CharacterPlacement, LintCode::EmptyCharacterName) => fixes.push(behavior_fix(
            "character_prune_or_fill_invalid_names",
            "Corregir nombres vacios",
            "Fix empty names",
            BehaviorQuickFixRisk::Review,
            false,
        )),
        (NodeKind::CharacterPlacement, LintCode::InvalidCharacterScale) => {
            fixes.push(behavior_fix(
                "character_set_default_scale",
                "Usar escala por defecto",
                "Use default scale",
                BehaviorQuickFixRisk::Safe,
                false,
            ))
        }
        _ => {}
    }
    fixes
}

pub(crate) fn normalized_audio_channel(channel: &str) -> Option<&'static str> {
    match channel.trim().to_ascii_lowercase().as_str() {
        "bgm" | "music" => Some("bgm"),
        "sfx" | "fx" | "sound" => Some("sfx"),
        "voice" | "vo" => Some("voice"),
        _ => None,
    }
}

pub(crate) fn normalized_audio_action(action: &str) -> Option<&'static str> {
    match action.trim().to_ascii_lowercase().as_str() {
        "play" | "start" => Some("play"),
        "stop" => Some("stop"),
        "fade" | "fadeout" | "fade_out" => Some("fade_out"),
        _ => None,
    }
}

fn scene_music_is_empty(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::Scene { music, .. })
            if music.as_deref().is_some_and(|val| val.trim().is_empty())
    )
}

fn audio_asset_is_empty(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { asset, .. })
            if asset.as_deref().is_some_and(|val| val.trim().is_empty())
    )
}

fn audio_play_is_missing_asset(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { action, asset, .. })
            if action.trim().eq_ignore_ascii_case("play")
                && asset.as_deref().is_none_or(|val| val.trim().is_empty())
    )
}

fn audio_channel_can_normalize(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { channel, .. })
            if normalized_audio_channel(channel).is_some_and(|normalized| normalized != channel)
    )
}

fn audio_action_can_normalize(ctx: &QuickFixCtx<'_>, issue: &LintIssue) -> bool {
    matches!(
        issue.node_id.and_then(|node_id| ctx.graph().get_node(node_id)),
        Some(StoryNode::AudioAction { action, .. })
            if normalized_audio_action(action).is_some_and(|normalized| normalized != action)
    )
}

fn has_clearable_asset_field(graph: &NodeGraph, issue: &LintIssue) -> bool {
    let Some(node_id) = issue.node_id else {
        return false;
    };
    let Some(node) = graph.get_node(node_id) else {
        return false;
    };
    let target = issue.asset_path.as_deref();
    let mut matches = 0;
    match node {
        StoryNode::Scene {
            background, music, ..
        } => {
            count_asset_match(&mut matches, background, target);
            count_asset_match(&mut matches, music, target);
        }
        StoryNode::ScenePatch(patch) => {
            count_asset_match(&mut matches, &patch.background, target);
            count_asset_match(&mut matches, &patch.music, target);
        }
        StoryNode::AudioAction { asset, .. } => {
            count_asset_match(&mut matches, asset, target);
        }
        _ => {}
    }
    matches == 1
}

fn count_asset_match(matches: &mut usize, val: &Option<String>, target: Option<&str>) {
    let Some(val) = val.as_deref() else {
        return;
    };
    let matched = match target {
        Some(target) => val == target,
        None => is_unsafe_asset_ref(val),
    };
    if matched {
        *matches += 1;
    }
}

const fn behavior_fix(
    fix_id: &'static str,
    title_es: &'static str,
    title_en: &'static str,
    risk: BehaviorQuickFixRisk,
    structural: bool,
) -> BehaviorQuickFix {
    BehaviorQuickFix::new(fix_id, title_es, title_en, risk, structural)
}

fn existing_jump_target(graph: &NodeGraph) -> Option<String> {
    if graph
        .nodes()
        .any(|(_, node, _)| matches!(node, StoryNode::Start))
    {
        return Some("start".to_string());
    }
    graph
        .nodes()
        .find(|(_, node, _)| !node.is_marker())
        .map(|(id, _, _)| format!("node_{id}"))
}
