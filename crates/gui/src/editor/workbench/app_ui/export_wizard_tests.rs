use std::collections::BTreeMap;
use std::fs;

use super::*;
use crate::VnConfig;
use visual_novel_engine::runtime::{DialogueRaw, EventRaw, ScriptRaw};

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
