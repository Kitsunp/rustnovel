use std::collections::BTreeMap;
use std::fs;

use super::*;
use crate::VnConfig;
use visual_novel_engine::runtime::{DialogueRaw, EventRaw, ScriptRaw};

fn export_frame(
    workbench: &mut EditorWorkbench,
    ctx: &egui::Context,
    time: f64,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 960.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| workbench.render_export_wizard_window(ctx),
    )
}

fn click_export_button(
    workbench: &mut EditorWorkbench,
    ctx: &egui::Context,
    label: &str,
    time: f64,
) {
    export_frame(workbench, ctx, time, vec![]);
    let output = export_frame(workbench, ctx, time + 0.1, vec![]);
    let position = output
        .shapes
        .iter()
        .find_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("button '{label}' was not visible"));
    export_frame(
        workbench,
        ctx,
        time + 0.2,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
        ],
    );
    export_frame(
        workbench,
        ctx,
        time + 0.3,
        vec![egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::default(),
        }],
    );
}

#[test]
fn export_buttons_plan_test_and_publish_through_real_egui_events() {
    let tmp = tempfile::tempdir().unwrap();
    let entry = tmp.path().join("story.json");
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".into(),
            text: "Button test".into(),
        })],
        BTreeMap::from([("start".into(), 0)]),
    );
    fs::write(&entry, script.to_json().unwrap()).unwrap();
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.load_script(entry);
    workbench.open_export_wizard();
    workbench.export_wizard.export_kind = ExportWizardKind::CompiledScriptBundle;
    let output = tmp.path().join("out");
    workbench.export_wizard.output_root = output.to_string_lossy().into();
    let ctx = egui::Context::default();
    click_export_button(&mut workbench, &ctx, "Check export", 1.0);
    assert!(
        workbench.export_wizard.last_plan.is_some(),
        "{:?}",
        workbench.export_wizard.last_error
    );
    click_export_button(&mut workbench, &ctx, "Test export", 2.0);
    assert!(
        workbench.export_wizard.last_error.is_none(),
        "{:?}",
        workbench.export_wizard.last_error
    );
    assert!(!output.exists());
    workbench.export_wizard.dry_run = false;
    click_export_button(&mut workbench, &ctx, "Save and export", 3.0);
    assert!(
        workbench.export_wizard.last_report.is_some(),
        "{:?}",
        workbench.export_wizard.last_error
    );
    assert!(output.join("scripts/compiled.vnc").is_file());
}

#[test]
fn planning_standalone_export_never_creates_project_files() {
    let tmp = tempfile::tempdir().unwrap();
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".into(),
            text: "Hello".into(),
        })],
        BTreeMap::from([("start".into(), 0)]),
    );
    let entry = tmp.path().join("story.json");
    fs::write(&entry, script.to_json().unwrap()).unwrap();
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.load_script(entry);
    workbench.export_wizard.entry_script = "story.json".into();
    workbench.plan_export_wizard();
    assert!(workbench.export_wizard.last_error.is_some());
    assert!(
        !tmp.path().join("project.vnm").exists(),
        "invalid plan must not create a manifest"
    );
    workbench.export_wizard.output_root = tmp.path().join("out").to_string_lossy().into();
    workbench.export_wizard.export_kind = ExportWizardKind::CompiledScriptBundle;
    workbench.export_wizard.require_executable = false;
    workbench.execute_export_wizard();
    assert!(
        workbench.export_wizard.last_error.is_none(),
        "{:?}",
        workbench.export_wizard.last_error
    );
    assert!(
        !tmp.path().join("project.vnm").exists(),
        "dry-run must not create a manifest"
    );
    assert!(!tmp.path().join("out").exists());
}

#[test]
fn loading_another_document_discards_export_paths_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".into(),
            text: "Hello".into(),
        })],
        BTreeMap::from([("start".into(), 0)]),
    );
    let entry = tmp.path().join("new.json");
    fs::write(&entry, script.to_json().unwrap()).unwrap();
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.export_wizard.entry_script = "old.json".into();
    workbench.export_wizard.output_root = "old-project/export".into();
    workbench.export_wizard.last_error = Some("old error".into());
    workbench.load_script(entry);
    workbench.open_export_wizard();
    assert_eq!(workbench.export_wizard.entry_script, "new.json");
    assert!(std::path::Path::new(&workbench.export_wizard.output_root).starts_with(tmp.path()));
    assert!(workbench.export_wizard.last_error.is_none());
}

#[test]
fn export_uses_current_edits_and_script_bundle_ignores_old_runtime_selection() {
    let tmp = tempfile::tempdir().unwrap();
    let entry = tmp.path().join("story.json");
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Ava".into(),
            text: "Before".into(),
        })],
        BTreeMap::from([("start".into(), 0)]),
    );
    fs::write(&entry, script.to_json().unwrap()).unwrap();
    let original = fs::read(&entry).unwrap();
    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.load_script(entry.clone());
    let dialogue = workbench
        .node_graph
        .nodes()
        .find_map(|(id, node, _)| {
            matches!(node, crate::editor::StoryNode::Dialogue { .. }).then_some(id)
        })
        .unwrap();
    workbench.node_graph.edit_node(dialogue, |node| {
        if let crate::editor::StoryNode::Dialogue { text, .. } = node {
            *text = "Current edit".into();
        }
    });
    workbench.export_wizard.entry_script = "story.json".into();
    workbench.export_wizard.output_root = tmp.path().join("out").to_string_lossy().into();
    workbench.export_wizard.export_kind = ExportWizardKind::CompiledScriptBundle;
    workbench.export_wizard.runtime_artifact = "missing-old-runtime.exe".into();
    workbench.execute_export_wizard();
    assert!(workbench
        .export_wizard
        .last_error
        .as_ref()
        .unwrap()
        .contains("unsaved changes"));
    assert_eq!(fs::read(&entry).unwrap(), original);
    assert!(!tmp.path().join("out").exists());
    workbench.export_wizard.dry_run = false;
    workbench.execute_export_wizard();
    assert!(
        workbench.export_wizard.last_error.is_none(),
        "{:?}",
        workbench.export_wizard.last_error
    );
    assert!(workbench.export_wizard.last_report.is_some());
    assert!(fs::read_to_string(&entry).unwrap().contains("Current edit"));
    let bundle = tmp.path().join("out/scripts/compiled.vnscript.json");
    assert!(fs::read_to_string(bundle).unwrap().contains("Current edit"));
    assert!(!tmp.path().join("project.vnm").exists());
    assert!(workbench
        .export_wizard
        .last_report
        .as_ref()
        .unwrap()
        .executable
        .is_none());
    // A failed retry must not keep displaying the old successful report/plan.
    workbench.export_wizard.output_root.clear();
    workbench.execute_export_wizard();
    assert!(workbench.export_wizard.last_error.is_some());
    assert!(workbench.export_wizard.last_report.is_none());
    assert!(workbench.export_wizard.last_plan.is_none());
}

#[test]
fn export_wizard_blocks_executable_on_release_blocking_diagnostics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().join("project");
    fs::create_dir_all(&project_root).expect("project dir");
    visual_novel_engine::ProjectManifest::new("game", "studio")
        .save(&project_root.join("project.vnm"))
        .expect("manifest");
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "hello".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        project_root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");

    let mut workbench = EditorWorkbench::new(VnConfig::default());
    workbench.project_root = Some(project_root.clone());
    workbench.manifest = Some(visual_novel_engine::ProjectManifest::new("game", "studio"));
    workbench.export_wizard.output_root = project_root.join("dist").to_string_lossy().to_string();
    workbench.export_wizard.entry_script = "main.json".to_string();
    workbench.export_wizard.runtime_artifact.clear();
    workbench.export_wizard.require_executable = true;
    workbench.export_wizard.export_kind = ExportWizardKind::ExecutableGame;
    workbench.export_wizard.dry_run = false;

    workbench.plan_export_wizard();
    let plan = workbench.export_wizard.last_plan.as_ref().expect("plan");
    assert!(plan.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "export.runtime_artifact.missing"
            && diagnostic.blocking_release
            && diagnostic.trace_id.starts_with("export-")
    }));
    assert!(workbench.export_wizard.last_error.is_none());

    workbench.execute_export_wizard();
    let error = workbench
        .export_wizard
        .last_error
        .as_deref()
        .expect("blocking error");
    assert!(
        error.contains("Executable export blocked by diagnostics"),
        "{error}"
    );
    assert!(error.contains("export.runtime_artifact.missing"), "{error}");
    assert!(error.contains("trace_id=export-"), "{error}");
    assert!(error.contains("field=runtime_artifact"), "{error}");
    assert!(
        !project_root.join("dist").exists(),
        "blocked executable export must not publish output"
    );
}
