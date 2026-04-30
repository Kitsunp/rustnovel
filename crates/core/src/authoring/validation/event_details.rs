use super::assets::validate_asset;
use super::scene::validate_character_scale;
use crate::authoring::{
    DiagnosticTarget, FieldPath, LintCode, LintIssue, SemanticValue, SemanticValueKind,
    ValidationPhase,
};

pub(super) struct AudioValidation<'a> {
    pub id: u32,
    pub channel: &'a str,
    pub action: &'a str,
    pub asset: &'a Option<String>,
    pub volume: &'a Option<f32>,
    pub fade_duration_ms: &'a Option<u64>,
}

pub(super) fn validate_audio<F>(
    audio: AudioValidation<'_>,
    asset_exists: &F,
    issues: &mut Vec<LintIssue>,
) where
    F: Fn(&str) -> bool,
{
    if !matches!(audio.channel, "bgm" | "sfx" | "voice") {
        issues.push(
            LintIssue::error(
                Some(audio.id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioChannel,
                "Invalid audio channel",
            )
            .with_target(DiagnosticTarget::AudioChannel {
                node_id: Some(audio.id),
                channel: audio.channel.to_string(),
            })
            .with_field_path(format!("graph.nodes[{}].channel", audio.id))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::AudioChannelRef,
                audio.channel.to_string(),
                format!("graph.nodes[{}].channel", audio.id),
            ))
            .with_evidence_trace(),
        );
    }
    if !matches!(audio.action, "play" | "stop" | "fade_out") {
        issues.push(
            LintIssue::error(
                Some(audio.id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioAction,
                "Invalid audio action",
            )
            .with_field_path(format!("graph.nodes[{}].action", audio.id))
            .with_evidence_trace(),
        );
    }
    if audio
        .volume
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        issues.push(
            LintIssue::error(
                Some(audio.id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioVolume,
                "Invalid audio volume",
            )
            .with_field_path(format!("graph.nodes[{}].volume", audio.id))
            .with_evidence_trace(),
        );
    }
    if matches!(audio.action, "stop" | "fade_out") && audio.fade_duration_ms.unwrap_or(0) == 0 {
        issues.push(
            LintIssue::warning(
                Some(audio.id),
                ValidationPhase::Graph,
                LintCode::InvalidAudioFade,
                "Missing audio fade duration",
            )
            .with_field_path(format!("graph.nodes[{}].fade_duration_ms", audio.id))
            .with_evidence_trace(),
        );
    }
    if audio.action == "play" && audio.asset.is_none() {
        issues.push(
            LintIssue::warning(
                Some(audio.id),
                ValidationPhase::Graph,
                LintCode::AudioAssetMissing,
                "Audio asset path is missing",
            )
            .with_field_path(format!("graph.nodes[{}].asset", audio.id))
            .with_evidence_trace(),
        );
    }
    validate_asset(Some(audio.id), audio.asset, "audio", asset_exists, issues);
}

pub(super) fn validate_transition(
    id: u32,
    kind: &str,
    duration_ms: u32,
    issues: &mut Vec<LintIssue>,
) {
    if duration_ms == 0 {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::InvalidTransitionDuration,
                "Transition duration should be > 0 ms",
            )
            .with_target(DiagnosticTarget::Transition {
                node_id: Some(id),
                kind: kind.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].duration_ms"))
            .with_evidence_trace(),
        );
    }
    if !matches!(kind, "fade" | "fade_black" | "dissolve" | "cut") {
        issues.push(
            LintIssue::warning(
                Some(id),
                ValidationPhase::Graph,
                LintCode::InvalidTransitionKind,
                "Unknown transition kind",
            )
            .with_target(DiagnosticTarget::Transition {
                node_id: Some(id),
                kind: kind.to_string(),
            })
            .with_field_path(format!("graph.nodes[{id}].kind"))
            .with_semantic_value(SemanticValue::new(
                SemanticValueKind::TransitionKind,
                kind.to_string(),
                format!("graph.nodes[{id}].kind"),
            ))
            .with_evidence_trace(),
        );
    }
}

pub(super) fn validate_character(
    id: u32,
    name: &str,
    scale: &Option<f32>,
    issues: &mut Vec<LintIssue>,
) {
    if name.trim().is_empty() {
        issues.push(
            LintIssue::error(
                Some(id),
                ValidationPhase::Graph,
                LintCode::EmptyCharacterName,
                "Character name is empty",
            )
            .with_target(DiagnosticTarget::Character {
                node_id: Some(id),
                name: name.to_string(),
                field_path: Some(FieldPath::new(format!("graph.nodes[{id}].name"))),
            })
            .with_field_path(format!("graph.nodes[{id}].name"))
            .with_evidence_trace(),
        );
    }
    validate_character_scale(Some(id), scale, issues);
}
