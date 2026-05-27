mod objects;
mod overlays;
mod presentation;
mod preview;
mod runtime_objects;
mod types;

use crate::localization::LocalizationCatalog;
use crate::runtime::Engine;

use super::NodeGraph;
use objects::{collect_authoring_objects, preview_engine_for_selection};
use overlays::{
    overlay_layer_objects, overlay_source_for_authoring_node, overlays_from_authoring_node,
    overlays_from_event,
};
use runtime_objects::collect_visual_objects;

pub use objects::{
    apply_layer_overrides, list_layered_objects, move_scene_object, set_layer_locked,
    set_layer_visible,
};
pub use presentation::build_presentation_snapshot;
pub use preview::ComposerPreviewSession;
pub use types::{
    list_stage_layers, BackgroundFit, ComposerOverlay, ComposerSnapshot, LayerOverride,
    LayeredSceneObject, PresentationLayout, PresentationRect, PresentationSnapshot,
    PresentationTransition, StageLayerKind,
};

pub fn compose_scene_snapshot(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> ComposerSnapshot {
    let (stage_width, stage_height) = stage_resolution.unwrap_or((1280, 720));
    let preview_engine =
        engine.map(|engine| preview_engine_for_selection(engine, graph, selected_node_id));
    let snapshot_engine = preview_engine.as_ref().or(engine);
    let mut objects = snapshot_engine
        .map(|engine| collect_visual_objects(graph, engine))
        .unwrap_or_default();
    if objects.is_empty() {
        collect_authoring_objects(graph, selected_node_id, &mut objects);
    }

    let mut overlays = Vec::new();
    if let Some(node_id) = selected_node_id {
        if let Some(node) = graph.get_node(node_id) {
            overlays.extend(overlays_from_authoring_node(
                node,
                locale.unwrap_or("en"),
                catalog,
            ));
        }
    }
    if overlays.is_empty() {
        if let Some(engine) = snapshot_engine {
            if let Ok(event) = engine.current_event() {
                overlays.extend(overlays_from_event(&event, locale.unwrap_or("en"), catalog));
            }
        }
    }
    if overlays.is_empty() {
        if let Some(engine) = engine {
            if let Ok(event) = engine.current_event() {
                overlays.extend(overlays_from_event(&event, locale.unwrap_or("en"), catalog));
            }
        }
    }
    if overlays.is_empty() {
        if let Some(node_id) = selected_node_id {
            if let Some(node) = graph.get_node(node_id) {
                overlays.extend(overlays_from_authoring_node(
                    node,
                    locale.unwrap_or("en"),
                    catalog,
                ));
            }
        }
    }

    let overlay_source = selected_node_id
        .and_then(|node_id| {
            graph
                .get_node(node_id)
                .and_then(|node| overlay_source_for_authoring_node(node_id, node))
        })
        .or_else(|| {
            snapshot_engine.and_then(|engine| {
                graph
                    .node_for_event_ip(engine.state().position)
                    .and_then(|node_id| {
                        graph
                            .get_node(node_id)
                            .and_then(|node| overlay_source_for_authoring_node(node_id, node))
                    })
            })
        });
    objects.extend(overlay_layer_objects(&overlays, overlay_source));

    ComposerSnapshot {
        schema: "vnengine.composer_snapshot.v1".to_string(),
        stage_width,
        stage_height,
        objects,
        overlays,
    }
}
