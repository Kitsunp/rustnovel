mod objects;
mod overlays;
mod presentation;
mod preview;
mod route;
mod runtime_objects;
mod types;

use crate::localization::LocalizationCatalog;
use crate::runtime::Engine;

use super::NodeGraph;
use objects::collect_authoring_objects;
use overlays::{
    overlay_layer_objects, overlay_source_for_authoring_node, overlays_from_authoring_node,
    overlays_from_event,
};
use runtime_objects::collect_visual_objects;

pub use objects::{
    apply_layer_overrides, list_layered_objects, move_scene_object, set_layer_locked,
    set_layer_visible, set_scene_object_pose,
};
pub use presentation::build_presentation_snapshot;
pub use preview::{advance_preview_engine, jump_preview_engine, ComposerPreviewSession};
pub use route::preview_scene_route;
pub use types::{
    list_stage_layers, BackgroundFit, ComposerOverlay, ComposerSnapshot, LayerOverride,
    LayeredSceneObject, PresentationLayout, PresentationRect, PresentationSnapshot,
    PresentationTransition, StageLayerKind,
};

pub const DEFAULT_COMPOSER_RESOLUTION: (u32, u32) = (1280, 720);

pub fn compose_scene_snapshot(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> ComposerSnapshot {
    compose_snapshot_with_preview(
        graph,
        selected_node_id,
        stage_resolution,
        engine,
        locale,
        catalog,
    )
    .0
}

fn compose_snapshot_with_preview(
    graph: &NodeGraph,
    selected_node_id: Option<u32>,
    stage_resolution: Option<(u32, u32)>,
    engine: Option<&Engine>,
    locale: Option<&str>,
    catalog: Option<&LocalizationCatalog>,
) -> (ComposerSnapshot, Option<Engine>) {
    let (stage_width, stage_height) = stage_resolution.unwrap_or(DEFAULT_COMPOSER_RESOLUTION);
    let (preview_engine, preview_error) = match engine
        .map(|engine| preview_scene_route(engine, graph, selected_node_id))
        .transpose()
    {
        Ok(preview) => (preview, None),
        Err(error) => (None, Some(error.to_string())),
    };
    let snapshot_engine = preview_engine.as_ref().map(|(engine, _)| engine);
    let mut objects = match &preview_engine {
        Some((engine, visited)) => {
            collect_visual_objects(graph, engine, selected_node_id.map(|_| visited.as_slice()))
        }
        None => {
            let mut objects = Vec::new();
            collect_authoring_objects(graph, selected_node_id, &mut objects);
            objects
        }
    };

    let mut overlays = Vec::new();
    let mut overlay_source = None;
    let locale = locale.unwrap_or("en");
    if let Some((node_id, node)) =
        selected_node_id.and_then(|id| graph.get_node(id).map(|node| (id, node)))
    {
        overlays = overlays_from_authoring_node(node, locale, catalog);
        overlay_source = overlay_source_for_authoring_node(node_id, node);
    }
    if overlays.is_empty() {
        // The live event can supply the active dialogue in an unselected preview,
        // but it does not belong to a reconstructed route for a selected node.
        let live_event = engine.filter(|_| selected_node_id.is_none() && preview_error.is_none());
        for source in snapshot_engine.into_iter().chain(live_event) {
            if let Ok(event) = source.current_event_ref() {
                overlays = overlays_from_event(event, locale, catalog);
                if !overlays.is_empty() {
                    overlay_source =
                        graph
                            .node_for_event_ip(source.state().position)
                            .and_then(|id| {
                                graph
                                    .get_node(id)
                                    .and_then(|node| overlay_source_for_authoring_node(id, node))
                            });
                    break;
                }
            }
        }
    }
    objects.extend(overlay_layer_objects(&overlays, overlay_source));

    (
        ComposerSnapshot {
            schema: "vnengine.composer_snapshot.v1".to_string(),
            stage_width,
            stage_height,
            objects,
            overlays,
            preview_error,
        },
        preview_engine.map(|(engine, _)| engine),
    )
}
