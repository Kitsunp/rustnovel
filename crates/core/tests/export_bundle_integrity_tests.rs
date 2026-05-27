use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use tempfile::TempDir;
use visual_novel_engine::{
    export_bundle,
    runtime::{DialogueRaw, EventRaw, ScriptRaw},
    BundleIntegrity, ExportBundleSpec, ExportTargetPlatform, ProjectManifest,
};

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets")).expect("assets dir");
    ProjectManifest::new("fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest save");
    let script = ScriptRaw::new(
        vec![EventRaw::Dialogue(DialogueRaw {
            speaker: "Narrator".to_string(),
            text: "hello".to_string(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    (tmp, root)
}

#[test]
fn export_bundle_hmac_integrity_writes_signature() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Linux,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: Some("top-secret".to_string()),
    })
    .expect("bundle export with hmac");

    assert_eq!(report.integrity, "hmac_sha256");
    let signature = report.bundle_hmac_sha256.expect("signature in report");
    assert!(!signature.is_empty());

    let signature_file =
        fs::read_to_string(out.join("meta/bundle.hmac_sha256")).expect("signature file");
    assert_eq!(signature, signature_file);
    assert!(Path::new(&out.join("launch.sh")).is_file());
}

#[test]
fn export_bundle_hmac_requires_key() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("missing key should fail");

    assert!(format!("{err}").contains("requires hmac_key"));
}
