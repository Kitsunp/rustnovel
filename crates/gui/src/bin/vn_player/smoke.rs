use super::*;

pub(super) fn run_smoke(
    script_json: &str,
    launch_paths: &LaunchPaths,
    security_mode: SecurityMode,
    smoke_report_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_root = launch_paths.bundle_root.as_deref();
    let fallback_target = std::env::consts::OS.to_string();
    let failure_artifacts = |target_platform: String,
                             package_report_path: Option<PathBuf>,
                             compat_report_path: Option<PathBuf>| {
        SmokeArtifactContext {
            target_platform,
            bundle_root: launch_paths.bundle_root.clone(),
            script_path: launch_paths.script_path.clone(),
            assets_root: launch_paths.assets_root.clone(),
            manifest_path: launch_paths.manifest_path.clone(),
            package_report_path,
            compat_report_path,
            smoke_report_path: smoke_report_path.clone(),
        }
    };

    let package_report_path =
        match optional_bundle_metadata_file(bundle_root, "package_report.json", "package_report") {
            Ok(path) => path,
            Err(failure) => {
                let artifacts = failure_artifacts(fallback_target.clone(), None, None);
                let err = smoke_report_read_error(&failure, &fallback_target);
                persist_failed_smoke_artifacts(&artifacts, err.as_ref())?;
                return Err(err);
            }
        };
    let compat_report_path =
        match optional_bundle_metadata_file(bundle_root, "compat_report.json", "compat_report") {
            Ok(path) => path,
            Err(failure) => {
                let artifacts =
                    failure_artifacts(fallback_target.clone(), package_report_path.clone(), None);
                let err = smoke_report_read_error(&failure, &fallback_target);
                persist_failed_smoke_artifacts(&artifacts, err.as_ref())?;
                return Err(err);
            }
        };
    let package_report = read_json_value(
        package_report_path.as_deref(),
        "package_report",
        bundle_root,
    );
    let compat_report =
        read_json_value(compat_report_path.as_deref(), "compat_report", bundle_root);
    let target_platform = package_report
        .as_ref()
        .ok()
        .and_then(report_target_platform)
        .or_else(|| compat_report.as_ref().ok().and_then(report_target_platform))
        .unwrap_or(&fallback_target)
        .to_string();

    let artifacts = SmokeArtifactContext {
        target_platform: target_platform.clone(),
        bundle_root: launch_paths.bundle_root.clone(),
        script_path: launch_paths.script_path.clone(),
        assets_root: launch_paths.assets_root.clone(),
        manifest_path: launch_paths.manifest_path.clone(),
        package_report_path,
        compat_report_path,
        smoke_report_path: smoke_report_path.clone(),
    };
    let _package_report = match package_report {
        Ok(value) => value,
        Err(failure) => {
            let err = smoke_report_read_error(&failure, &target_platform);
            persist_failed_smoke_artifacts(&artifacts, err.as_ref())?;
            return Err(err);
        }
    };
    let _compat_report = match compat_report {
        Ok(value) => value,
        Err(failure) => {
            let err = smoke_report_read_error(&failure, &target_platform);
            persist_failed_smoke_artifacts(&artifacts, err.as_ref())?;
            return Err(err);
        }
    };
    let mut checks = Vec::new();

    let smoke_run = (|| -> Result<(), Box<dyn std::error::Error>> {
        checks.push(smoke_check_with_file(
            "export.runtime_smoke.launch_paths",
            "passed",
            &target_platform,
            &format!(
                "script='{}' assets_root='{}' manifest='{}'",
                launch_paths.script_path.display(),
                launch_paths.assets_root.display(),
                launch_paths
                    .manifest_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "none".to_string())
            ),
            &normalize_path_display(&launch_paths.script_path),
        ));

        let script_file = normalize_path_display(&launch_paths.script_path);
        let script = ScriptRaw::from_json(script_json).map_err(|err| {
            smoke_error_with_scope(
                "export.runtime_smoke.script_parse",
                &target_platform,
                format!("script='{}': {err}", launch_paths.script_path.display()),
                SmokeCheckScope {
                    file: Some(&script_file),
                    ..SmokeCheckScope::default()
                },
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.script_parse",
            "passed",
            &target_platform,
            "runtime script parsed with the core schema policy",
        ));

        let engine = Engine::new(
            script,
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )
        .map_err(|err| {
            smoke_error(
                "export.runtime_smoke.engine_init",
                &target_platform,
                format!("engine init failed: {err}"),
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.engine_init",
            "passed",
            &target_platform,
            "engine initialized from packaged runtime script",
        ));

        let manifest_file = launch_paths
            .manifest_path
            .as_ref()
            .map(|path| normalize_path_display(path));
        let asset_store = vnengine_assets::AssetStore::new(
            launch_paths.assets_root.clone(),
            security_mode,
            launch_paths.manifest_path.clone(),
            launch_paths.require_manifest,
        )
        .map_err(|err| {
            smoke_error_with_scope(
                "export.runtime_smoke.asset_manifest",
                &target_platform,
                format!(
                    "assets_root='{}' manifest='{}': {err}",
                    launch_paths.assets_root.display(),
                    launch_paths
                        .manifest_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "none".to_string())
                ),
                SmokeCheckScope {
                    file: manifest_file.as_deref(),
                    ..SmokeCheckScope::default()
                },
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.asset_manifest",
            "passed",
            &target_platform,
            "asset store accepted packaged assets manifest",
        ));

        let mut app = RuntimeApp::new(
            engine,
            ConfigurableInput::default(),
            SilentAudio,
            asset_store,
        )
        .map_err(|err| {
            smoke_error(
                "export.runtime_smoke.runtime_app",
                &target_platform,
                format!("runtime app init failed: {err}"),
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.runtime_app",
            "passed",
            &target_platform,
            "runtime app loaded packaged scene state",
        ));

        verify_frame_assets(&app, &target_platform, &mut checks)?;
        present_runtime_frame(&app, &target_platform, &mut checks)?;

        app.handle_action(InputAction::Advance).map_err(|err| {
            smoke_error(
                "export.runtime_smoke.advance_scene",
                &target_platform,
                format!("advance failed: {err}"),
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.advance_scene",
            "passed",
            &target_platform,
            "runtime advanced one scene step without losing engine errors",
        ));

        verify_frame_assets(&app, &target_platform, &mut checks)?;
        present_runtime_frame(&app, &target_platform, &mut checks)?;

        if app.handle_action(InputAction::Quit).map_err(|err| {
            smoke_error(
                "export.runtime_smoke.close",
                &target_platform,
                format!("close failed: {err}"),
            )
        })? {
            return Err(smoke_error(
                "export.runtime_smoke.close",
                &target_platform,
                "quit action did not close the runtime app",
            ));
        }
        checks.push(smoke_check(
            "export.runtime_smoke.close",
            "passed",
            &target_platform,
            "runtime accepted close action without panic",
        ));

        Ok(())
    })();

    if let Err(err) = smoke_run {
        let smoke_result = failed_smoke_result(&target_platform, &checks, err.as_ref());
        if let Err(report_err) = write_smoke_artifacts(&artifacts, &smoke_result, false) {
            return Err(format!(
                "{err}; additionally failed to write runtime smoke failure artifacts: {report_err}"
            )
            .into());
        }
        return Err(err);
    }

    let trace_id = stable_trace_id(
        "export.runtime_smoke.passed",
        &target_platform,
        &launch_paths.script_path.display().to_string(),
    );
    let smoke_result = ExportRuntimeSmokeResult {
        status: "passed".to_string(),
        backend: "software".to_string(),
        details:
            "loaded package, validated manifest assets, rendered frame, advanced scene and closed"
                .to_string(),
        phase: "smoke".to_string(),
        target: target_platform.clone(),
        trace_id: trace_id.clone(),
        checks: checks.clone(),
    };
    write_smoke_artifacts(&artifacts, &smoke_result, true)?;
    Ok(())
}
