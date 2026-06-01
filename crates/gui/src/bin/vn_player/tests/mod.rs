use super::*;

fn create_file_symlink(link: &Path, target: &Path) -> bool {
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

#[test]
fn standalone_launch_defaults_to_bundle_files_next_to_executable(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("scripts"))?;
    std::fs::create_dir_all(dir.path().join("meta"))?;
    std::fs::write(
        dir.path().join("scripts").join("compiled.vnscript.json"),
        "{}",
    )?;
    std::fs::write(dir.path().join("meta").join("assets_manifest.json"), "{}")?;

    let paths = resolve_launch_paths_for_exe_dir(None, None, None, false, Some(dir.path()));

    assert_eq!(
        paths.script_path,
        dir.path().join("scripts").join("compiled.vnscript.json")
    );
    assert_eq!(paths.bundle_root, Some(dir.path().to_path_buf()));
    assert_eq!(paths.assets_root, dir.path());
    assert_eq!(
        paths.manifest_path,
        Some(dir.path().join("meta").join("assets_manifest.json"))
    );
    assert!(
        paths.require_manifest,
        "direct executable launch should enforce the bundled asset manifest"
    );
    Ok(())
}

#[test]
fn explicit_script_keeps_cli_paths_even_when_executable_has_bundle_files(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("scripts"))?;
    std::fs::write(
        dir.path().join("scripts").join("compiled.vnscript.json"),
        "{}",
    )?;
    let script_path = PathBuf::from("custom").join("story.json");

    let paths = resolve_launch_paths_for_exe_dir(
        Some(script_path.clone()),
        None,
        None,
        false,
        Some(dir.path()),
    );

    assert_eq!(paths.script_path, script_path);
    assert_eq!(paths.bundle_root, None);
    assert_eq!(paths.assets_root, PathBuf::from("custom"));
    assert!(!paths.require_manifest);
    assert_eq!(paths.manifest_path, None);
    Ok(())
}

#[test]
fn packaged_project_manifest_parse_errors_are_not_ignored() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("meta"))?;
    std::fs::write(dir.path().join("meta").join("project.vnm"), "not = [toml")?;

    let err = load_packaged_project_manifest(dir.path())
        .expect_err("existing packaged project manifest must not be silently ignored");
    assert!(err.to_string().contains("project.vnm"));
    Ok(())
}

#[test]
fn packaged_project_manifest_symlink_is_rejected_instead_of_followed(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    std::fs::create_dir_all(dir.path().join("meta"))?;
    let outside_manifest = outside.path().join("project.vnm");
    std::fs::write(&outside_manifest, "[project]\nname = \"outside\"\n")?;
    let link = dir.path().join("meta").join("project.vnm");
    if !create_file_symlink(&link, &outside_manifest) {
        eprintln!("file symlink creation not supported on this platform");
        return Ok(());
    }

    let err = load_packaged_project_manifest(dir.path())
        .expect_err("packaged project manifest symlink must not be followed");

    assert!(err.to_string().contains("regular file"), "{err}");
    Ok(())
}

#[test]
fn smoke_mode_rejects_package_report_symlink_without_touching_target(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("scripts"))?;
    std::fs::create_dir_all(bundle.join("meta"))?;
    std::fs::write(bundle.join("scripts/compiled.vnscript.json"), "{}")?;
    std::fs::write(bundle.join("meta/assets_manifest.json"), "{}")?;

    let outside_report = outside.path().join("package_report.json");
    let outside_json = serde_json::json!({
        "target_platform": "windows",
        "smoke_result": {
            "status": "not_run",
            "backend": "software",
            "details": "outside"
        }
    })
    .to_string();
    std::fs::write(&outside_report, &outside_json)?;
    let link = bundle.join("meta/package_report.json");
    if !create_file_symlink(&link, &outside_report) {
        eprintln!("file symlink creation not supported on this platform");
        return Ok(());
    }

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let err = run_from_args(vec![
        bundle
            .join("scripts/compiled.vnscript.json")
            .display()
            .to_string(),
        "--assets-root".to_string(),
        bundle.display().to_string(),
        "--manifest".to_string(),
        bundle
            .join("meta/assets_manifest.json")
            .display()
            .to_string(),
        "--require-manifest".to_string(),
        "--smoke".to_string(),
        "--smoke-report".to_string(),
        smoke_report.display().to_string(),
    ])
    .expect_err("package_report symlink must fail instead of reading outside bundle");

    assert!(err.to_string().contains("regular file"), "{err}");
    assert_eq!(
        std::fs::read_to_string(&outside_report)?,
        outside_json,
        "runtime smoke must not retropropagate results through the symlink"
    );
    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["status"], "failed");
    let report_check = smoke["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.report_read")
        .expect("report read failure check");
    assert_eq!(report_check["field"], "package_report");
    assert!(report_check["message"]
        .as_str()
        .unwrap()
        .contains("regular file"));
    Ok(())
}

#[test]
fn smoke_mode_loads_packaged_flow_and_updates_reports() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("scripts"))?;
    std::fs::create_dir_all(bundle.join("meta"))?;
    std::fs::create_dir_all(bundle.join("assets/backgrounds"))?;

    let asset = b"smoke-asset";
    let asset_path = bundle.join("assets/backgrounds/smoke.png");
    std::fs::write(&asset_path, asset)?;
    let asset_hash = sha256_hex(asset);
    std::fs::write(
        bundle.join("meta/assets_manifest.json"),
        serde_json::json!({
            "manifest_version": 1,
            "assets": {
                "assets/backgrounds/smoke.png": {
                    "sha256": asset_hash,
                    "size": asset.len()
                }
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("scripts/compiled.vnscript.json"),
        serde_json::json!({
            "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
            "events": [
                {
                    "type": "scene",
                    "background": "assets/backgrounds/smoke.png",
                    "characters": []
                },
                {
                    "type": "dialogue",
                    "speaker": "Narrator",
                    "text": "Smoke reached dialogue"
                }
            ],
            "labels": {
                "start": 0
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/package_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_bundle_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/compat_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_compat_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(bundle.join("meta/bundle_file_manifest.json"), "{}")?;
    std::fs::write(bundle.join("meta/bundle.hmac_sha256"), "abc123")?;

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    run_from_args(vec![
        bundle
            .join("scripts/compiled.vnscript.json")
            .display()
            .to_string(),
        "--assets-root".to_string(),
        bundle.display().to_string(),
        "--manifest".to_string(),
        bundle
            .join("meta/assets_manifest.json")
            .display()
            .to_string(),
        "--require-manifest".to_string(),
        "--smoke".to_string(),
        "--smoke-report".to_string(),
        smoke_report.display().to_string(),
    ])?;

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["schema"], "vnengine.player_runtime_smoke.v1");
    assert_eq!(smoke["status"], "passed");
    assert_eq!(smoke["target_platform"], "windows");
    let asset_check = smoke["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.asset_load")
        .expect("asset load smoke check");
    assert_eq!(asset_check["severity"], "info");
    assert_eq!(asset_check["blocking_release"], false);
    assert_eq!(asset_check["asset"], "assets/backgrounds/smoke.png");
    assert!(asset_check["trace_id"]
        .as_str()
        .unwrap()
        .starts_with("export-smoke-"));
    assert!(asset_check["probable_cause"]
        .as_str()
        .unwrap()
        .contains("asset"));
    assert!(asset_check["suggested_action"]
        .as_str()
        .unwrap()
        .contains("asset"));
    assert!(asset_check["consequence"]
        .as_str()
        .unwrap()
        .contains("frame"));

    let package: Value = serde_json::from_str(&std::fs::read_to_string(
        bundle.join("meta/package_report.json"),
    )?)?;
    let compat: Value = serde_json::from_str(&std::fs::read_to_string(
        bundle.join("meta/compat_report.json"),
    )?)?;
    assert_eq!(package["smoke_result"], smoke["smoke_result"]);
    assert_eq!(compat["smoke_result"], smoke["smoke_result"]);
    assert_eq!(package["smoke_result"]["status"], "passed");
    Ok(())
}

#[test]
fn failed_smoke_artifacts_record_metadata_read_errors() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("meta"))?;
    std::fs::write(bundle.join("meta/bundle.hmac_sha256"), [0xff, 0xfe])?;
    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let artifacts = SmokeArtifactContext {
        target_platform: "windows".to_string(),
        bundle_root: Some(bundle.to_path_buf()),
        script_path: bundle.join("scripts/compiled.vnscript.json"),
        assets_root: bundle.to_path_buf(),
        manifest_path: None,
        package_report_path: None,
        compat_report_path: None,
        smoke_report_path: Some(smoke_report.clone()),
    };
    let primary_check = smoke_check(
        "export.runtime_smoke.asset_load",
        "failed",
        "windows",
        "primary asset failure",
    );
    let smoke_result = ExportRuntimeSmokeResult {
        status: "failed".to_string(),
        backend: "software".to_string(),
        details: "primary asset failure".to_string(),
        phase: "smoke".to_string(),
        target: "windows".to_string(),
        trace_id: primary_check.trace_id.clone(),
        checks: vec![primary_check],
    };

    write_smoke_artifacts(&artifacts, &smoke_result, false)?;

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["smoke_result"]["details"], "primary asset failure");
    assert_eq!(smoke["bundle_hmac_sha256"], Value::Null);
    let metadata_check = smoke["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.metadata_read")
        .expect("metadata read check should be recorded");
    assert_eq!(metadata_check["field"], "bundle_hmac_sha256");
    assert_eq!(metadata_check["severity"], "error");
    assert_eq!(metadata_check["blocking_release"], true);
    assert!(metadata_check["message"]
        .as_str()
        .unwrap()
        .contains("bundle.hmac_sha256"));
    assert!(smoke["smoke_result"]["checks"]
        .as_array()
        .expect("smoke result checks")
        .iter()
        .any(|check| check["code"] == "export.runtime_smoke.metadata_read"));
    Ok(())
}

#[test]
fn smoke_artifacts_record_metadata_symlink_without_hashing_target(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("meta"))?;
    let outside_manifest = outside.path().join("bundle_file_manifest.json");
    std::fs::write(&outside_manifest, b"outside metadata")?;
    let link = bundle.join("meta/bundle_file_manifest.json");
    if !create_file_symlink(&link, &outside_manifest) {
        eprintln!("file symlink creation not supported on this platform");
        return Ok(());
    }

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let artifacts = SmokeArtifactContext {
        target_platform: "windows".to_string(),
        bundle_root: Some(bundle.to_path_buf()),
        script_path: bundle.join("scripts/compiled.vnscript.json"),
        assets_root: bundle.to_path_buf(),
        manifest_path: None,
        package_report_path: None,
        compat_report_path: None,
        smoke_report_path: Some(smoke_report.clone()),
    };
    let primary_check = smoke_check(
        "export.runtime_smoke.asset_load",
        "failed",
        "windows",
        "primary asset failure",
    );
    let smoke_result = ExportRuntimeSmokeResult {
        status: "failed".to_string(),
        backend: "software".to_string(),
        details: "primary asset failure".to_string(),
        phase: "smoke".to_string(),
        target: "windows".to_string(),
        trace_id: primary_check.trace_id.clone(),
        checks: vec![primary_check],
    };

    write_smoke_artifacts(&artifacts, &smoke_result, false)?;

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["bundle_file_manifest_sha256"], Value::Null);
    let metadata_check = smoke["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| {
            check["code"] == "export.runtime_smoke.metadata_read"
                && check["field"] == "bundle_file_manifest_sha256"
        })
        .expect("metadata symlink read check");
    assert!(metadata_check["message"]
        .as_str()
        .unwrap()
        .contains("regular file"));
    Ok(())
}

#[test]
fn smoke_mode_rejects_missing_asset_after_advance() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("scripts"))?;
    std::fs::create_dir_all(bundle.join("meta"))?;
    std::fs::create_dir_all(bundle.join("assets/backgrounds"))?;

    std::fs::write(
        bundle.join("meta/assets_manifest.json"),
        serde_json::json!({
            "manifest_version": 1,
            "assets": {}
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("scripts/compiled.vnscript.json"),
        serde_json::json!({
            "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
            "events": [
                {
                    "type": "dialogue",
                    "speaker": "Narrator",
                    "text": "Initial frame has no image assets"
                },
                {
                    "type": "scene",
                    "background": "assets/backgrounds/missing_after_advance.png",
                    "characters": []
                }
            ],
            "labels": {
                "start": 0
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/package_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_bundle_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/compat_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_compat_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(bundle.join("meta/bundle_file_manifest.json"), "{}")?;
    std::fs::write(bundle.join("meta/bundle.hmac_sha256"), "abc123")?;

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let err = run_from_args(vec![
        bundle
            .join("scripts/compiled.vnscript.json")
            .display()
            .to_string(),
        "--assets-root".to_string(),
        bundle.display().to_string(),
        "--manifest".to_string(),
        bundle
            .join("meta/assets_manifest.json")
            .display()
            .to_string(),
        "--require-manifest".to_string(),
        "--smoke".to_string(),
        "--smoke-report".to_string(),
        smoke_report.display().to_string(),
    ])
    .expect_err("missing asset introduced after advance should fail runtime smoke");
    let err_text = err.to_string();
    assert!(err_text.contains("export.runtime_smoke.asset_load"));
    assert!(err_text.contains("missing_after_advance.png"));

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["schema"], "vnengine.player_runtime_smoke.v1");
    assert_eq!(smoke["status"], "failed");
    assert_eq!(smoke["smoke_result"]["status"], "failed");
    let failed_check = smoke["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.asset_load")
        .expect("asset load failure after advance");
    assert_eq!(
        failed_check["asset"],
        "assets/backgrounds/missing_after_advance.png"
    );
    Ok(())
}

#[test]
fn smoke_mode_persists_failed_result_and_updates_reports() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("scripts"))?;
    std::fs::create_dir_all(bundle.join("meta"))?;
    std::fs::create_dir_all(bundle.join("assets/backgrounds"))?;

    std::fs::write(
        bundle.join("meta/assets_manifest.json"),
        serde_json::json!({
            "manifest_version": 1,
            "assets": {}
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("scripts/compiled.vnscript.json"),
        serde_json::json!({
            "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
            "events": [
                {
                    "type": "scene",
                    "background": "assets/backgrounds/missing.png",
                    "characters": []
                },
                {
                    "type": "dialogue",
                    "speaker": "Narrator",
                    "text": "Smoke should fail before this is released"
                }
            ],
            "labels": {
                "start": 0
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/package_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_bundle_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/compat_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_compat_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(bundle.join("meta/bundle_file_manifest.json"), "{}")?;
    std::fs::write(bundle.join("meta/bundle.hmac_sha256"), "abc123")?;

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let err = run_from_args(vec![
        bundle
            .join("scripts/compiled.vnscript.json")
            .display()
            .to_string(),
        "--assets-root".to_string(),
        bundle.display().to_string(),
        "--manifest".to_string(),
        bundle
            .join("meta/assets_manifest.json")
            .display()
            .to_string(),
        "--require-manifest".to_string(),
        "--smoke".to_string(),
        "--smoke-report".to_string(),
        smoke_report.display().to_string(),
    ])
    .expect_err("missing packaged asset should fail runtime smoke");
    let err_text = err.to_string();
    assert!(err_text.contains("export.runtime_smoke.asset_load"));
    assert!(err_text.contains("cause="));
    assert!(err_text.contains("action="));
    assert!(err_text.contains("consequence="));

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["schema"], "vnengine.player_runtime_smoke.v1");
    assert_eq!(smoke["status"], "failed");
    assert_eq!(smoke["smoke_result"]["status"], "failed");
    assert_eq!(smoke["target_platform"], "windows");
    let failed_check = smoke["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.asset_load")
        .expect("asset load failure smoke check");
    assert_eq!(failed_check["status"], "failed");
    assert_eq!(failed_check["severity"], "error");
    assert_eq!(failed_check["blocking_release"], true);
    assert_eq!(failed_check["asset"], "assets/backgrounds/missing.png");
    assert!(failed_check["trace_id"]
        .as_str()
        .unwrap()
        .starts_with("export-smoke-"));
    assert!(failed_check["probable_cause"]
        .as_str()
        .unwrap()
        .contains("asset"));
    assert!(failed_check["suggested_action"]
        .as_str()
        .unwrap()
        .contains("asset"));
    assert!(failed_check["consequence"]
        .as_str()
        .unwrap()
        .contains("frame"));

    let package: Value = serde_json::from_str(&std::fs::read_to_string(
        bundle.join("meta/package_report.json"),
    )?)?;
    let compat: Value = serde_json::from_str(&std::fs::read_to_string(
        bundle.join("meta/compat_report.json"),
    )?)?;
    assert_eq!(package["smoke_result"], smoke["smoke_result"]);
    assert_eq!(compat["smoke_result"], smoke["smoke_result"]);
    assert_eq!(package["smoke_result"]["status"], "failed");
    Ok(())
}

#[test]
fn smoke_mode_persists_failed_report_when_package_report_is_corrupt(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let bundle = dir.path();
    std::fs::create_dir_all(bundle.join("scripts"))?;
    std::fs::create_dir_all(bundle.join("meta"))?;

    std::fs::write(
        bundle.join("scripts/compiled.vnscript.json"),
        serde_json::json!({
            "script_schema_version": visual_novel_engine::SCRIPT_SCHEMA_VERSION,
            "events": [
                {
                    "type": "dialogue",
                    "speaker": "Narrator",
                    "text": "Smoke should fail on corrupt package report first"
                }
            ],
            "labels": {
                "start": 0
            }
        })
        .to_string(),
    )?;
    std::fs::write(
        bundle.join("meta/assets_manifest.json"),
        serde_json::json!({
            "manifest_version": 1,
            "assets": {}
        })
        .to_string(),
    )?;
    std::fs::write(bundle.join("meta/package_report.json"), "{not-json")?;
    std::fs::write(
        bundle.join("meta/compat_report.json"),
        serde_json::json!({
            "schema": "vnengine.export_compat_report.v1",
            "target_platform": "windows",
            "smoke_result": {
                "status": "not_run",
                "backend": "software",
                "details": "pending"
            }
        })
        .to_string(),
    )?;
    std::fs::write(bundle.join("meta/bundle_file_manifest.json"), "{}")?;
    std::fs::write(bundle.join("meta/bundle.hmac_sha256"), "abc123")?;

    let smoke_report = bundle.join("meta/runtime_smoke_report.json");
    let err = run_from_args(vec![
        bundle
            .join("scripts/compiled.vnscript.json")
            .display()
            .to_string(),
        "--assets-root".to_string(),
        bundle.display().to_string(),
        "--manifest".to_string(),
        bundle
            .join("meta/assets_manifest.json")
            .display()
            .to_string(),
        "--require-manifest".to_string(),
        "--smoke".to_string(),
        "--smoke-report".to_string(),
        smoke_report.display().to_string(),
    ])
    .expect_err("corrupt package report should fail runtime smoke");
    let err_text = err.to_string();
    assert!(err_text.contains("export.runtime_smoke.report_parse"));
    assert!(err_text.contains("target=windows"));
    assert!(err_text.contains("file=meta/package_report.json"));
    assert!(err_text.contains("field=package_report"));
    assert!(err_text.contains("trace_id=export-smoke-"));
    assert!(err_text.contains("cause="));
    assert!(err_text.contains("action="));
    assert!(err_text.contains("consequence="));

    let smoke: Value = serde_json::from_str(&std::fs::read_to_string(&smoke_report)?)?;
    assert_eq!(smoke["schema"], "vnengine.player_runtime_smoke.v1");
    assert_eq!(smoke["status"], "failed");
    assert_eq!(smoke["target_platform"], "windows");
    assert_eq!(smoke["smoke_result"]["status"], "failed");
    let failed_check = smoke["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["code"] == "export.runtime_smoke.report_parse")
        .expect("report parse failure smoke check");
    assert_eq!(failed_check["status"], "failed");
    assert_eq!(failed_check["severity"], "error");
    assert_eq!(failed_check["blocking_release"], true);
    assert_eq!(failed_check["file"], "meta/package_report.json");
    assert_eq!(failed_check["field"], "package_report");
    assert!(failed_check["trace_id"]
        .as_str()
        .unwrap()
        .starts_with("export-smoke-"));
    assert!(failed_check["probable_cause"]
        .as_str()
        .unwrap()
        .contains("report"));
    assert!(failed_check["suggested_action"]
        .as_str()
        .unwrap()
        .contains("report"));
    assert!(failed_check["consequence"].as_str().unwrap().contains("CI"));

    let compat: Value = serde_json::from_str(&std::fs::read_to_string(
        bundle.join("meta/compat_report.json"),
    )?)?;
    assert_eq!(
        compat["smoke_result"], smoke["smoke_result"],
        "valid compat report should still receive the failed smoke_result"
    );
    Ok(())
}
