use std::collections::BTreeMap;

use eframe::egui;
use visual_novel_engine::{
    resolve_layout,
    runtime::{DialogueRaw, Engine, EventRaw, SceneUpdateRaw, ScriptRaw},
    DisplayProfile, HeadlessSceneFramePresenter, LayoutPolicy, LayoutRect, RenderCommand,
    ResourceLimiter, SceneFrame, SceneFramePresenter, SecurityPolicy, StageProfile, UiTheme,
};
use visual_novel_gui::editor::EguiSceneFramePresenter;
use visual_novel_runtime::RuntimeSceneFramePresenter;

fn raw_input(width: f32, height: f32, events: Vec<egui::Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        events,
        ..Default::default()
    }
}

fn pointer_button_event(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    }
}

#[test]
fn egui_scene_frame_presenter_implements_shared_contract() {
    let frame = SceneFrame {
        frame_schema: "vnengine.scene_frame.v1".to_string(),
        commands: vec![
            RenderCommand::Clear {
                color: "stage.background".to_string(),
            },
            RenderCommand::Panel {
                style: "dialogue_box".to_string(),
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 120.0,
                },
            },
        ],
        ..Default::default()
    };
    let mut presenter = EguiSceneFramePresenter::default();
    let response = presenter.present(
        &frame,
        &DisplayProfile::new(800.0, 600.0),
        &UiTheme::default(),
    );

    assert!(response.activated_actions.is_empty());
    assert!(
        response
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("no resolved layout")),
        "presenter should expose fallback-layout diagnostics"
    );
}

#[test]
fn theme_switching_and_presenter_parity_use_same_scene_frame_contract() {
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: Some("bg/parity.png".to_string()),
                music: None,
                characters: Vec::new(),
            }),
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Ava".to_string(),
                text: "Same frame everywhere".to_string(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )
    .expect("engine");
    engine.step().expect("scene");
    let mut frame = engine.scene_frame();
    let display = DisplayProfile::new(1280.0, 720.0);
    frame.layout = Some(resolve_layout(
        display.clone(),
        StageProfile::default(),
        LayoutPolicy::default(),
    ));

    let mut theme = UiTheme::default();
    theme
        .colors
        .insert("dialogue.background".to_string(), "#203040EE".to_string());
    let command_count = frame.commands.len();

    let mut egui = EguiSceneFramePresenter::default();
    let mut runtime = RuntimeSceneFramePresenter::default();
    let mut headless = HeadlessSceneFramePresenter::default();
    let egui_response = egui.present(&frame, &display, &theme);
    let runtime_response = runtime.present(&frame, &display, &theme);
    let headless_response = headless.present(&frame, &display, &theme);

    assert!(egui_response.activated_actions.is_empty());
    assert!(runtime_response.activated_actions.is_empty());
    assert!(headless_response.activated_actions.is_empty());
    assert_eq!(runtime.last_command_count, command_count);
    assert_eq!(
        frame.commands.len(),
        command_count,
        "theme switching must not rebuild story logic"
    );
    assert!(frame.commands.iter().any(
        |command| matches!(command, RenderCommand::Panel { style, .. } if style == "dialogue_box")
    ));
}

#[test]
fn egui_scene_frame_presenter_paints_layout_theme_and_button_actions() {
    let display = DisplayProfile::new(400.0, 300.0);
    let layout = resolve_layout(
        display.clone(),
        StageProfile::default(),
        LayoutPolicy::default(),
    );
    let mut frame = SceneFrame {
        frame_schema: "vnengine.scene_frame.v1".to_string(),
        layout: Some(layout),
        commands: vec![
            RenderCommand::Clear {
                color: "stage.background".to_string(),
            },
            RenderCommand::Panel {
                style: "dialogue_box".to_string(),
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 720.0,
                },
            },
            RenderCommand::Text {
                text: "Presenter paints text".to_string(),
                style: "dialogue.text".to_string(),
                rect: LayoutRect {
                    x: 48.0,
                    y: 48.0,
                    width: 520.0,
                    height: 80.0,
                },
            },
            RenderCommand::Button {
                id: "advance".to_string(),
                label: "Advance".to_string(),
                style: "primary".to_string(),
                rect: LayoutRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 720.0,
                },
            },
        ],
        ..Default::default()
    };
    let mut theme = UiTheme::default();
    theme
        .colors
        .insert("stage.background".to_string(), "#111827".to_string());
    theme
        .colors
        .insert("dialogue.text".to_string(), "#E5E7EB".to_string());
    theme
        .colors
        .insert("dialogue.background".to_string(), "#203040EE".to_string());
    if let Some(component) = theme.components.components.get_mut("dialogue_box") {
        component.background = Some("dialogue.background".to_string());
    }

    let ctx = egui::Context::default();
    let mut presenter = EguiSceneFramePresenter::default();
    let mut response = None;
    let output = ctx.run(raw_input(400.0, 300.0, Vec::new()), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            response = Some(presenter.present_egui(ui, &frame, &display, &theme));
        });
    });
    let passive = response.take().expect("presenter response");
    assert!(
        passive.diagnostics.is_empty(),
        "resolved frame should not require fallback diagnostics: {:?}",
        passive.diagnostics
    );
    assert!(
        !output.shapes.is_empty(),
        "present_egui should paint SceneFrame commands"
    );
    for (idx, clipped) in output.shapes.iter().enumerate() {
        let visible = clipped
            .shape
            .visual_bounding_rect()
            .intersect(clipped.clip_rect);
        if visible.is_positive() {
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 300.0))
                    .expand(1.0)
                    .contains_rect(visible),
                "scene frame command {idx} painted outside viewport: {visible:?}"
            );
        }
    }

    let click = egui::pos2(200.0, 150.0);
    let output = ctx.run(
        raw_input(
            400.0,
            300.0,
            vec![
                egui::Event::PointerMoved(click),
                pointer_button_event(click, true),
            ],
        ),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                response = Some(presenter.present_egui(ui, &frame, &display, &theme));
            });
        },
    );
    drop(output);
    let output = ctx.run(
        raw_input(
            400.0,
            300.0,
            vec![
                egui::Event::PointerMoved(click),
                pointer_button_event(click, false),
            ],
        ),
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                response = Some(presenter.present_egui(ui, &frame, &display, &theme));
            });
        },
    );
    drop(output);
    let clicked = response.take().expect("clicked presenter response");
    assert_eq!(
        clicked.activated_actions,
        vec!["advance".to_string()],
        "egui presenter should translate painted SceneFrame buttons into actions"
    );

    frame.commands.clear();
    let mut runtime = RuntimeSceneFramePresenter::default();
    let mut headless = HeadlessSceneFramePresenter::default();
    assert!(runtime
        .present(&frame, &display, &theme)
        .diagnostics
        .is_empty());
    assert!(headless
        .present(&frame, &display, &theme)
        .diagnostics
        .is_empty());
    assert_eq!(runtime.last_command_count, 0);
}
