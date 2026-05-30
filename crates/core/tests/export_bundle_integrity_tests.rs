use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
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
    let package_report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.join("meta/package_report.json")).expect("package report"),
    )
    .expect("package report json");
    assert_eq!(package_report["bundle_hmac_sha256"], signature);
    assert_eq!(
        package_report["bundle_file_manifest"],
        "meta/bundle_file_manifest.json"
    );
    let file_manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.join("meta/bundle_file_manifest.json"))
            .expect("bundle file manifest"),
    )
    .expect("manifest json");
    assert_eq!(
        file_manifest["integrity_scope"],
        "bundle_file_manifest_v2_signed_manifest_covers_payload_files"
    );
    assert_eq!(file_manifest["manifest_version"], 2);
    let manifest_files = file_manifest["files"].as_array().expect("files array");
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "scripts/compiled.vnc"));
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "launch.sh"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "meta/package_report.json"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "meta/compat_report.json"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "meta/bundle_file_manifest.json"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "meta/bundle.hmac_sha256"));
    assert_eq!(
        report.integrity_scope,
        "bundle_file_manifest_v2_signed_manifest_covers_payload_files"
    );
    let manifest_text =
        fs::read_to_string(out.join("meta/bundle_file_manifest.json")).expect("manifest text");
    let mut mac = Hmac::<Sha256>::new_from_slice(b"top-secret").expect("hmac key");
    mac.update(manifest_text.as_bytes());
    let expected = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(signature, expected);
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

#[test]
fn export_bundle_hmac_changes_when_manifest_is_tampered() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist_tamper");
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
    let signature = report.bundle_hmac_sha256.expect("signature in report");
    let manifest_text =
        fs::read_to_string(out.join("meta/bundle_file_manifest.json")).expect("manifest text");
    let tampered_manifest = manifest_text.replace("scripts/compiled.vnc", "scripts/tampered.vnc");

    let mut mac = Hmac::<Sha256>::new_from_slice(b"top-secret").expect("hmac key");
    mac.update(tampered_manifest.as_bytes());
    let tampered_signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_ne!(signature, tampered_signature);

    let compat: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.join("meta/compat_report.json")).expect("compat report"),
    )
    .expect("compat json");
    let package_report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(out.join("meta/package_report.json")).expect("package report"),
    )
    .expect("package report json");
    let manifest_hash = Sha256::digest(manifest_text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(compat["bundle_file_manifest_sha256"], manifest_hash);
    assert_eq!(package_report["bundle_file_manifest_sha256"], manifest_hash);
    assert_eq!(compat["bundle_hmac_sha256"], signature);
    assert_eq!(package_report["bundle_hmac_sha256"], signature);
}
