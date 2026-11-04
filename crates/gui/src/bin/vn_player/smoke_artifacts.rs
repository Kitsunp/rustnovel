use super::*;

// Resolve aliases through existing ancestors, including paths whose final file
// (or parent directories) has not yet been created.
fn resolved_output_path(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(mut resolved) => {
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let name = ancestor.file_name().ok_or(err)?;
                missing.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| std::io::Error::other("cannot resolve report path"))?;
            }
            Err(err) => return Err(err),
        }
    }
}

pub(super) fn validate_smoke_report_path(
    path: &Path,
    launch_paths: &LaunchPaths,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let resolved = resolved_output_path(path)?;
    let mut protected = vec![launch_paths.script_path.clone()];
    protected.extend(launch_paths.manifest_path.iter().cloned());
    if let Some(root) = &launch_paths.bundle_root {
        protected.extend(
            [
                "package_report.json",
                "compat_report.json",
                "project.vnm",
                "assets_manifest.json",
                "bundle_file_manifest.json",
                "bundle.hmac_sha256",
            ]
            .iter()
            .map(|name| root.join("meta").join(name)),
        );
        if resolved.starts_with(resolved_output_path(&root.join("assets"))?) {
            return Err(format!(
                "smoke report destination '{}' is inside bundle assets",
                path.display()
            )
            .into());
        }
    }
    for input in protected {
        if resolved == resolved_output_path(&input)? {
            return Err(format!(
                "smoke report path '{}' conflicts with protected input '{}'",
                path.display(),
                input.display()
            )
            .into());
        }
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            // Read at most a bounded amount even when the destination is not ours.
            let existing: Value =
                serde_json::from_slice(&read_bounded_json(path)?).map_err(|_| {
                    format!(
                        "smoke report destination '{}' is an existing non-smoke file",
                        path.display()
                    )
                })?;
            if existing.get("schema").and_then(Value::as_str)
                != Some("vnengine.player_runtime_smoke.v1")
            {
                return Err(format!(
                    "smoke report destination '{}' is an existing non-smoke file",
                    path.display()
                )
                .into());
            }
            if let Some(manifest) = launch_paths.manifest_path.as_deref() {
                let value: Value =
                    serde_json::from_slice(&read_bounded_json(manifest)?).map_err(|err| {
                        format!(
                            "cannot inspect assets manifest '{}': {err}",
                            manifest.display()
                        )
                    })?;
                let assets = value
                    .get("assets")
                    .and_then(Value::as_object)
                    .ok_or_else(|| {
                        format!(
                            "cannot inspect assets manifest '{}': assets must be an object",
                            manifest.display()
                        )
                    })?;
                for asset in assets.keys() {
                    let normalized = asset.trim().replace('\\', "/");
                    let relative = vnengine_assets::sanitize_rel_path(Path::new(&normalized))
                        .map_err(|error| {
                            format!("invalid manifest asset path '{asset}': {error}")
                        })?;
                    if resolved == resolved_output_path(&launch_paths.assets_root.join(relative))? {
                        return Err(format!(
                            "smoke report destination '{}' conflicts with manifest asset '{asset}'",
                            path.display()
                        )
                        .into());
                    }
                }
            }
        }
        Ok(_) => {
            return Err(format!(
                "smoke report destination '{}' is not a regular file",
                path.display()
            )
            .into())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "inspect smoke report destination '{}': {err}",
                path.display()
            )
            .into())
        }
    }
    Ok(resolved)
}

fn read_bounded_json(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!(
            "'{}' exceeds the {} byte inspection limit",
            path.display(),
            MAX_BYTES
        )
        .into());
    }
    Ok(bytes)
}

pub(super) fn persist_failed_smoke_artifacts(
    artifacts: &SmokeArtifactContext,
    err: &(dyn std::error::Error + 'static),
) -> Result<(), Box<dyn std::error::Error>> {
    let smoke_result = failed_smoke_result(&artifacts.target_platform, &[], err);
    write_smoke_artifacts(artifacts, &smoke_result, false).map_err(|report_err| {
        format!("{err}; additionally failed to write runtime smoke failure artifacts: {report_err}")
            .into()
    })
}

pub(super) fn write_smoke_artifacts(
    artifacts: &SmokeArtifactContext,
    smoke_result: &ExportRuntimeSmokeResult,
    strict_metadata: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_root = artifacts.bundle_root.as_deref();
    let mut metadata_checks = Vec::new();
    let bundle_file_manifest_sha256 = match optional_bundle_metadata_file(
        bundle_root,
        "bundle_file_manifest.json",
        "bundle_file_manifest_sha256",
    ) {
        Ok(Some(path)) => match sha256_file(&path) {
            Ok(value) => (!value.is_empty()).then_some(value),
            Err(err) if strict_metadata => {
                return Err(format!(
                    "failed to hash runtime smoke metadata '{}': {err}",
                    path.display()
                )
                .into());
            }
            Err(err) => {
                metadata_checks.push(smoke_metadata_check(
                    &artifacts.target_platform,
                    bundle_root,
                    &path,
                    "bundle_file_manifest_sha256",
                    err.as_ref(),
                ));
                None
            }
        },
        Ok(None) => None,
        Err(failure) if strict_metadata => {
            return Err(smoke_report_read_error(
                &failure,
                &artifacts.target_platform,
            ));
        }
        Err(failure) => {
            metadata_checks.push(smoke_metadata_failure_check(
                &artifacts.target_platform,
                &failure,
                "bundle_file_manifest_sha256",
            ));
            None
        }
    };
    let bundle_hmac_sha256 = match optional_bundle_metadata_file(
        bundle_root,
        "bundle.hmac_sha256",
        "bundle_hmac_sha256",
    ) {
        Ok(Some(path)) => match std::fs::read_to_string(&path) {
            Ok(value) => {
                let value = value.trim().to_string();
                (!value.is_empty()).then_some(value)
            }
            Err(err) if strict_metadata => {
                return Err(format!(
                    "failed to read runtime smoke metadata '{}': {err}",
                    path.display()
                )
                .into());
            }
            Err(err) => {
                metadata_checks.push(smoke_metadata_check(
                    &artifacts.target_platform,
                    bundle_root,
                    &path,
                    "bundle_hmac_sha256",
                    &err,
                ));
                None
            }
        },
        Ok(None) => None,
        Err(failure) if strict_metadata => {
            return Err(smoke_report_read_error(
                &failure,
                &artifacts.target_platform,
            ));
        }
        Err(failure) => {
            metadata_checks.push(smoke_metadata_failure_check(
                &artifacts.target_platform,
                &failure,
                "bundle_hmac_sha256",
            ));
            None
        }
    };
    let mut smoke_result_for_report = smoke_result.clone();
    smoke_result_for_report.checks.extend(metadata_checks);

    let report = PlayerSmokeReport {
        schema: "vnengine.player_runtime_smoke.v1".to_string(),
        status: smoke_result_for_report.status.clone(),
        phase: smoke_result_for_report.phase.clone(),
        target_platform: artifacts.target_platform.clone(),
        backend: smoke_result_for_report.backend.clone(),
        wgpu_fallback: true,
        trace_id: smoke_result_for_report.trace_id.clone(),
        bundle_root: bundle_root.map(normalize_path_display),
        script_path: normalize_path_display(&artifacts.script_path),
        assets_root: normalize_path_display(&artifacts.assets_root),
        manifest_path: artifacts
            .manifest_path
            .as_ref()
            .map(|path| normalize_path_display(path)),
        package_report: artifacts
            .package_report_path
            .as_deref()
            .map(|path| report_relative_path(path, bundle_root)),
        compat_report: artifacts
            .compat_report_path
            .as_deref()
            .map(|path| report_relative_path(path, bundle_root)),
        bundle_file_manifest_sha256,
        bundle_hmac_sha256,
        checks: smoke_result_for_report.checks.clone(),
        smoke_result: smoke_result_for_report.clone(),
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    if !strict_metadata {
        if let Some(path) = artifacts.smoke_report_path.as_deref() {
            visual_novel_engine::atomic_io::atomic_replace(path, report_json.as_bytes())?;
        } else {
            println!("{report_json}");
        }
    }
    let mut updated = Vec::<(PathBuf, Vec<u8>)>::new();
    let publish = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut update_errors = Vec::new();
        for (path, label) in [
            (artifacts.package_report_path.as_deref(), "package_report"),
            (artifacts.compat_report_path.as_deref(), "compat_report"),
        ] {
            let Some(path) = path else {
                continue;
            };
            let result = update_smoke_result(
                path,
                &smoke_result_for_report,
                label,
                &artifacts.target_platform,
                bundle_root,
            );
            match result {
                Ok(original) => updated.push((path.to_path_buf(), original)),
                Err(error) if strict_metadata => return Err(error),
                Err(error) => update_errors.push(error.to_string()),
            }
        }
        if !update_errors.is_empty() {
            return Err(format!(
                "failed to retropropagate smoke_result into all export reports: {}",
                update_errors.join("; ")
            )
            .into());
        }
        if strict_metadata {
            if let Some(path) = artifacts.smoke_report_path.as_deref() {
                visual_novel_engine::atomic_io::atomic_replace(path, report_json.as_bytes())?;
            } else {
                println!("{report_json}");
            }
        }
        Ok(())
    })();
    if let Err(error) = publish {
        let mut rollback_errors = Vec::new();
        for (path, original) in updated.iter().rev().filter(|_| strict_metadata) {
            if let Err(rollback) = visual_novel_engine::atomic_io::atomic_replace(path, original) {
                rollback_errors.push(format!("'{}': {rollback}", path.display()));
            }
        }
        if !rollback_errors.is_empty() {
            return Err(format!(
                "{error}; failed to restore export reports: {}",
                rollback_errors.join("; ")
            )
            .into());
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn failed_smoke_result(
    target_platform: &str,
    checks: &[ExportRuntimeSmokeCheck],
    err: &(dyn std::error::Error + 'static),
) -> ExportRuntimeSmokeResult {
    let failure_check = err
        .downcast_ref::<SmokeFailure>()
        .map(|failure| failure.check.clone())
        .unwrap_or_else(|| {
            smoke_check(
                "export.runtime_smoke.failed",
                "failed",
                target_platform,
                &err.to_string(),
            )
        });
    let mut checks = checks.to_vec();
    if !checks
        .iter()
        .any(|check| check.code == failure_check.code && check.trace_id == failure_check.trace_id)
    {
        checks.push(failure_check.clone());
    }
    ExportRuntimeSmokeResult {
        status: "failed".to_string(),
        backend: "software".to_string(),
        details: failure_check.message.clone(),
        phase: "smoke".to_string(),
        target: target_platform.to_string(),
        trace_id: failure_check.trace_id.clone(),
        checks,
    }
}

pub(super) fn verify_frame_assets(
    app: &RuntimeApp<ConfigurableInput, SilentAudio, vnengine_assets::AssetStore>,
    target: &str,
    checks: &mut Vec<ExportRuntimeSmokeCheck>,
) -> Result<(), Box<dyn std::error::Error>> {
    for command in &app.scene_frame().commands {
        let RenderCommand::Image { asset, .. } = command else {
            continue;
        };
        if app.scene_frame().visual.is_implicit_character_image(asset) {
            continue;
        }
        app.assets().load_bytes_shared(asset).map_err(|err| {
            smoke_error_with_scope(
                "export.runtime_smoke.asset_load",
                target,
                format!("asset='{asset}': {err}"),
                SmokeCheckScope {
                    asset: Some(asset),
                    ..SmokeCheckScope::default()
                },
            )
        })?;
        checks.push(smoke_check_with_asset(
            "export.runtime_smoke.asset_load",
            "passed",
            target,
            &format!("asset='{asset}' loaded from packaged bundle"),
            asset,
        ));
    }
    Ok(())
}

pub(super) fn present_runtime_frame(
    app: &RuntimeApp<ConfigurableInput, SilentAudio, vnengine_assets::AssetStore>,
    drawer: &mut BuiltinSoftwareDrawer,
    pixels: &mut [u8],
    target: &str,
    checks: &mut Vec<ExportRuntimeSmokeCheck>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut presenter = RuntimeSceneFramePresenter::default();
    let display = DisplayProfile::new(960.0, 540.0);
    let mut frame = app.scene_frame().clone();
    frame.layout = Some(resolve_layout(
        display.clone(),
        StageProfile::default(),
        LayoutPolicy::default(),
    ));
    let response = presenter.present(&frame, &display, &UiTheme::default());
    if !response.diagnostics.is_empty() {
        return Err(smoke_error(
            "export.runtime_smoke.render_frame",
            target,
            format!("frame diagnostics: {}", response.diagnostics.join("; ")),
        ));
    }
    drawer.draw(
        pixels,
        (960, 540),
        RenderFrame {
            ui: app.ui(),
            scene_frame: &frame,
            assets: app.assets(),
        },
    );
    for command in &frame.commands {
        let RenderCommand::Image { asset, .. } = command else {
            continue;
        };
        if !frame.visual.is_implicit_character_image(asset) {
            if let Some(error) = drawer.image_error(asset) {
                return Err(smoke_error_with_scope(
                    "export.runtime_smoke.render_frame",
                    target,
                    format!("asset='{asset}': {error}"),
                    SmokeCheckScope {
                        asset: Some(asset),
                        ..SmokeCheckScope::default()
                    },
                ));
            }
        }
    }
    checks.push(smoke_check(
        "export.runtime_smoke.render_frame",
        "passed",
        target,
        &format!(
            "frame rendered commands={} images={} buttons={}",
            presenter.last_command_count, presenter.last_image_count, presenter.last_button_count
        ),
    ));
    Ok(())
}

pub(super) fn read_json_value(
    path: Option<&Path>,
    label: &'static str,
    bundle_root: Option<&Path>,
) -> Result<Option<Value>, SmokeReportReadFailure> {
    let Some(path) = path else {
        return Ok(None);
    };
    let file = report_relative_path(path, bundle_root);
    let raw = std::fs::read_to_string(path).map_err(|err| SmokeReportReadFailure {
        code: "export.runtime_smoke.report_read",
        label,
        path: path.to_path_buf(),
        file: file.clone(),
        detail: err.to_string(),
    })?;
    let value = serde_json::from_str(&raw).map_err(|err| SmokeReportReadFailure {
        code: "export.runtime_smoke.report_parse",
        label,
        path: path.to_path_buf(),
        file,
        detail: err.to_string(),
    })?;
    Ok(Some(value))
}

pub(super) fn optional_bundle_metadata_file(
    bundle_root: Option<&Path>,
    filename: &'static str,
    label: &'static str,
) -> Result<Option<PathBuf>, SmokeReportReadFailure> {
    let Some(root) = bundle_root else {
        return Ok(None);
    };
    let path = root.join("meta").join(filename);
    let file = report_relative_path(&path, Some(root));
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(Some(path)),
        Ok(_) => Err(SmokeReportReadFailure {
            code: "export.runtime_smoke.report_read",
            label,
            path,
            file,
            detail: "metadata path is not a regular file".to_string(),
        }),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(SmokeReportReadFailure {
            code: "export.runtime_smoke.report_read",
            label,
            path,
            file,
            detail: format!("failed to inspect metadata path: {err}"),
        }),
    }
}

pub(super) fn report_target_platform(report: &Option<Value>) -> Option<&str> {
    report
        .as_ref()
        .and_then(|value| value.get("target_platform"))
        .and_then(Value::as_str)
}

pub(super) fn smoke_report_read_error(
    failure: &SmokeReportReadFailure,
    target: &str,
) -> Box<dyn std::error::Error> {
    smoke_error_with_scope(
        failure.code,
        target,
        format!(
            "{} '{}': {}",
            failure.label,
            failure.path.display(),
            failure.detail
        ),
        SmokeCheckScope {
            file: Some(&failure.file),
            field: Some(failure.label),
            ..SmokeCheckScope::default()
        },
    )
}

pub(super) fn smoke_metadata_failure_check(
    target: &str,
    failure: &SmokeReportReadFailure,
    field: &'static str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(
        "export.runtime_smoke.metadata_read",
        "failed",
        target,
        &format!("{field} '{}': {}", failure.path.display(), failure.detail),
        SmokeCheckScope {
            file: Some(&failure.file),
            field: Some(field),
            ..SmokeCheckScope::default()
        },
    )
}

pub(super) fn update_smoke_result(
    path: &Path,
    smoke_result: &ExportRuntimeSmokeResult,
    label: &str,
    target: &str,
    bundle_root: Option<&Path>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = report_relative_path(path, bundle_root);
    let raw = read_bounded_json(path).map_err(|err| {
        smoke_error_with_scope(
            "export.runtime_smoke.report_read",
            target,
            format!("{label} '{}': {err}", path.display()),
            SmokeCheckScope {
                file: Some(&file),
                field: Some("smoke_result"),
                ..SmokeCheckScope::default()
            },
        )
    })?;
    let mut value: Value = serde_json::from_slice(&raw).map_err(|err| {
        smoke_error_with_scope(
            "export.runtime_smoke.report_parse",
            target,
            format!("{label} '{}': {err}", path.display()),
            SmokeCheckScope {
                file: Some(&file),
                field: Some("smoke_result"),
                ..SmokeCheckScope::default()
            },
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        smoke_error_with_scope(
            "export.runtime_smoke.report_parse",
            target,
            format!("{label} '{}': expected a JSON object", path.display()),
            SmokeCheckScope {
                file: Some(&file),
                field: Some("smoke_result"),
                ..SmokeCheckScope::default()
            },
        )
    })?;
    object.insert("smoke_result".into(), serde_json::to_value(smoke_result)?);
    let json = serde_json::to_string_pretty(&value)?;
    visual_novel_engine::atomic_io::atomic_replace(path, json.as_bytes()).map_err(|err| {
        smoke_error_with_scope(
            "export.runtime_smoke.report_write",
            target,
            format!("{label} '{}': {err}", path.display()),
            SmokeCheckScope {
                file: Some(&file),
                field: Some("smoke_result"),
                ..SmokeCheckScope::default()
            },
        )
    })?;
    Ok(raw)
}
