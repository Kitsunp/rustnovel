use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use visual_novel_engine::{
    authoring::{AuthoringDocument, AuthoringPosition, NodeGraph, StoryNode},
    build_export_plan, export_bundle,
    runtime::{
        AudioActionRaw, CharacterPlacementRaw, DialogueRaw, EventRaw, ScenePatchRaw,
        SceneTransitionRaw, SceneUpdateRaw, ScriptRaw,
    },
    BundleIntegrity, ExportBundleSpec, ExportTargetPlatform, ProjectManifest,
};

type HmacSha256 = Hmac<Sha256>;

fn create_escape_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

fn create_escape_dir_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

fn build_project_fixture() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/bgm")).expect("assets dir");

    let manifest = ProjectManifest::new("fixture", "qa");
    manifest
        .save(&root.join("project.vnm"))
        .expect("manifest save");

    let script = ScriptRaw::new(
        vec![
            EventRaw::Dialogue(DialogueRaw {
                speaker: "Narrator".to_string(),
                text: "hello".to_string(),
            }),
            EventRaw::AudioAction(AudioActionRaw {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: Some(0.8),
                fade_duration_ms: None,
                loop_playback: Some(true),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    fs::write(root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");
    fs::write(root.join("assets/bgm/unused.ogg"), [5u8, 6, 7, 8]).expect("unused asset");

    (tmp, root)
}

fn minimal_pe_exe() -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("json file")).expect("json")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn hmac_manifest_hex(key: &str, manifest_text: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(manifest_text.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn has_diagnostic_code(diagnostics: &[visual_novel_engine::ExportDiagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.code == code
            && diagnostic.phase == "plan"
            && diagnostic.trace_id.starts_with("export-")
    })
}

fn assert_no_legacy_export_warning_fields(value: &serde_json::Value) {
    assert!(
        value.get("warnings").is_none(),
        "export reports must use diagnostics, not legacy warnings: {value}"
    );
    assert!(
        value.get("errors").is_none(),
        "export reports must use diagnostics, not legacy errors: {value}"
    );
    if let Some(capabilities) = value.get("capabilities") {
        assert!(
            capabilities.get("warnings").is_none(),
            "capability report must use export diagnostics, not legacy warnings: {capabilities}"
        );
    }
}

#[test]
fn export_package_flow_reports_manifest_hashes_and_hmac_agree() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("runtime dir");
    let runtime_bytes = minimal_pe_exe();
    fs::write(runtime_dir.join("vn-runtime.exe"), &runtime_bytes).expect("runtime exe");
    let out = project_root.join("dist_flow");
    let spec = ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("runtime/vn-runtime.exe")),
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: Some("flow-secret".to_string()),
    };

    let plan = build_export_plan(&spec).expect("plan");
    assert_eq!(plan.target_platform, "windows");
    assert_eq!(plan.executable.as_deref(), Some("game.exe"));
    assert!(
        plan.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.trace_id.starts_with("export-")),
        "every plan diagnostic should carry a trace id"
    );

    let report = export_bundle(spec).expect("bundle export");
    let package_report = read_json(&out.join("meta/package_report.json"));
    let compat_report = read_json(&out.join("meta/compat_report.json"));
    let manifest_text =
        fs::read_to_string(out.join("meta/bundle_file_manifest.json")).expect("file manifest");
    let file_manifest: serde_json::Value =
        serde_json::from_str(&manifest_text).expect("file manifest json");
    let signature =
        fs::read_to_string(out.join("meta/bundle.hmac_sha256")).expect("signature file");

    assert_eq!(
        package_report,
        serde_json::to_value(&report).expect("report json")
    );
    assert_eq!(package_report["target_platform"], "windows");
    assert_eq!(package_report["runtime_artifact"], "runtime/vn-runtime.exe");
    assert_eq!(
        package_report["runtime_artifact_sha256"],
        sha256_hex(&runtime_bytes)
    );
    assert_eq!(package_report["executable"], "game.exe");
    assert_eq!(package_report["bundle_hmac_sha256"], signature);

    assert_eq!(
        compat_report["target_platform"],
        package_report["target_platform"]
    );
    assert_eq!(
        package_report["generator_os"],
        compat_report["generator_os"]
    );
    assert_eq!(
        compat_report["runtime_artifact"],
        package_report["runtime_artifact"]
    );
    assert_eq!(
        compat_report["runtime_artifact_sha256"],
        package_report["runtime_artifact_sha256"]
    );
    assert_eq!(compat_report["executable"], package_report["executable"]);
    assert_eq!(
        package_report["expected_executable"],
        compat_report["expected_executable"]
    );
    assert_eq!(compat_report["expected_executable"], "game.exe");
    assert_eq!(compat_report["graphics_backend"], "software");
    assert_eq!(
        package_report["graphics_backend"],
        compat_report["graphics_backend"]
    );
    assert_eq!(
        package_report["wgpu_fallback"],
        compat_report["wgpu_fallback"]
    );
    assert_eq!(compat_report["bundle_hmac_sha256"], signature);
    assert_eq!(
        compat_report["bundle_file_manifest_sha256"],
        sha256_hex(manifest_text.as_bytes())
    );
    assert_eq!(
        package_report["bundle_file_manifest_sha256"],
        compat_report["bundle_file_manifest_sha256"]
    );
    assert_eq!(
        signature,
        hmac_manifest_hex("flow-secret", &manifest_text),
        "signature must authenticate the exact manifest payload"
    );

    let manifest_files = file_manifest["files"].as_array().expect("manifest files");
    let compat_hashes = compat_report["hashes"].as_array().expect("compat hashes");
    let package_hashes = package_report["hashes"].as_array().expect("package hashes");
    assert_eq!(
        compat_hashes, manifest_files,
        "compat hashes must be the same manifest the HMAC signs"
    );
    assert_eq!(
        package_hashes, manifest_files,
        "package hashes must be the same manifest the HMAC signs"
    );
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "game.exe"));
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "runtime/vn-runtime.exe"));
    assert!(manifest_files.iter().any(|entry| {
        entry["path"] == package_report["runtime_artifact"]
            && entry["sha256"] == package_report["runtime_artifact_sha256"]
    }));
    assert!(manifest_files
        .iter()
        .any(|entry| entry["path"] == "assets/bgm/theme.ogg"));
    assert!(!manifest_files
        .iter()
        .any(|entry| entry["path"] == "assets/bgm/unused.ogg"));

    let mut total_size = 0u64;
    for entry in manifest_files {
        let rel = entry["path"].as_str().expect("entry path");
        let bytes = fs::read(out.join(rel)).unwrap_or_else(|err| {
            panic!("manifest entry '{rel}' should exist and be readable: {err}")
        });
        total_size += bytes.len() as u64;
        assert_eq!(entry["size"], bytes.len() as u64, "size mismatch for {rel}");
        assert_eq!(
            entry["sha256"],
            sha256_hex(&bytes),
            "hash mismatch for {rel}"
        );
    }
    assert_eq!(compat_report["total_size"], total_size);
    assert_eq!(package_report["total_size"], total_size);

    let plan_trace_ids: Vec<_> = plan
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.trace_id.as_str())
        .collect();
    let report_trace_ids: Vec<_> = report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.trace_id.as_str())
        .collect();
    assert_eq!(
        report_trace_ids, plan_trace_ids,
        "execute report must preserve plan diagnostic trace ids"
    );
    assert_eq!(compat_report["diagnostics"], package_report["diagnostics"]);
    assert_eq!(package_report["smoke_result"]["status"], "not_run");
    assert_eq!(
        package_report["smoke_result"]["checks"][0]["severity"],
        "warning"
    );
    assert_eq!(
        package_report["smoke_result"]["checks"][0]["blocking_release"],
        true
    );
    assert!(
        package_report["smoke_result"]["checks"][0]["probable_cause"]
            .as_str()
            .expect("smoke cause")
            .contains("target runtime smoke")
    );
    assert!(
        package_report["smoke_result"]["checks"][0]["suggested_action"]
            .as_str()
            .expect("smoke action")
            .contains("package smoke job")
    );
    assert_eq!(
        compat_report["smoke_result"],
        package_report["smoke_result"]
    );
    assert!(package_report["smoke_result"]["trace_id"]
        .as_str()
        .expect("smoke trace id")
        .starts_with("export-smoke-"));
}

#[test]
fn export_bundle_builds_expected_layout_and_manifest() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export");

    assert_eq!(report.target_platform, "windows");
    assert_eq!(report.integrity, "none");
    assert_eq!(report.script_source, "scripts/compiled.vnscript.json");
    assert_eq!(report.script_binary, "scripts/compiled.vnc");
    assert_eq!(report.assets_copied, 1);
    assert_eq!(
        report.capabilities.audio_actions,
        vec!["bgm:play".to_string()]
    );
    assert!(has_diagnostic_code(
        &report.diagnostics,
        "export.capability.audio_backend_required"
    ));
    assert!(!Path::new(&out.join("scripts/main.vnc")).exists());
    assert!(Path::new(&out.join("scripts/compiled.vnc")).is_file());
    assert!(Path::new(&out.join("scripts/compiled.vnscript.json")).is_file());
    assert!(!Path::new(&out.join("scripts/main.json")).exists());
    assert!(Path::new(&out.join("assets/bgm/theme.ogg")).is_file());
    assert!(!Path::new(&out.join("assets/bgm/unused.ogg")).exists());
    assert!(Path::new(&out.join("meta/assets_manifest.json")).is_file());
    assert!(Path::new(&out.join("meta/package_report.json")).is_file());
    assert!(Path::new(&out.join("launch.bat")).is_file());

    let manifest_raw =
        fs::read_to_string(out.join("meta/assets_manifest.json")).expect("assets manifest");
    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_raw).expect("assets manifest json");
    assert_eq!(
        manifest.get("manifest_version").and_then(|v| v.as_u64()),
        Some(1)
    );
    let assets = manifest
        .get("assets")
        .and_then(|value| value.as_object())
        .expect("assets map");
    assert!(assets.get("assets/bgm/theme.ogg").is_some());
    assert!(assets.get("assets/bgm/unused.ogg").is_none());

    assert_eq!(
        report.compat_report.as_deref(),
        Some("meta/compat_report.json")
    );
    let compat_raw =
        fs::read_to_string(out.join("meta/compat_report.json")).expect("compat report");
    let compat: serde_json::Value = serde_json::from_str(&compat_raw).expect("compat json");
    let package_report = read_json(&out.join("meta/package_report.json"));
    assert_no_legacy_export_warning_fields(&package_report);
    assert_no_legacy_export_warning_fields(&compat);
    assert_eq!(compat["schema"], "vnengine.export_compat_report.v1");
    assert_eq!(compat["target_platform"], "windows");
    assert_eq!(compat["expected_executable"], "game.exe");
    assert_eq!(
        package_report["expected_executable"],
        compat["expected_executable"]
    );
    assert_eq!(compat["graphics_backend"], "software");
    assert_eq!(
        package_report["graphics_backend"],
        compat["graphics_backend"]
    );
    assert_eq!(compat["wgpu_fallback"], true);
    assert_eq!(package_report["wgpu_fallback"], compat["wgpu_fallback"]);
    assert_eq!(compat["assets_copied"], 1);
    assert_eq!(package_report["total_size"], compat["total_size"]);
    assert!(compat["total_size"].as_u64().expect("total size") > 0);
    assert_eq!(compat["smoke_result"]["status"], "not_run");
    assert_eq!(package_report["smoke_result"], compat["smoke_result"]);
    let diagnostics = compat["diagnostics"].as_array().expect("diagnostics");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "export.runtime_artifact.missing"
            && diagnostic["phase"] == "plan"
            && diagnostic["target"] == "windows"
            && diagnostic["trace_id"]
                .as_str()
                .is_some_and(|trace_id| trace_id.starts_with("export-"))
            && diagnostic["blocking_release"] == true
    }));
    let hashes = compat["hashes"].as_array().expect("hashes");
    assert_eq!(package_report["hashes"], compat["hashes"]);
    assert!(hashes
        .iter()
        .any(|entry| entry["path"] == "scripts/compiled.vnc"));
}

#[test]
fn export_bundle_manifest_matches_runtime_asset_path_for_root_relative_assets() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("bg")).expect("bg dir");
    ProjectManifest::new("root-relative-asset-fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest save");
    let script = ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: Vec::new(),
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(root.join("main.json"), script.to_json().expect("script")).expect("script");
    fs::write(root.join("bg/room.png"), [1u8, 2, 3, 4]).expect("asset");

    let out = root.join("dist_root_relative_asset");
    export_bundle(ExportBundleSpec {
        project_root: root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export");

    assert!(out.join("assets/bg/room.png").is_file());
    let manifest = read_json(&out.join("meta/assets_manifest.json"));
    let assets = manifest["assets"].as_object().expect("assets manifest");
    assert!(
        assets.contains_key("assets/bg/room.png"),
        "manifest must use the runtime-visible bundle path: {assets:?}"
    );
    assert!(
        !assets.contains_key("bg/room.png"),
        "manifest must not keep the source-only project path"
    );
}

#[test]
fn export_bundle_rejects_case_sensitive_asset_collisions() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets")).expect("assets dir");
    ProjectManifest::new("case-fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest save");
    fs::write(root.join("assets/Ava.png"), [1u8, 2, 3]).expect("upper asset");
    fs::write(root.join("assets/ava.png"), [4u8, 5, 6]).expect("lower asset");
    let script = ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("assets/Ava.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("assets/ava.png".to_string()),
                position: None,
                x: None,
                y: None,
                scale: None,
            }],
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");

    let out = root.join("dist_case_collision");
    let err = export_bundle(ExportBundleSpec {
        project_root: root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("case collision must fail");

    let message = err.to_string();
    assert!(
        message.contains("case-sensitive asset collision"),
        "{message}"
    );
    assert!(
        !out.exists(),
        "failed export must not publish a partial final bundle"
    );
}

#[test]
fn export_plan_and_bundle_reject_exact_asset_destination_collisions() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/bg")).expect("assets dir");
    fs::create_dir_all(root.join("bg")).expect("root bg dir");
    ProjectManifest::new("destination-collision-fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest save");
    fs::write(root.join("bg/room.png"), [1u8, 2, 3]).expect("root-relative asset");
    fs::write(root.join("assets/bg/room.png"), [4u8, 5, 6]).expect("assets asset");
    let script = ScriptRaw::new(
        vec![EventRaw::Scene(SceneUpdateRaw {
            background: Some("bg/room.png".to_string()),
            music: None,
            characters: vec![CharacterPlacementRaw {
                name: "Ava".to_string(),
                expression: Some("assets/bg/room.png".to_string()),
                position: None,
                x: None,
                y: None,
                scale: None,
            }],
        })],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");
    let spec = ExportBundleSpec {
        project_root: root.clone(),
        output_root: root.join("dist_destination_collision"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };

    let plan = build_export_plan(&spec).expect("plan");
    assert!(
        has_diagnostic_code(&plan.diagnostics, "export.asset.destination_collision"),
        "plan must surface exact destination collisions: {:?}",
        plan.diagnostics
    );

    let err = export_bundle(spec).expect_err("destination collision must fail");
    let message = err.to_string();
    assert!(message.contains("asset destination collision"), "{message}");
}

#[test]
fn export_plan_cli_py_gui_parity() {
    let (_tmp, project_root) = build_project_fixture();
    let spec = ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };

    let cli_plan = build_export_plan(&spec).expect("cli plan");
    let py_plan_json = serde_json::to_string(&cli_plan).expect("plan json");
    let py_plan: visual_novel_engine::ExportPlan =
        serde_json::from_str(&py_plan_json).expect("py/gui plan");

    assert_eq!(cli_plan.script_sha256, py_plan.script_sha256);
    assert_eq!(cli_plan.layout, py_plan.layout);
    assert_eq!(cli_plan.capabilities, py_plan.capabilities);
    let plan_value = serde_json::to_value(&cli_plan).expect("plan value");
    assert_no_legacy_export_warning_fields(&plan_value);
    let mut legacy_plan_value = plan_value.clone();
    legacy_plan_value["warnings"] = serde_json::json!(["legacy warning"]);
    legacy_plan_value["errors"] = serde_json::json!(["legacy error"]);
    legacy_plan_value["capabilities"]["warnings"] = serde_json::json!(["legacy capability"]);
    let legacy_plan: visual_novel_engine::ExportPlan =
        serde_json::from_value(legacy_plan_value).expect("legacy plan compatibility");
    assert!(
        legacy_plan.warnings.is_empty(),
        "legacy flat warnings must not be accepted into the runtime model"
    );
    assert!(
        legacy_plan.errors.is_empty(),
        "legacy flat errors must not be accepted into the runtime model"
    );
    assert!(
        legacy_plan.capabilities.warnings.is_empty(),
        "legacy capability warnings must not be accepted into the runtime model"
    );
    assert!(has_diagnostic_code(
        &cli_plan.diagnostics,
        "export.runtime_artifact.missing"
    ));
}

#[test]
fn export_plan_extcall_audio_transition_missing_runtime() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().join("project");
    fs::create_dir_all(root.join("assets/bgm")).expect("assets dir");
    ProjectManifest::new("fixture", "qa")
        .save(&root.join("project.vnm"))
        .expect("manifest");
    let script = ScriptRaw::new(
        vec![
            EventRaw::AudioAction(AudioActionRaw {
                channel: "bgm".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: Some(0.5),
                fade_duration_ms: Some(300),
                loop_playback: Some(true),
            }),
            EventRaw::Transition(SceneTransitionRaw {
                kind: "fade".to_string(),
                duration_ms: 250,
                color: None,
            }),
            EventRaw::ExtCall {
                command: "plugin".to_string(),
                args: vec![],
            },
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(root.join("main.json"), script.to_json().expect("script")).expect("script");
    fs::write(root.join("assets/bgm/theme.ogg"), [1u8, 2, 3, 4]).expect("asset");

    let plan = build_export_plan(&ExportBundleSpec {
        project_root: root.clone(),
        output_root: root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("plan");

    let plan_value = serde_json::to_value(&plan).expect("plan value");
    assert_no_legacy_export_warning_fields(&plan_value);
    assert!(plan
        .capabilities
        .ext_call_commands
        .contains(&"plugin".to_string()));
    assert!(!plan.capabilities.audio_actions.is_empty());
    assert!(!plan.capabilities.transitions.is_empty());
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.capability.ext_call_runtime_required"
    ));
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.capability.audio_backend_required"
    ));
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.capability.transition_support_required"
    ));
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.runtime_artifact.missing"
    ));
}

#[test]
fn export_plan_capability_policy_contract() {
    let (_tmp, project_root) = build_project_fixture();
    let plan = build_export_plan(&ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("missing-runtime.exe")),
        integrity: BundleIntegrity::HmacSha256,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("plan with policy errors");

    let plan_value = serde_json::to_value(&plan).expect("plan value");
    assert_no_legacy_export_warning_fields(&plan_value);
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.runtime_artifact.unreadable"
    ));
    assert!(has_diagnostic_code(
        &plan.diagnostics,
        "export.integrity.hmac_key_missing"
    ));
}

#[test]
fn export_bundle_reports_extcall_audio_transition_capabilities() {
    let (_tmp, project_root) = build_project_fixture();
    let script = ScriptRaw::new(
        vec![
            EventRaw::Scene(SceneUpdateRaw {
                background: None,
                music: Some("assets/bgm/theme.ogg".to_string()),
                characters: Vec::new(),
            }),
            EventRaw::AudioAction(AudioActionRaw {
                channel: "sfx".to_string(),
                action: "play".to_string(),
                asset: Some("assets/bgm/theme.ogg".to_string()),
                volume: None,
                fade_duration_ms: None,
                loop_playback: None,
            }),
            EventRaw::Transition(SceneTransitionRaw {
                kind: "dissolve".to_string(),
                duration_ms: 250,
                color: None,
            }),
            EventRaw::ExtCall {
                command: "plugin.unlock".to_string(),
                args: Vec::new(),
            },
            EventRaw::Patch(ScenePatchRaw {
                background: None,
                music: Some("assets/bgm/theme.ogg".to_string()),
                add: Vec::new(),
                update: Vec::new(),
                remove: Vec::new(),
            }),
        ],
        BTreeMap::from([("start".to_string(), 0)]),
    );
    fs::write(
        project_root.join("main.json"),
        script.to_json().expect("script json"),
    )
    .expect("script");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist_caps"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export");

    assert_eq!(
        report.capabilities.ext_call_commands,
        vec!["plugin.unlock".to_string()]
    );
    assert_eq!(
        report.capabilities.audio_actions,
        vec![
            "bgm:scene_music".to_string(),
            "bgm:scene_patch_music".to_string(),
            "sfx:play".to_string(),
        ]
    );
    assert_eq!(
        report.capabilities.transitions,
        vec!["dissolve".to_string()]
    );
    assert!(has_diagnostic_code(
        &report.diagnostics,
        "export.capability.ext_call_runtime_required"
    ));
    assert!(has_diagnostic_code(
        &report.diagnostics,
        "export.capability.transition_support_required"
    ));
}

#[test]
fn export_bundle_accepts_authoring_document_entry() {
    let (_tmp, project_root) = build_project_fixture();
    let mut manifest = ProjectManifest::new("fixture", "qa");
    manifest.settings.entry_point = "main.vnauthoring".to_string();
    manifest
        .save(&project_root.join("project.vnm"))
        .expect("manifest save");

    let mut graph = NodeGraph::new();
    let start = graph.add_node(StoryNode::Start, AuthoringPosition::new(0.0, 0.0));
    let line = graph.add_node(
        StoryNode::Dialogue {
            speaker: "Narrator".to_string(),
            text: "from authoring".to_string(),
        },
        AuthoringPosition::new(0.0, 90.0),
    );
    let end = graph.add_node(StoryNode::End, AuthoringPosition::new(0.0, 180.0));
    graph.connect(start, line);
    graph.connect(line, end);
    let document = AuthoringDocument::new(graph);
    fs::write(
        project_root.join("main.vnauthoring"),
        document.to_json().expect("authoring json"),
    )
    .expect("write authoring");

    let out = project_root.join("dist_authoring");
    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export from authoring");

    assert_eq!(report.script_source, "scripts/compiled.vnscript.json");
    assert!(Path::new(&out.join("scripts/compiled.vnc")).is_file());
    assert!(!Path::new(&out.join("scripts/main.vnauthoring")).exists());
}

#[test]
fn export_bundle_windows_runtime_exe_creates_top_level_executable() {
    let (_tmp, project_root) = build_project_fixture();
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("mkdir runtime");
    let runtime_bytes = minimal_pe_exe();
    fs::write(runtime_dir.join("vn-runtime.exe"), &runtime_bytes).expect("write runtime");
    let out = project_root.join("dist_exe");

    let report = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("runtime/vn-runtime.exe")),
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect("bundle export with runtime exe");

    assert_eq!(
        report.runtime_artifact.as_deref(),
        Some("runtime/vn-runtime.exe")
    );
    assert_eq!(report.executable.as_deref(), Some("game.exe"));
    assert_eq!(
        fs::read(out.join("game.exe")).expect("game exe"),
        runtime_bytes
    );
    let launcher = fs::read_to_string(out.join("launch.bat")).expect("launcher");
    assert!(
        launcher.contains("\"%~dp0game.exe\""),
        "launcher should execute the top-level exe: {launcher}"
    );
}

#[test]
fn export_bundle_rejects_entry_script_traversal() {
    let (_tmp, project_root) = build_project_fixture();
    let out = project_root.join("dist");

    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: Some("../outside.json".into()),
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("entry traversal must fail");

    let message = format!("{err}");
    assert!(message.contains("path traversal"));
}

#[test]
fn export_bundle_rejects_entry_script_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape.json");
    fs::write(
        &escaped,
        r#"{
  "script_schema_version": "1.0",
  "events": [],
  "labels": {}
}"#,
    )
    .expect("write escaped script");

    let entry_script_path = project_root.join("main.json");
    if !create_escape_symlink(&entry_script_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("entry symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}

#[test]
fn export_plan_and_bundle_reject_project_manifest_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped_manifest = tmp.path().join("outside_project.vnm");
    fs::write(
        &escaped_manifest,
        r#"
manifest_schema_version = 1

[project]
name = "outside"

[settings]
entry_point = "main.json"
default_language = "en"
"#,
    )
    .expect("write escaped manifest");
    let manifest_path = project_root.join("project.vnm");
    fs::remove_file(&manifest_path).expect("remove normal manifest");
    if !create_escape_symlink(&manifest_path, &escaped_manifest) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let spec = ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: project_root.join("dist"),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };

    let plan_err = build_export_plan(&spec).expect_err("plan must reject manifest symlink escape");
    assert!(format!("{plan_err}").contains("regular file"));

    let export_err =
        export_bundle(spec).expect_err("bundle export must reject manifest symlink escape");
    assert!(format!("{export_err}").contains("regular file"));
}

#[test]
fn export_plan_and_bundle_reject_output_root_directory_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let outside_output = tmp.path().join("outside_dist");
    fs::create_dir_all(&outside_output).expect("outside output");
    fs::write(outside_output.join("sentinel.txt"), b"keep").expect("sentinel");
    let output_link = project_root.join("dist_link");
    if !create_escape_dir_symlink(&output_link, &outside_output) {
        eprintln!("directory symlink creation not supported on this platform");
        return;
    }

    let spec = ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: output_link.clone(),
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    };

    let plan_err = build_export_plan(&spec).expect_err("plan must reject output symlink");
    assert!(format!("{plan_err}").contains("output_root is not a directory"));

    let export_err = export_bundle(spec).expect_err("bundle export must reject output symlink");
    assert!(format!("{export_err}").contains("output_root is not a directory"));
    assert_eq!(
        fs::read(outside_output.join("sentinel.txt")).expect("sentinel untouched"),
        b"keep",
        "export must not publish into the symlink target"
    );
    assert!(
        !outside_output.join("scripts").exists(),
        "export must not materialize bundle files in the symlink target"
    );
    assert!(
        fs::symlink_metadata(&output_link)
            .expect("output link")
            .file_type()
            .is_symlink(),
        "rejected output link should remain a symlink"
    );
}

#[test]
fn export_bundle_rejects_asset_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape.ogg");
    fs::write(&escaped, [9u8, 9, 9]).expect("write escaped asset");
    let symlink_path = project_root.join("assets").join("bgm").join("theme.ogg");
    fs::remove_file(&symlink_path).expect("remove normal referenced asset");
    if !create_escape_symlink(&symlink_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: None,
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("asset symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}

#[test]
fn export_bundle_rejects_runtime_artifact_symlink_escape() {
    let (tmp, project_root) = build_project_fixture();
    let escaped = tmp.path().join("escape-runtime.bin");
    fs::write(&escaped, b"runtime").expect("write escaped runtime");
    let runtime_dir = project_root.join("runtime");
    fs::create_dir_all(&runtime_dir).expect("mkdir runtime");
    let runtime_path = runtime_dir.join("engine.bin");
    if !create_escape_symlink(&runtime_path, &escaped) {
        eprintln!("symlink creation not supported on this platform");
        return;
    }

    let out = project_root.join("dist");
    let err = export_bundle(ExportBundleSpec {
        project_root: project_root.clone(),
        output_root: out,
        target_platform: ExportTargetPlatform::Windows,
        entry_script: None,
        runtime_artifact: Some(PathBuf::from("runtime/engine.bin")),
        integrity: BundleIntegrity::None,
        output_layout_version: 1,
        hmac_key: None,
    })
    .expect_err("runtime artifact symlink escape must fail");

    assert!(format!("{err}").contains("escapes project root"));
}
