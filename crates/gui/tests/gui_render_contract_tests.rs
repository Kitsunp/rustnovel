use std::collections::HashMap;

use eframe::egui;
use visual_novel_engine::{
    authoring::composer::{
        LayeredSceneObject, PresentationLayout, PresentationRect, PresentationSnapshot,
        StageLayerKind,
    },
    runtime::{ChoiceOptionRaw, ChoiceRaw, DialogueRaw, Engine, EventRaw, ScriptRaw},
    EntityId, PropertyType, ResourceLimiter, SceneState, SecurityPolicy, Timeline,
};
use visual_novel_gui::editor::{
    node_rendering::render_context_menu,
    resource_service::EditorResourceService,
    timeline_panel::{add_keyframe, TimelinePanel},
    visual_composer::VisualComposerPanelParams,
    workbench::{
        layout::{resolve_editor_dock_layout, EditorDockVisibility},
        LayoutOverrides,
    },
    BackgroundFit, ComposerPreviewMode, ContextMenu, NodeEditorPanel, NodeGraph, PreviewQuality,
    RouteTreeView, StageFit, StoryNode, UndoStack, VisualComposerPanel,
};
use visual_novel_gui::{EditorMode, EditorWorkbench, VnConfig};

const VIEWPORT_MATRIX: &[(f32, f32)] = &[
    (320.0, 240.0),
    (800.0, 600.0),
    (1280.0, 720.0),
    (1920.0, 1080.0),
    (3440.0, 1440.0),
    (720.0, 1280.0),
];

const SCALE_MATRIX: &[f32] = &[0.75, 1.0, 2.0, 3.0];

fn raw_input(width: f32, height: f32, _scale: f32, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        events,
        ..Default::default()
    }
}

fn run_workbench_frame(
    ctx: &egui::Context,
    workbench: &mut EditorWorkbench,
    width: f32,
    height: f32,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(raw_input(width, height, 1.0, events), |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        workbench.ui(ctx);
    })
}

fn run_panel_frame(
    ctx: &egui::Context,
    width: f32,
    height: f32,
    events: Vec<egui::Event>,
    render: impl FnOnce(&egui::Context),
) -> egui::FullOutput {
    ctx.run(raw_input(width, height, 1.0, events), |ctx| {
        ctx.set_visuals(egui::Visuals::dark());
        render(ctx);
    })
}

fn assert_rect_finite(label: &str, rect: egui::Rect) {
    assert!(
        rect.min.x.is_finite()
            && rect.min.y.is_finite()
            && rect.max.x.is_finite()
            && rect.max.y.is_finite(),
        "{label} has invalid coordinates: {rect:?}"
    );
    assert!(
        rect.width().is_finite() && rect.height().is_finite(),
        "{label} has invalid size: {rect:?}"
    );
}

fn assert_used_rect_inside_screen(label: &str, used: egui::Rect, screen: egui::Rect) {
    assert_rect_finite(label, used);
    let allowed = screen.expand(1.0);
    assert!(
        allowed.contains_rect(used),
        "{label} escaped the viewport: used={used:?}, screen={screen:?}"
    );
}

fn assert_visible_shapes_inside_screen(
    label: &str,
    shapes: &[egui::epaint::ClippedShape],
    screen: egui::Rect,
) {
    let allowed = screen.expand(1.0);
    for (idx, clipped) in shapes.iter().enumerate() {
        assert_rect_finite(&format!("{label} shape {idx} clip"), clipped.clip_rect);
        let visible = clipped
            .shape
            .visual_bounding_rect()
            .intersect(clipped.clip_rect);
        if visible.is_positive() {
            assert_rect_finite(&format!("{label} shape {idx} visible"), visible);
            assert!(
                allowed.contains_rect(visible),
                "{label} painted outside viewport in shape {idx}: visible={visible:?}, screen={screen:?}"
            );
        }
    }
}

#[test]
fn editor_workbench_renders_real_egui_frames_inside_viewport_matrix() {
    for &(width, height) in VIEWPORT_MATRIX {
        for &scale in SCALE_MATRIX {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
            let mut workbench = EditorWorkbench::new(VnConfig::default());
            workbench.mode = EditorMode::Editor;
            workbench.show_graph = true;
            workbench.show_inspector = true;
            workbench.show_timeline = true;
            workbench.show_asset_browser = true;
            workbench.show_validation = true;

            let warmup = ctx.run(raw_input(width, height, scale, Vec::new()), |ctx| {
                ctx.set_visuals(egui::Visuals::dark());
                workbench.ui(ctx);
            });
            drop(warmup);
            let output = ctx.run(raw_input(width, height, scale, Vec::new()), |ctx| {
                ctx.set_visuals(egui::Visuals::dark());
                workbench.ui(ctx);
            });
            assert!(
                !output.shapes.is_empty(),
                "editor frame produced no paint shapes at {width}x{height} scale {scale}"
            );
            assert_visible_shapes_inside_screen(
                &format!("editor frame {width}x{height} scale {scale}"),
                &output.shapes,
                screen,
            );
        }
    }
}

#[test]
fn loaded_graph_pending_fit_uses_visible_graph_panel() {
    let ctx = egui::Context::default();
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.mode = EditorMode::Editor;
    workbench.show_graph = true;
    workbench.show_inspector = false;
    workbench.show_timeline = false;
    workbench.show_asset_browser = false;
    let node = workbench.node_graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "Loaded far away".to_string(),
        },
        egui::pos2(2400.0, 1600.0),
    );
    workbench.pending_graph_fit = true;

    let output = run_workbench_frame(&ctx, &mut workbench, 1280.0, 720.0, Vec::new());

    assert!(!output.shapes.is_empty());
    assert!(!workbench.pending_graph_fit);
    assert_ne!(
        workbench.node_graph.pan(),
        egui::Vec2::ZERO,
        "loaded graphs should fit to the real node editor viewport instead of leaving the camera at empty space"
    );
    let pos = workbench.node_graph.get_node_pos(node).expect("node pos");
    let screen_pos = (pos.to_vec2() + workbench.node_graph.pan()) * workbench.node_graph.zoom();
    assert!(
        screen_pos.x.is_finite()
            && screen_pos.y.is_finite()
            && screen_pos.x >= -1.0
            && screen_pos.y >= -1.0
            && screen_pos.x <= 1280.0
            && screen_pos.y <= 720.0,
        "pending fit should bring node into the visible editor frame, got {screen_pos:?}"
    );
}

#[test]
fn editor_workbench_splitters_resize_real_panels_end_to_end() {
    #[derive(Clone, Copy)]
    enum SplitterProbe {
        Asset,
        Graph,
        Inspector,
    }

    impl SplitterProbe {
        fn label(self) -> &'static str {
            match self {
                Self::Asset => "asset browser",
                Self::Graph => "graph",
                Self::Inspector => "inspector",
            }
        }

        fn delta(self) -> f32 {
            match self {
                Self::Asset | Self::Graph => 72.0,
                Self::Inspector => -72.0,
            }
        }

        fn splitter(
            self,
            layout: visual_novel_gui::editor::workbench::layout::EditorDockLayout,
        ) -> Option<visual_novel_gui::editor::workbench::layout::WorkspacePanelRect> {
            match self {
                Self::Asset => layout.asset_splitter,
                Self::Graph => layout.graph_splitter,
                Self::Inspector => layout.inspector_splitter,
            }
        }

        fn override_value(self, overrides: &LayoutOverrides) -> Option<f32> {
            match self {
                Self::Asset => overrides.asset_width,
                Self::Graph => overrides.graph_width,
                Self::Inspector => overrides.inspector_width,
            }
        }
    }

    let width = 1280.0;
    let height = 720.0;
    let visibility = EditorDockVisibility {
        asset_browser: true,
        graph: true,
        inspector: true,
    };

    for probe in [
        SplitterProbe::Asset,
        SplitterProbe::Graph,
        SplitterProbe::Inspector,
    ] {
        let label = probe.label();
        let delta = probe.delta();
        let ctx = egui::Context::default();
        let mut workbench = EditorWorkbench::new(VnConfig::default());
        workbench.mode = EditorMode::Editor;
        workbench.show_asset_browser = true;
        workbench.show_graph = true;
        workbench.show_inspector = true;
        workbench.show_timeline = true;
        workbench.show_validation = true;
        workbench.layout_overrides = LayoutOverrides::default();

        let initial_layout =
            resolve_editor_dock_layout(width, height, &workbench.layout_overrides, visibility);
        let splitter_rect = probe
            .splitter(initial_layout)
            .expect("splitter should be visible");
        let start = egui::pos2(
            splitter_rect.x + splitter_rect.w * 0.5,
            (splitter_rect.y + 150.0).clamp(90.0, height - 180.0),
        );
        let end = start + egui::vec2(delta, 0.0);
        let initial_width = probe.override_value(&workbench.layout_overrides);

        let output = run_workbench_frame(&ctx, &mut workbench, width, height, Vec::new());
        drop(output);
        let output = run_workbench_frame(
            &ctx,
            &mut workbench,
            width,
            height,
            vec![
                egui::Event::PointerMoved(start),
                pointer_button_event(start, egui::PointerButton::Primary, true),
            ],
        );
        drop(output);
        let output = run_workbench_frame(
            &ctx,
            &mut workbench,
            width,
            height,
            vec![egui::Event::PointerMoved(end)],
        );
        drop(output);
        let output = run_workbench_frame(
            &ctx,
            &mut workbench,
            width,
            height,
            vec![pointer_button_event(
                end,
                egui::PointerButton::Primary,
                false,
            )],
        );

        let resized_width = probe.override_value(&workbench.layout_overrides);
        assert_ne!(
            resized_width, initial_width,
            "{label} splitter drag did not persist a layout override"
        );
        assert!(
            resized_width.is_some(),
            "{label} splitter drag should store a manual width override"
        );
        let resized_layout =
            resolve_editor_dock_layout(width, height, &workbench.layout_overrides, visibility);
        if let Some(inspector) = resized_layout.inspector {
            assert!(
                resized_layout.composer.x + resized_layout.composer.w <= inspector.x + 0.1,
                "{label} resize left composer and inspector overlapping: composer={:?}, inspector={:?}",
                resized_layout.composer,
                inspector
            );
        }
        assert!(
            resized_layout.composer.w >= resized_layout.panel_sizes.central_min - 1.0,
            "{label} resize starved the visual composer: {:?}",
            resized_layout.composer
        );
        assert_visible_shapes_inside_screen(
            &format!("{label} splitter resized frame"),
            &output.shapes,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height)),
        );
    }
}

#[test]
fn editor_workbench_composer_and_inspector_shapes_do_not_overlap_semantically() {
    for &(width, height) in &[(800.0, 600.0), (1280.0, 720.0), (1920.0, 1080.0)] {
        let ctx = egui::Context::default();
        let mut workbench = EditorWorkbench::new(VnConfig::default());
        workbench.mode = EditorMode::Editor;
        workbench.show_asset_browser = true;
        workbench.show_graph = true;
        workbench.show_inspector = true;
        workbench.show_timeline = true;
        workbench.show_validation = true;
        workbench.layout_overrides = LayoutOverrides::default();

        let output = run_workbench_frame(&ctx, &mut workbench, width, height, Vec::new());
        let dock = resolve_editor_dock_layout(
            width,
            height,
            &workbench.layout_overrides,
            EditorDockVisibility {
                asset_browser: true,
                graph: true,
                inspector: true,
            },
        );
        let inspector = dock.inspector.expect("inspector visible");
        let composer_x = dock.composer.x..=(dock.composer.x + dock.composer.w);
        let inspector_x = inspector.x..=(inspector.x + inspector.w);

        assert!(
            dock.composer.x + dock.composer.w <= inspector.x + 0.1,
            "layout resolver produced overlapping composer/inspector at {width}x{height}: composer={:?}, inspector={inspector:?}",
            dock.composer
        );

        for (idx, clipped) in output.shapes.iter().enumerate() {
            let visible = clipped
                .shape
                .visual_bounding_rect()
                .intersect(clipped.clip_rect);
            if !visible.is_positive() || visible.height() < 2.0 || visible.width() < 2.0 {
                continue;
            }
            if visible.bottom() < 72.0 || clipped.clip_rect.width() > width * 0.80 {
                continue;
            }
            let touches_composer =
                composer_x.contains(&visible.left()) || composer_x.contains(&visible.right());
            let touches_inspector =
                inspector_x.contains(&visible.left()) || inspector_x.contains(&visible.right());
            assert!(
                !(touches_composer && touches_inspector),
                "shape {idx} bridges Visual Composer and Inspector at {width}x{height}: visible={visible:?}, composer={:?}, inspector={inspector:?}",
                dock.composer
            );
        }
    }
}

#[test]
fn canvas_context_menu_renders_inside_tiny_and_edge_viewports() {
    for &(width, height) in &[(240.0, 190.0), (320.0, 240.0), (800.0, 600.0)] {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut graph = NodeGraph::new();
        graph.context_menu = Some(ContextMenu::for_canvas(
            egui::pos2(width - 2.0, height - 2.0),
            egui::pos2(10.0, 20.0),
        ));

        let output = ctx.run(raw_input(width, height, 1.0, Vec::new()), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_context_menu(&mut graph, ui);
            });
        });
        let used = ctx.used_rect();

        assert!(
            !output.shapes.is_empty(),
            "context menu frame produced no paint shapes at {width}x{height}"
        );
        assert_visible_shapes_inside_screen(
            &format!("canvas context menu {width}x{height}"),
            &output.shapes,
            screen,
        );
        assert_used_rect_inside_screen(
            &format!("canvas context menu {width}x{height}"),
            used,
            screen,
        );
    }
}

#[test]
fn node_context_menu_renders_scene_actions_inside_edge_viewports() {
    for &(width, height) in &[(320.0, 240.0), (800.0, 600.0)] {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut graph = NodeGraph::new();
        let node_id = graph.add_node(
            StoryNode::Scene {
                profile: Some("intro".to_string()),
                background: Some("bg/room.png".to_string()),
                music: None,
                characters: Vec::new(),
            },
            egui::pos2(120.0, 80.0),
        );
        graph.context_menu = Some(ContextMenu::for_node(
            node_id,
            egui::pos2(width - 4.0, height - 4.0),
        ));

        let output = ctx.run(raw_input(width, height, 1.0, Vec::new()), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_context_menu(&mut graph, ui);
            });
        });

        assert!(
            !output.shapes.is_empty(),
            "node context menu frame produced no paint shapes at {width}x{height}"
        );
        assert_visible_shapes_inside_screen(
            &format!("node context menu {width}x{height}"),
            &output.shapes,
            screen,
        );
        assert!(
            graph.context_menu.is_some(),
            "rendering the node context menu should not consume an action without a click"
        );
    }
}

#[test]
fn editor_auxiliary_windows_render_without_escaping_viewports() {
    for &(width, height) in &[(800.0, 600.0), (1280.0, 720.0), (1920.0, 1080.0)] {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut workbench = EditorWorkbench::new(VnConfig::default());
        workbench.mode = EditorMode::Editor;
        workbench.show_theme_editor = true;
        workbench.show_export_report_panel = true;
        workbench.show_layout_debug_overlay = true;
        workbench.show_scene_frame_inspector = true;
        workbench.show_profiler_cache_panel = true;

        let warmup = ctx.run(raw_input(width, height, 1.0, Vec::new()), |ctx| {
            ctx.set_visuals(egui::Visuals::dark());
            workbench.ui(ctx);
        });
        drop(warmup);
        let output = ctx.run(raw_input(width, height, 1.0, Vec::new()), |ctx| {
            ctx.set_visuals(egui::Visuals::dark());
            workbench.ui(ctx);
        });

        assert!(
            !output.shapes.is_empty(),
            "auxiliary windows produced no paint shapes at {width}x{height}"
        );
        assert_visible_shapes_inside_screen(
            &format!("auxiliary windows {width}x{height}"),
            &output.shapes,
            screen,
        );
    }
}

fn stress_presentation_snapshot() -> PresentationSnapshot {
    let long_path =
        "graph.nodes[42].scene_patch.characters[12].pose.asset_with_a_very_long_unbroken_name";
    PresentationSnapshot {
        schema: "vnengine.presentation_snapshot.v1".to_string(),
        stage_width: 1280,
        stage_height: 720,
        safe_area: PresentationRect {
            x: 64.0,
            y: 36.0,
            width: 1152.0,
            height: 648.0,
        },
        layout: PresentationLayout {
            dialogue_rect: None,
            choices_rect: None,
        },
        visual_background: Some("assets/backgrounds/very_long_background_name.png".to_string()),
        visual_music: None,
        visual_character_count: 8,
        transition: None,
        objects: (0..14)
            .map(|idx| LayeredSceneObject {
                object_id: format!("character-instance-{idx}"),
                layer_id: "character_main".to_string(),
                source_node_id: Some(42),
                source_field_path: format!("{long_path}.{idx}"),
                asset_path: Some(format!("assets/characters/hero_variant_{idx}.png")),
                character_name: Some(format!("Character {idx}")),
                expression: Some("neutral".to_string()),
                object_index: idx,
                x: Some(80 + idx as i32 * 12),
                y: Some(120),
                scale: Some(1.0),
                z_index: idx as i32,
                visible: true,
                locked: idx % 3 == 0,
                kind: StageLayerKind::CharacterMain,
            })
            .collect(),
        overlays: Vec::new(),
        provenance: vec!["gui-render-contract".to_string()],
    }
}

#[test]
fn visual_composer_panel_renders_real_responsive_rows_without_horizontal_escape() {
    for &(width, height) in &[
        (260.0, 240.0),
        (320.0, 240.0),
        (560.0, 360.0),
        (900.0, 520.0),
    ] {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut scene = SceneState::new();
        let engine: Option<Engine> = None;
        let mut preview_quality = PreviewQuality::High;
        let mut stage_fit = StageFit::Fill;
        let mut background_fit = BackgroundFit::Cover;
        let mut preview_mode = ComposerPreviewMode::RuntimeInherited;
        let mut image_cache = HashMap::new();
        let mut image_failures = HashMap::new();
        let mut resource_service = EditorResourceService::new();
        let mut selected_entity_id = None;
        let layer_overrides = HashMap::new();
        let entity_owners = HashMap::new();
        let snapshot = stress_presentation_snapshot();
        let selected_node = StoryNode::Choice {
            prompt: "A prompt with a deliberately long text that must not force the toolbar out of its dock"
                .to_string(),
            options: vec![
                "First very long option that should wrap rather than expand".to_string(),
                "Second very long option that should wrap rather than expand".to_string(),
            ],
        };

        let output = run_panel_frame(&ctx, width, height, Vec::new(), |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show(ctx, |ui| {
                    let panel_rect =
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
                    ui.allocate_ui_at_rect(panel_rect, |ui| {
                        ui.set_clip_rect(panel_rect);
                        let mut panel = VisualComposerPanel::new(VisualComposerPanelParams {
                            scene: &mut scene,
                            engine: &engine,
                            project_root: None,
                            stage_resolution: Some((1280, 720)),
                            preview_quality: &mut preview_quality,
                            stage_fit: &mut stage_fit,
                            background_fit: &mut background_fit,
                            preview_mode: &mut preview_mode,
                            image_cache: &mut image_cache,
                            image_failures: &mut image_failures,
                            resource_service: &mut resource_service,
                            selected_entity_id: &mut selected_entity_id,
                            layer_overrides: &layer_overrides,
                            active_event_node_id: Some(42),
                            selected_authoring_node_id: Some(42),
                            selected_authoring_node: Some(&selected_node),
                            presentation_snapshot: Some(&snapshot),
                        });
                        let action = panel.ui(ui, &entity_owners);
                        assert!(
                            action.is_none(),
                            "passive composer render should not emit mutation actions"
                        );
                    });
                });
        });

        assert!(
            !output.shapes.is_empty(),
            "visual composer produced no paint at {width}x{height}"
        );
        assert_visible_shapes_inside_screen(
            &format!("visual composer direct {width}x{height}"),
            &output.shapes,
            screen,
        );
        assert!(
            ctx.used_rect().right() <= width + 1.0,
            "visual composer requested horizontal space outside its dock at {width}x{height}: {:?}",
            ctx.used_rect()
        );
        if height >= 500.0 {
            let max_visible_shape_height = output
                .shapes
                .iter()
                .map(|shape| {
                    shape
                        .shape
                        .visual_bounding_rect()
                        .intersect(shape.clip_rect)
                        .height()
                })
                .fold(0.0, f32::max);
            assert!(
                max_visible_shape_height >= height * 0.35,
                "selected choice controls should not push the visual composer stage into a tiny strip at {width}x{height}; max painted height was {max_visible_shape_height}"
            );
        }
    }
}

fn click_timeline(
    ctx: &egui::Context,
    timeline: &mut Timeline,
    current_time: &mut u32,
    is_playing: &mut bool,
    pos: egui::Pos2,
) {
    let output = run_panel_frame(
        ctx,
        640.0,
        180.0,
        vec![
            egui::Event::PointerMoved(pos),
            pointer_button_event(pos, egui::PointerButton::Primary, true),
        ],
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                TimelinePanel::new(timeline, current_time, is_playing)
                    .with_selected_entity(Some(7))
                    .ui(ui);
            });
        },
    );
    drop(output);
    let output = run_panel_frame(
        ctx,
        640.0,
        180.0,
        vec![
            egui::Event::PointerMoved(pos),
            pointer_button_event(pos, egui::PointerButton::Primary, false),
        ],
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                TimelinePanel::new(timeline, current_time, is_playing)
                    .with_selected_entity(Some(7))
                    .ui(ui);
            });
        },
    );
    drop(output);
}

#[test]
fn timeline_panel_supports_real_transport_scrub_and_add_keyframe_ui() {
    let ctx = egui::Context::default();
    let mut timeline = Timeline::new(60);
    let mut current_time = 30;
    let mut is_playing = false;
    add_keyframe(&mut timeline, 7, PropertyType::PositionX, 120, 240).expect("seed keyframe");

    let output = run_panel_frame(&ctx, 640.0, 180.0, Vec::new(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            TimelinePanel::new(&mut timeline, &mut current_time, &mut is_playing)
                .with_selected_entity(Some(7))
                .ui(ui);
        });
    });
    assert_visible_shapes_inside_screen(
        "timeline initial frame",
        &output.shapes,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 180.0)),
    );

    click_timeline(
        &ctx,
        &mut timeline,
        &mut current_time,
        &mut is_playing,
        egui::pos2(86.0, 18.0),
    );
    assert!(is_playing, "clicking Play should toggle playback");

    current_time = 60;
    is_playing = true;
    click_timeline(
        &ctx,
        &mut timeline,
        &mut current_time,
        &mut is_playing,
        egui::pos2(124.0, 18.0),
    );
    assert!(!is_playing, "clicking Stop should stop playback");
    assert_eq!(current_time, 0, "Stop should seek to the beginning");

    current_time = 60;
    click_timeline(
        &ctx,
        &mut timeline,
        &mut current_time,
        &mut is_playing,
        egui::pos2(176.0, 18.0),
    );
    assert_eq!(current_time, 0, "Rewind should seek to the beginning");

    let drag_start = egui::pos2(24.0, 48.0);
    let drag_end = egui::pos2(600.0, 48.0);
    let output = run_panel_frame(
        &ctx,
        640.0,
        180.0,
        vec![
            egui::Event::PointerMoved(drag_start),
            pointer_button_event(drag_start, egui::PointerButton::Primary, true),
        ],
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                TimelinePanel::new(&mut timeline, &mut current_time, &mut is_playing)
                    .with_selected_entity(Some(7))
                    .ui(ui);
            });
        },
    );
    drop(output);
    let output = run_panel_frame(
        &ctx,
        640.0,
        180.0,
        vec![egui::Event::PointerMoved(drag_end)],
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                TimelinePanel::new(&mut timeline, &mut current_time, &mut is_playing)
                    .with_selected_entity(Some(7))
                    .ui(ui);
            });
        },
    );
    drop(output);
    assert!(
        current_time > 0,
        "dragging the scrubber should update current_time, got {current_time}"
    );

    current_time = 45;
    let tracks_before = timeline.track_count();
    let mut added_from_ui = false;
    for x in [70.0, 92.0, 116.0, 140.0] {
        click_timeline(
            &ctx,
            &mut timeline,
            &mut current_time,
            &mut is_playing,
            egui::pos2(x, 78.0),
        );
        if timeline.track_count() > tracks_before
            || timeline
                .tracks()
                .any(|track| track.target == EntityId::new(7) && track.len() > 1)
        {
            added_from_ui = true;
            break;
        }
    }
    assert!(
        added_from_ui,
        "clicking the timeline Add control should insert a selected-entity keyframe"
    );
}

#[test]
fn route_tree_view_renders_real_engine_snapshot_inside_viewport() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "Route opening".to_string(),
            }),
            EventRaw::Choice(ChoiceRaw {
                prompt: "Pick a route".to_string(),
                options: vec![
                    ChoiceOptionRaw {
                        text: "A".to_string(),
                        target: "route_a".to_string(),
                    },
                    ChoiceOptionRaw {
                        text: "B".to_string(),
                        target: "route_b".to_string(),
                    },
                ],
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "A".to_string(),
                text: "Route A".to_string(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "B".to_string(),
                text: "Route B".to_string(),
            }),
        ],
        std::collections::BTreeMap::from([
            ("start".to_string(), 0),
            ("route_a".to_string(), 2),
            ("route_b".to_string(), 3),
        ]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine should compile route script");
    engine.step().expect("first dialogue step");
    let route_tree = engine.route_tree();
    assert!(
        route_tree.coverage.total_nodes >= 3,
        "route tree fixture should expose branches"
    );

    let ctx = egui::Context::default();
    let output = run_panel_frame(&ctx, 420.0, 260.0, Vec::new(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            RouteTreeView::new(&route_tree)
                .with_choice_history(engine.choice_history())
                .with_max_height(220.0)
                .ui(ui);
        });
    });

    assert!(
        !output.shapes.is_empty(),
        "RouteTreeView should paint the route snapshot"
    );
    assert_visible_shapes_inside_screen(
        "route tree view",
        &output.shapes,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 260.0)),
    );
}

fn run_node_editor_frame(
    ctx: &egui::Context,
    graph: &mut NodeGraph,
    undo: &mut UndoStack,
    events: Vec<egui::Event>,
) {
    let output = ctx.run(raw_input(900.0, 640.0, 1.0, events), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            NodeEditorPanel::new(graph, undo).ui(ui);
        });
    });
    drop(output);
}

fn pointer_button_event(
    pos: egui::Pos2,
    button: egui::PointerButton,
    pressed: bool,
) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}

#[test]
fn egui_pointer_harness_registers_secondary_click_and_drag() {
    let ctx = egui::Context::default();
    let mut clicked = false;
    let mut dragged = false;
    let pos = egui::pos2(420.0, 360.0);
    let drag_pos = egui::pos2(500.0, 410.0);

    let output = ctx.run(raw_input(900.0, 640.0, 1.0, Vec::new()), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        });
    });
    drop(output);

    let output = ctx.run(
        raw_input(
            900.0,
            640.0,
            1.0,
            vec![
                egui::Event::PointerMoved(pos),
                pointer_button_event(pos, egui::PointerButton::Secondary, true),
            ],
        ),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            });
        },
    );
    drop(output);
    let output = ctx.run(
        raw_input(
            900.0,
            640.0,
            1.0,
            vec![
                egui::Event::PointerMoved(pos),
                pointer_button_event(pos, egui::PointerButton::Secondary, false),
            ],
        ),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (response, _) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                clicked = response.secondary_clicked();
            });
        },
    );
    drop(output);

    let output = ctx.run(
        raw_input(
            900.0,
            640.0,
            1.0,
            vec![
                egui::Event::PointerMoved(pos),
                pointer_button_event(pos, egui::PointerButton::Secondary, true),
            ],
        ),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            });
        },
    );
    drop(output);
    let output = ctx.run(
        raw_input(900.0, 640.0, 1.0, vec![egui::Event::PointerMoved(drag_pos)]),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (response, _) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                dragged = response.dragged_by(egui::PointerButton::Secondary);
            });
        },
    );
    drop(output);

    assert!(clicked, "test harness must register secondary clicks");
    assert!(dragged, "test harness must register secondary drags");
}

#[test]
fn node_editor_empty_frame_renders_without_hanging() {
    let ctx = egui::Context::default();
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
    assert!(ctx.used_rect().is_finite());
}

#[test]
fn node_editor_secondary_press_frame_returns_without_hanging() {
    let ctx = egui::Context::default();
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let pos = egui::pos2(420.0, 360.0);
    run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![
            egui::Event::PointerMoved(pos),
            pointer_button_event(pos, egui::PointerButton::Secondary, true),
        ],
    );
    assert!(ctx.used_rect().is_finite());
}

#[test]
fn node_editor_secondary_release_frame_opens_canvas_menu() {
    let ctx = egui::Context::default();
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let pos = egui::pos2(420.0, 360.0);
    run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![
            egui::Event::PointerMoved(pos),
            pointer_button_event(pos, egui::PointerButton::Secondary, true),
        ],
    );
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![
            egui::Event::PointerMoved(pos),
            pointer_button_event(pos, egui::PointerButton::Secondary, false),
        ],
    );
    assert!(graph.context_menu.is_some());
}

#[test]
fn node_editor_secondary_drag_frame_pans_canvas() {
    let ctx = egui::Context::default();
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let drag_start = egui::pos2(420.0, 360.0);
    let drag_end = egui::pos2(500.0, 410.0);
    run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
    let pan_before = graph.pan;
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![
            egui::Event::PointerMoved(drag_start),
            pointer_button_event(drag_start, egui::PointerButton::Secondary, true),
        ],
    );
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![egui::Event::PointerMoved(drag_end)],
    );
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![pointer_button_event(
            drag_end,
            egui::PointerButton::Secondary,
            false,
        )],
    );

    assert!(graph.context_menu.is_none());
    assert_ne!(graph.pan, pan_before);
}

#[test]
fn node_editor_secondary_drag_move_frame_returns_without_hanging() {
    let ctx = egui::Context::default();
    let mut graph = NodeGraph::new();
    let mut undo = UndoStack::new();
    let drag_start = egui::pos2(420.0, 360.0);
    let drag_end = egui::pos2(500.0, 410.0);
    run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![
            egui::Event::PointerMoved(drag_start),
            pointer_button_event(drag_start, egui::PointerButton::Secondary, true),
        ],
    );
    run_node_editor_frame(
        &ctx,
        &mut graph,
        &mut undo,
        vec![egui::Event::PointerMoved(drag_end)],
    );
    assert!(ctx.used_rect().is_finite());
}

#[test]
fn node_editor_right_click_opens_menu_but_right_drag_pans_canvas() {
    let click_opened_canvas_menu = [
        egui::pos2(180.0, 160.0),
        egui::pos2(420.0, 360.0),
        egui::pos2(760.0, 560.0),
    ]
    .into_iter()
    .any(|click_pos| {
        let ctx = egui::Context::default();
        let mut graph = NodeGraph::new();
        let mut undo = UndoStack::new();
        run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![
                egui::Event::PointerMoved(click_pos),
                pointer_button_event(click_pos, egui::PointerButton::Secondary, true),
            ],
        );
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![
                egui::Event::PointerMoved(click_pos),
                pointer_button_event(click_pos, egui::PointerButton::Secondary, false),
            ],
        );
        graph
            .context_menu
            .as_ref()
            .is_some_and(|menu| menu.node_id.is_none())
    });
    assert!(
        click_opened_canvas_menu,
        "secondary click on empty canvas should open the canvas context menu"
    );

    let drag_panned = [
        egui::pos2(180.0, 160.0),
        egui::pos2(420.0, 360.0),
        egui::pos2(760.0, 560.0),
    ]
    .into_iter()
    .any(|drag_start| {
        let ctx = egui::Context::default();
        let mut graph = NodeGraph::new();
        let mut undo = UndoStack::new();
        let drag_end = drag_start + egui::vec2(80.0, 50.0);
        run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
        let pan_before = graph.pan;
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![
                egui::Event::PointerMoved(drag_start),
                pointer_button_event(drag_start, egui::PointerButton::Secondary, true),
            ],
        );
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![egui::Event::PointerMoved(drag_end)],
        );
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![pointer_button_event(
                drag_end,
                egui::PointerButton::Secondary,
                false,
            )],
        );
        graph.context_menu.is_none() && graph.pan != pan_before
    });
    assert!(
        drag_panned,
        "secondary drag should pan the graph without leaving a context menu open"
    );
}

#[test]
fn node_editor_right_click_on_node_targets_node_menu() {
    let node_menu_opened = [
        egui::pos2(188.0, 220.0),
        egui::pos2(210.0, 250.0),
        egui::pos2(260.0, 280.0),
    ]
    .into_iter()
    .any(|click_pos| {
        let ctx = egui::Context::default();
        let mut graph = NodeGraph::new();
        let mut undo = UndoStack::new();
        let node_id = graph.add_node(StoryNode::Start, egui::pos2(180.0, 160.0));
        run_node_editor_frame(&ctx, &mut graph, &mut undo, Vec::new());
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![
                egui::Event::PointerMoved(click_pos),
                pointer_button_event(click_pos, egui::PointerButton::Secondary, true),
            ],
        );
        run_node_editor_frame(
            &ctx,
            &mut graph,
            &mut undo,
            vec![
                egui::Event::PointerMoved(click_pos),
                pointer_button_event(click_pos, egui::PointerButton::Secondary, false),
            ],
        );
        graph.context_menu.as_ref().and_then(|menu| menu.node_id) == Some(node_id)
    });

    assert!(
        node_menu_opened,
        "secondary click on a node should target the node context menu"
    );
}
