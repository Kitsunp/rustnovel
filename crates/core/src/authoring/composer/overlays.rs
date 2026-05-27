use crate::event::EventCompiled;
use crate::localization::LocalizationCatalog;

use super::super::StoryNode;
use super::{ComposerOverlay, LayeredSceneObject, StageLayerKind};

pub(super) fn overlay_source_for_authoring_node(
    node_id: u32,
    node: &StoryNode,
) -> Option<(Option<u32>, &'static str)> {
    match node {
        StoryNode::Dialogue { .. } => Some((Some(node_id), "dialogue")),
        StoryNode::Choice { .. } => Some((Some(node_id), "choice")),
        StoryNode::Transition { .. } => Some((Some(node_id), "transition")),
        _ => None,
    }
}

pub(super) fn overlay_layer_objects(
    overlays: &[ComposerOverlay],
    source: Option<(Option<u32>, &'static str)>,
) -> Vec<LayeredSceneObject> {
    overlays
        .iter()
        .map(|overlay| {
            let (name, kind, z_index) = match overlay {
                ComposerOverlay::Dialogue { .. } => {
                    ("dialogue", StageLayerKind::DialogueUi, 10_000)
                }
                ComposerOverlay::Choice { .. } => ("choice", StageLayerKind::InteractionUi, 10_100),
                ComposerOverlay::Transition { .. } => {
                    ("transition", StageLayerKind::Effects, 9_900)
                }
                ComposerOverlay::DebugTrace { .. } => {
                    ("debug_trace", StageLayerKind::DebugTrace, 10_200)
                }
            };
            let (source_node_id, field) = source.unwrap_or((None, name));
            LayeredSceneObject {
                object_id: format!("overlay:{name}"),
                layer_id: format!("{kind:?}"),
                source_node_id,
                source_field_path: source_node_id
                    .map(|node_id| format!("graph.nodes[{node_id}].{field}"))
                    .unwrap_or_else(|| format!("runtime.current_event.{name}")),
                asset_path: None,
                character_name: None,
                expression: None,
                object_index: 0,
                x: None,
                y: None,
                scale: None,
                z_index,
                visible: true,
                locked: true,
                kind,
            }
        })
        .collect()
}

pub(super) fn overlays_from_event(
    event: &EventCompiled,
    locale: &str,
    catalog: Option<&LocalizationCatalog>,
) -> Vec<ComposerOverlay> {
    match event {
        EventCompiled::Dialogue(dialogue) => vec![ComposerOverlay::Dialogue {
            speaker: localize(dialogue.speaker.as_ref(), locale, catalog),
            text: localize(dialogue.text.as_ref(), locale, catalog),
        }],
        EventCompiled::Choice(choice) => vec![ComposerOverlay::Choice {
            prompt: localize(choice.prompt.as_ref(), locale, catalog),
            options: choice
                .options
                .iter()
                .map(|option| localize(option.text.as_ref(), locale, catalog))
                .collect(),
        }],
        EventCompiled::Transition(transition) => vec![ComposerOverlay::Transition {
            kind: transition.kind.to_string(),
            duration_ms: transition.duration_ms,
        }],
        _ => Vec::new(),
    }
}

pub(super) fn overlays_from_authoring_node(
    node: &StoryNode,
    locale: &str,
    catalog: Option<&LocalizationCatalog>,
) -> Vec<ComposerOverlay> {
    match node {
        StoryNode::Dialogue { speaker, text } => vec![ComposerOverlay::Dialogue {
            speaker: localize(speaker, locale, catalog),
            text: localize(text, locale, catalog),
        }],
        StoryNode::Choice { prompt, options } => vec![ComposerOverlay::Choice {
            prompt: localize(prompt, locale, catalog),
            options: options
                .iter()
                .map(|option| localize(option, locale, catalog))
                .collect(),
        }],
        StoryNode::Transition {
            kind, duration_ms, ..
        } => vec![ComposerOverlay::Transition {
            kind: kind.clone(),
            duration_ms: *duration_ms,
        }],
        _ => Vec::new(),
    }
}

fn localize(value: &str, locale: &str, catalog: Option<&LocalizationCatalog>) -> String {
    if let Some(key) = crate::localization_key(value) {
        catalog
            .map(|catalog| catalog.resolve_or_key(locale, key))
            .unwrap_or_else(|| key.to_string())
    } else {
        value.to_string()
    }
}
