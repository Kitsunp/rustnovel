use crate::editor::{LintIssue, NodeGraph};

use super::QuickFixCandidate;

mod audio;
mod builders;
mod character;
mod graph;
mod predicates;
mod scene;
mod support;

struct QuickFixRule {
    fix_id: &'static str,
    build: fn() -> QuickFixCandidate,
    matches: fn(&LintIssue, &NodeGraph) -> bool,
    apply: fn(&mut NodeGraph, &LintIssue) -> Result<bool, String>,
}

pub(super) fn suggest_fixes(issue: &LintIssue, graph: &NodeGraph) -> Vec<QuickFixCandidate> {
    quick_fix_rules()
        .iter()
        .filter(|rule| (rule.matches)(issue, graph))
        .map(|rule| (rule.build)())
        .collect()
}

pub(super) fn apply_fix(
    graph: &mut NodeGraph,
    issue: &LintIssue,
    fix_id: &str,
) -> Result<bool, String> {
    let rule = quick_fix_rules()
        .iter()
        .find(|rule| rule.fix_id == fix_id)
        .ok_or_else(|| format!("unsupported fix_id '{fix_id}'"))?;
    if !(rule.matches)(issue, graph) {
        return Err(format!(
            "fix '{fix_id}' preconditions failed for issue {}",
            issue.diagnostic_id()
        ));
    }
    (rule.apply)(graph, issue)
}

fn quick_fix_rules() -> &'static [QuickFixRule] {
    const RULES: &[QuickFixRule] = &[
        QuickFixRule {
            fix_id: "graph_add_start",
            build: builders::fix_add_missing_start,
            matches: predicates::matches_missing_start,
            apply: graph::apply_missing_start,
        },
        QuickFixRule {
            fix_id: "node_connect_dead_end_to_end",
            build: builders::fix_dead_end_to_end,
            matches: predicates::matches_dead_end,
            apply: graph::apply_dead_end,
        },
        QuickFixRule {
            fix_id: "choice_add_default_option",
            build: builders::fix_choice_add_default_option,
            matches: predicates::matches_choice_no_options,
            apply: graph::apply_choice_no_options,
        },
        QuickFixRule {
            fix_id: "choice_link_unlinked_to_end",
            build: builders::fix_choice_link_unlinked_to_end,
            matches: predicates::matches_choice_option_unlinked,
            apply: graph::apply_choice_option_unlinked,
        },
        QuickFixRule {
            fix_id: "choice_expand_options_to_ports",
            build: builders::fix_choice_expand_options_to_ports,
            matches: predicates::matches_choice_port_out_of_range,
            apply: graph::apply_choice_port_out_of_range,
        },
        QuickFixRule {
            fix_id: "dialogue_fill_speaker",
            build: builders::fix_fill_speaker,
            matches: predicates::matches_empty_speaker,
            apply: graph::apply_empty_speaker,
        },
        QuickFixRule {
            fix_id: "jump_set_start_target",
            build: builders::fix_fill_jump_target,
            matches: predicates::matches_empty_jump_target,
            apply: graph::apply_empty_jump_target,
        },
        QuickFixRule {
            fix_id: "transition_set_fade",
            build: builders::fix_transition_kind,
            matches: predicates::matches_invalid_transition_kind,
            apply: graph::apply_invalid_transition_kind,
        },
        QuickFixRule {
            fix_id: "transition_set_default_duration",
            build: builders::fix_transition_duration,
            matches: predicates::matches_invalid_transition_duration,
            apply: graph::apply_invalid_transition_duration,
        },
        QuickFixRule {
            fix_id: "audio_normalize_channel",
            build: builders::fix_audio_channel,
            matches: predicates::matches_invalid_audio_channel,
            apply: audio::apply_invalid_audio_channel,
        },
        QuickFixRule {
            fix_id: "audio_normalize_action",
            build: builders::fix_audio_action,
            matches: predicates::matches_invalid_audio_action,
            apply: audio::apply_invalid_audio_action,
        },
        QuickFixRule {
            fix_id: "audio_clamp_volume",
            build: builders::fix_audio_volume,
            matches: predicates::matches_invalid_audio_volume,
            apply: audio::apply_invalid_audio_volume,
        },
        QuickFixRule {
            fix_id: "audio_set_default_fade",
            build: builders::fix_audio_fade,
            matches: predicates::matches_invalid_audio_fade,
            apply: audio::apply_invalid_audio_fade,
        },
        QuickFixRule {
            fix_id: "scene_clear_empty_background",
            build: builders::fix_scene_bg_empty,
            matches: predicates::matches_empty_scene_background,
            apply: scene::apply_empty_scene_background,
        },
        QuickFixRule {
            fix_id: "scene_clear_empty_music",
            build: builders::fix_scene_music_empty,
            matches: predicates::matches_empty_scene_music,
            apply: scene::apply_empty_scene_music,
        },
        QuickFixRule {
            fix_id: "audio_clear_empty_asset",
            build: builders::fix_audio_asset_empty,
            matches: predicates::matches_empty_audio_asset,
            apply: audio::apply_empty_audio_asset,
        },
        QuickFixRule {
            fix_id: "audio_missing_asset_to_stop",
            build: builders::fix_audio_missing_asset,
            matches: predicates::matches_audio_missing_asset,
            apply: audio::apply_audio_missing_asset,
        },
        QuickFixRule {
            fix_id: "clear_missing_asset_reference",
            build: builders::fix_clear_missing_asset_reference,
            matches: predicates::matches_missing_asset_reference,
            apply: scene::apply_clear_asset_reference,
        },
        QuickFixRule {
            fix_id: "clear_unsafe_asset_reference",
            build: builders::fix_clear_unsafe_asset_reference,
            matches: predicates::matches_unsafe_asset_reference,
            apply: scene::apply_clear_asset_reference,
        },
        QuickFixRule {
            fix_id: "character_prune_or_fill_invalid_names",
            build: builders::fix_character_entries,
            matches: predicates::matches_empty_character_name,
            apply: character::apply_empty_character_name,
        },
        QuickFixRule {
            fix_id: "character_set_default_scale",
            build: builders::fix_character_scale,
            matches: predicates::matches_invalid_character_scale,
            apply: character::apply_invalid_character_scale,
        },
    ];
    RULES
}
