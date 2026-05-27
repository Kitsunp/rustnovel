use crate::localization::LocalizationCatalog;
use crate::runtime::Engine;

use super::super::NodeGraph;
use super::{
    compose_scene_snapshot, ComposerOverlay, ComposerSnapshot, PresentationLayout,
    PresentationRect, PresentationSnapshot, PresentationTransition,
};

pub fn build_presentation_snapshot(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> PresentationSnapshot {
    let composer = compose_scene_snapshot(
        graph,
        selected_node_id,
        stage_resolution,
        engine,
        locale,
        catalog,
    );
    PresentationSnapshot::from_composer_snapshot(composer, engine)
}

impl PresentationSnapshot {
    pub fn from_composer_snapshot(snapshot: ComposerSnapshot, engine: Option<&Engine>) -> Self {
        let safe_area = calculate_safe_area(snapshot.stage_width, snapshot.stage_height);
        let layout = calculate_overlay_layout(&snapshot.overlays, &safe_area);
        let transition = snapshot.overlays.iter().find_map(|overlay| match overlay {
            ComposerOverlay::Transition { kind, duration_ms } => Some(PresentationTransition {
                kind: kind.clone(),
                duration_ms: *duration_ms,
            }),
            _ => None,
        });
        let provenance = snapshot
            .objects
            .iter()
            .map(|object| object.source_field_path.clone())
            .collect();
        let visual = engine.map(|engine| engine.visual_state());

        Self {
            schema: "vnengine.presentation_snapshot.v1".to_string(),
            stage_width: snapshot.stage_width,
            stage_height: snapshot.stage_height,
            safe_area,
            layout,
            visual_background: visual
                .and_then(|state| state.background.as_ref())
                .map(|value| value.as_ref().to_string()),
            visual_music: visual
                .and_then(|state| state.music.as_ref())
                .map(|value| value.as_ref().to_string()),
            visual_character_count: visual.map_or(0, |state| state.characters.len()),
            transition,
            objects: snapshot.objects,
            overlays: snapshot.overlays,
            provenance,
        }
    }
}

fn calculate_safe_area(stage_width: u32, stage_height: u32) -> PresentationRect {
    let margin_x = stage_width as f32 * 0.05;
    let margin_y = stage_height as f32 * 0.05;
    PresentationRect {
        x: margin_x,
        y: margin_y,
        width: (stage_width as f32 - margin_x * 2.0).max(0.0),
        height: (stage_height as f32 - margin_y * 2.0).max(0.0),
    }
}

fn calculate_overlay_layout(
    overlays: &[ComposerOverlay],
    safe_area: &PresentationRect,
) -> PresentationLayout {
    let has_dialogue = overlays
        .iter()
        .any(|overlay| matches!(overlay, ComposerOverlay::Dialogue { .. }));
    let has_choices = overlays
        .iter()
        .any(|overlay| matches!(overlay, ComposerOverlay::Choice { .. }));

    PresentationLayout {
        dialogue_rect: has_dialogue.then_some(PresentationRect {
            x: safe_area.x,
            y: safe_area.y + safe_area.height * 0.72,
            width: safe_area.width,
            height: safe_area.height * 0.24,
        }),
        choices_rect: has_choices.then_some(PresentationRect {
            x: safe_area.x + safe_area.width * 0.12,
            y: safe_area.y + safe_area.height * 0.18,
            width: safe_area.width * 0.76,
            height: safe_area.height * 0.56,
        }),
    }
}
