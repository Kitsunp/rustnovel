#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use visual_novel_engine::{
    resolve_layout,
    runtime::{Engine, ScriptRaw},
    DisplayProfile, ExportRuntimeSmokeCheck, ExportRuntimeSmokeResult, LayoutPolicy,
    PlayerMenuConfig, ProjectManifest, RenderCommand, ResourceLimiter, SceneFramePresenter,
    SecurityPolicy, StageProfile, UiTheme,
};
use visual_novel_gui::{run_app, SecurityMode, VnConfig};
use visual_novel_runtime::{
    AssetStore as RuntimeAssetStore, ConfigurableInput, InputAction, RuntimeApp,
    RuntimeSceneFramePresenter, SilentAudio,
};

fn main() {
    if let Err(err) = tracing_subscriber::fmt::try_init() {
        eprintln!("Player logging already initialized or unavailable: {err}");
    }
    if let Err(err) = run_from_args(std::env::args().skip(1)) {
        eprintln!("Error running player: {err}");
        std::process::exit(1);
    }
}

fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<(), Box<dyn std::error::Error>> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    let mut script_path: Option<PathBuf> = None;
    let mut assets_root: Option<PathBuf> = None;
    let mut manifest_path: Option<PathBuf> = None;
    let mut menu_config_path: Option<PathBuf> = None;
    let mut preferences_path: Option<PathBuf> = None;
    let mut title: Option<String> = None;
    let mut smoke = false;
    let mut smoke_report_path: Option<PathBuf> = None;
    let mut require_manifest = false;
    let mut security_mode = SecurityMode::Trusted;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--assets-root" => assets_root = Some(next_path(&mut iter, "--assets-root")?),
            "--manifest" => manifest_path = Some(next_path(&mut iter, "--manifest")?),
            "--menu-config" => menu_config_path = Some(next_path(&mut iter, "--menu-config")?),
            "--preferences-path" => {
                preferences_path = Some(next_path(&mut iter, "--preferences-path")?)
            }
            "--title" => title = Some(next_value(&mut iter, "--title")?),
            "--smoke" => smoke = true,
            "--smoke-report" => smoke_report_path = Some(next_path(&mut iter, "--smoke-report")?),
            "--require-manifest" => require_manifest = true,
            "--untrusted" => security_mode = SecurityMode::Untrusted,
            "--trusted" => security_mode = SecurityMode::Trusted,
            value if value.starts_with('-') => {
                return Err(format!("unknown player argument '{value}'").into());
            }
            value => {
                if script_path.replace(PathBuf::from(value)).is_some() {
                    return Err("player accepts only one script path".into());
                }
            }
        }
    }

    let launch_paths =
        resolve_launch_paths(script_path, assets_root, manifest_path, require_manifest);
    let script_path = launch_paths.script_path.clone();
    let script_json = std::fs::read_to_string(&script_path)?;
    let project_manifest = match launch_paths.bundle_root.as_deref() {
        Some(root) => load_packaged_project_manifest(root)?,
        None => None,
    };
    let player_menu = match menu_config_path {
        Some(path) => load_player_menu_config(&path)?,
        None => project_manifest
            .as_ref()
            .map(|manifest| manifest.settings.player_menu.clone())
            .unwrap_or_default(),
    };
    let (width, height) = project_manifest
        .as_ref()
        .map(|manifest| manifest.settings.resolution)
        .map(|(w, h)| (Some(w as f32), Some(h as f32)))
        .unwrap_or((None, None));

    if smoke {
        run_smoke(
            &script_json,
            &launch_paths,
            security_mode,
            smoke_report_path,
        )?;
        return Ok(());
    }

    run_app(
        script_json,
        Some(VnConfig {
            title: title.unwrap_or_else(|| {
                project_manifest
                    .as_ref()
                    .map(|manifest| manifest.metadata.name.clone())
                    .unwrap_or_else(|| "Visual Novel".to_string())
            }),
            width,
            height,
            assets_root: Some(launch_paths.assets_root),
            manifest_path: launch_paths.manifest_path,
            require_manifest: Some(launch_paths.require_manifest),
            security_mode,
            preferences_path,
            player_menu,
            ..VnConfig::default()
        }),
    )?;
    Ok(())
}

#[derive(Clone, Debug, Serialize)]
struct PlayerSmokeReport {
    schema: String,
    status: String,
    phase: String,
    target_platform: String,
    backend: String,
    wgpu_fallback: bool,
    trace_id: String,
    bundle_root: Option<String>,
    script_path: String,
    assets_root: String,
    manifest_path: Option<String>,
    package_report: Option<String>,
    compat_report: Option<String>,
    bundle_file_manifest_sha256: Option<String>,
    bundle_hmac_sha256: Option<String>,
    checks: Vec<ExportRuntimeSmokeCheck>,
    smoke_result: ExportRuntimeSmokeResult,
}

#[derive(Clone, Debug)]
struct SmokeArtifactContext {
    target_platform: String,
    bundle_root: Option<PathBuf>,
    script_path: PathBuf,
    assets_root: PathBuf,
    manifest_path: Option<PathBuf>,
    package_report_path: Option<PathBuf>,
    compat_report_path: Option<PathBuf>,
    smoke_report_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct SmokeReportReadFailure {
    code: &'static str,
    label: &'static str,
    path: PathBuf,
    file: String,
    detail: String,
}

fn run_smoke(
    script_json: &str,
    launch_paths: &LaunchPaths,
    security_mode: SecurityMode,
    smoke_report_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_root = launch_paths.bundle_root.as_deref();
    let package_report_path = bundle_root
        .map(|root| root.join("meta").join("package_report.json"))
        .filter(|path| path.is_file());
    let compat_report_path = bundle_root
        .map(|root| root.join("meta").join("compat_report.json"))
        .filter(|path| path.is_file());

    let fallback_target = std::env::consts::OS.to_string();
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
        .and_then(Option::as_ref)
        .and_then(|value| value.get("target_platform"))
        .and_then(Value::as_str)
        .or_else(|| {
            compat_report
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .and_then(|value| value.get("target_platform"))
                .and_then(Value::as_str)
        })
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
        smoke_report_path,
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

fn persist_failed_smoke_artifacts(
    artifacts: &SmokeArtifactContext,
    err: &(dyn std::error::Error + 'static),
) -> Result<(), Box<dyn std::error::Error>> {
    let smoke_result = failed_smoke_result(&artifacts.target_platform, &[], err);
    write_smoke_artifacts(artifacts, &smoke_result, false).map_err(|report_err| {
        format!("{err}; additionally failed to write runtime smoke failure artifacts: {report_err}")
            .into()
    })
}

fn write_smoke_artifacts(
    artifacts: &SmokeArtifactContext,
    smoke_result: &ExportRuntimeSmokeResult,
    strict_metadata: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bundle_root = artifacts.bundle_root.as_deref();
    let mut metadata_checks = Vec::new();
    let bundle_file_manifest_sha256 = match bundle_root
        .map(|root| root.join("meta").join("bundle_file_manifest.json"))
        .filter(|path| path.is_file())
    {
        Some(path) => match sha256_file(&path) {
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
        None => None,
    };
    let bundle_hmac_sha256 = match bundle_root
        .map(|root| root.join("meta").join("bundle.hmac_sha256"))
        .filter(|path| path.is_file())
    {
        Some(path) => match std::fs::read_to_string(&path) {
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
        None => None,
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
    if let Some(path) = artifacts.smoke_report_path.as_deref() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, report_json)?;
    } else {
        println!("{report_json}");
    }

    let mut update_errors = Vec::new();
    for (path, label) in [
        (artifacts.package_report_path.as_deref(), "package_report"),
        (artifacts.compat_report_path.as_deref(), "compat_report"),
    ] {
        let Some(path) = path else {
            continue;
        };
        if let Err(err) = update_smoke_result(
            path,
            &smoke_result_for_report,
            &artifacts.target_platform,
            label,
            bundle_root,
        ) {
            if strict_metadata {
                return Err(err);
            }
            update_errors.push(err.to_string());
        }
    }
    if !update_errors.is_empty() {
        return Err(format!(
            "failed to retropropagate smoke_result into all export reports: {}",
            update_errors.join("; ")
        )
        .into());
    }
    Ok(())
}

fn failed_smoke_result(
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

fn verify_frame_assets(
    app: &RuntimeApp<ConfigurableInput, SilentAudio, vnengine_assets::AssetStore>,
    target: &str,
    checks: &mut Vec<ExportRuntimeSmokeCheck>,
) -> Result<(), Box<dyn std::error::Error>> {
    for command in &app.scene_frame().commands {
        let RenderCommand::Image { asset, .. } = command else {
            continue;
        };
        let asset_path = asset.to_string();
        RuntimeAssetStore::load_bytes(app.assets(), asset).map_err(|err| {
            smoke_error_with_scope(
                "export.runtime_smoke.asset_load",
                target,
                format!("asset='{asset_path}': {err}"),
                SmokeCheckScope {
                    asset: Some(&asset_path),
                    ..SmokeCheckScope::default()
                },
            )
        })?;
        checks.push(smoke_check_with_asset(
            "export.runtime_smoke.asset_load",
            "passed",
            target,
            &format!("asset='{asset_path}' loaded from packaged bundle"),
            &asset_path,
        ));
    }
    Ok(())
}

fn present_runtime_frame(
    app: &RuntimeApp<ConfigurableInput, SilentAudio, vnengine_assets::AssetStore>,
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

fn read_json_value(
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

fn smoke_report_read_error(
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

fn update_smoke_result(
    path: &Path,
    smoke_result: &ExportRuntimeSmokeResult,
    label: &str,
    target: &str,
    bundle_root: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = report_relative_path(path, bundle_root);
    let raw = std::fs::read_to_string(path).map_err(|err| {
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
    let mut value: Value = serde_json::from_str(&raw).map_err(|err| {
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
    value["smoke_result"] = serde_json::to_value(smoke_result)?;
    let json = serde_json::to_string_pretty(&value)?;
    std::fs::write(path, json).map_err(|err| {
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
    Ok(())
}

#[derive(Default)]
struct SmokeCheckScope<'a> {
    file: Option<&'a str>,
    asset: Option<&'a str>,
    node: Option<&'a str>,
    field: Option<&'a str>,
}

#[derive(Clone, Debug)]
struct SmokeFailure {
    check: ExportRuntimeSmokeCheck,
    detail: String,
}

impl std::fmt::Display for SmokeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runtime smoke failed code={} phase={} target={} trace_id={} {} cause={} action={} consequence={}: {}",
            self.check.code,
            self.check.phase,
            self.check.target,
            self.check.trace_id,
            smoke_scope_summary(&self.check),
            self.check.probable_cause,
            self.check.suggested_action,
            self.check.consequence,
            self.detail
        )
    }
}

impl std::error::Error for SmokeFailure {}

fn smoke_check(code: &str, status: &str, target: &str, message: &str) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(code, status, target, message, SmokeCheckScope::default())
}

fn smoke_check_with_file(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    file: &str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(
        code,
        status,
        target,
        message,
        SmokeCheckScope {
            file: Some(file),
            ..SmokeCheckScope::default()
        },
    )
}

fn smoke_check_with_asset(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    asset: &str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(
        code,
        status,
        target,
        message,
        SmokeCheckScope {
            asset: Some(asset),
            ..SmokeCheckScope::default()
        },
    )
}

fn smoke_metadata_check(
    target: &str,
    bundle_root: Option<&Path>,
    path: &Path,
    field: &'static str,
    err: &dyn std::error::Error,
) -> ExportRuntimeSmokeCheck {
    let file = report_relative_path(path, bundle_root);
    smoke_check_with_scope(
        "export.runtime_smoke.metadata_read",
        "failed",
        target,
        &format!("{field} '{}': {err}", path.display()),
        SmokeCheckScope {
            file: Some(&file),
            field: Some(field),
            ..SmokeCheckScope::default()
        },
    )
}

fn smoke_check_with_scope(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    scope: SmokeCheckScope<'_>,
) -> ExportRuntimeSmokeCheck {
    let (probable_cause, suggested_action, consequence) = smoke_metadata(code);
    ExportRuntimeSmokeCheck {
        code: code.to_string(),
        status: status.to_string(),
        severity: smoke_severity(status).to_string(),
        phase: "smoke".to_string(),
        target: target.to_string(),
        trace_id: stable_trace_id(code, target, message),
        message: message.to_string(),
        probable_cause: probable_cause.to_string(),
        suggested_action: suggested_action.to_string(),
        consequence: consequence.to_string(),
        blocking_release: status != "passed",
        file: scope.file.map(ToOwned::to_owned),
        asset: scope.asset.map(ToOwned::to_owned),
        node: scope.node.map(ToOwned::to_owned),
        field: scope.field.map(ToOwned::to_owned),
    }
}

fn smoke_scope_summary(check: &ExportRuntimeSmokeCheck) -> String {
    let mut scope = Vec::new();
    if let Some(file) = &check.file {
        scope.push(format!("file={file}"));
    }
    if let Some(asset) = &check.asset {
        scope.push(format!("asset={asset}"));
    }
    if let Some(node) = &check.node {
        scope.push(format!("node={node}"));
    }
    if let Some(field) = &check.field {
        scope.push(format!("field={field}"));
    }
    if scope.is_empty() {
        "scope=runtime".to_string()
    } else {
        scope.join(" ")
    }
}

fn smoke_error(code: &str, target: &str, detail: impl Into<String>) -> Box<dyn std::error::Error> {
    smoke_error_with_scope(code, target, detail, SmokeCheckScope::default())
}

fn smoke_error_with_scope(
    code: &str,
    target: &str,
    detail: impl Into<String>,
    scope: SmokeCheckScope<'_>,
) -> Box<dyn std::error::Error> {
    let detail = detail.into();
    let check = smoke_check_with_scope(code, "failed", target, &detail, scope);
    Box::new(SmokeFailure { check, detail })
}

fn smoke_severity(status: &str) -> &'static str {
    match status {
        "passed" => "info",
        "not_run" => "warning",
        _ => "error",
    }
}

fn smoke_metadata(code: &str) -> (&'static str, &'static str, &'static str) {
    match code {
        "export.runtime_smoke.launch_paths" => (
            "The launcher, script path, assets root, or manifest path may not match the exported bundle layout.",
            "Check the package report paths, launcher arguments, and bundle root used by the runtime.",
            "The packaged game cannot start from the exported layout.",
        ),
        "export.runtime_smoke.script_parse" => (
            "The packaged runtime script may be corrupt, missing schema data, or incompatible with the core schema policy.",
            "Rebuild the package from the current script and inspect scripts/compiled.vnscript.json.",
            "The runtime cannot initialize the story from the package.",
        ),
        "export.runtime_smoke.engine_init" => (
            "The packaged script may contain invalid labels, choices, resources, or security-sensitive content.",
            "Inspect the script diagnostics and rebuild after fixing the authored flow.",
            "The runtime engine cannot load the exported story.",
        ),
        "export.runtime_smoke.asset_manifest" => (
            "The asset manifest may be missing, malformed, or inconsistent with the packaged assets root.",
            "Compare meta/assets_manifest.json with the package report and exported asset files.",
            "The runtime cannot verify packaged assets before rendering.",
        ),
        "export.runtime_smoke.runtime_app" => (
            "The runtime app could not connect the engine, input, audio, or asset store for the packaged bundle.",
            "Inspect runtime initialization errors and the selected backend/audio configuration.",
            "The game cannot open a playable runtime session.",
        ),
        "export.runtime_smoke.asset_load" => (
            "A referenced packaged asset may be missing, unsafe, or inconsistent with the asset manifest.",
            "Inspect the asset path in the script, package report hashes, and meta/assets_manifest.json.",
            "A rendered frame may be missing required visual content.",
        ),
        "export.runtime_smoke.render_frame" => (
            "The software renderer contract reported invalid commands, layout, image, or button state.",
            "Inspect the scene frame diagnostics, renderer backend, and visual commands for the failing scene.",
            "The package cannot prove it can render a frame on the target backend.",
        ),
        "export.runtime_smoke.advance_scene" => (
            "The runtime engine returned an error while advancing the packaged scene.",
            "Inspect the current event, authored flow, choices, external calls, and runtime error message.",
            "The package cannot prove the player can progress through the exported story.",
        ),
        "export.runtime_smoke.close" => (
            "The runtime did not accept or complete the close action cleanly.",
            "Inspect runtime input handling and shutdown state for the packaged app.",
            "The smoke job cannot prove the game closes without panic or stuck state.",
        ),
        "export.runtime_smoke.report_read" => (
            "A package, compat, or smoke report path could not be read from disk.",
            "Check report paths, staging output, and file permissions.",
            "CI cannot connect runtime smoke evidence back into export artifacts.",
        ),
        "export.runtime_smoke.report_parse" => (
            "A package, compat, or smoke report is not valid JSON for the expected schema.",
            "Inspect the report content and the writer that produced it.",
            "CI cannot safely compare or update export artifacts.",
        ),
        "export.runtime_smoke.report_write" => (
            "The smoke job could not write updated smoke_result back to an export report.",
            "Check output permissions and whether the report path points inside the bundle.",
            "The export artifacts may not record the runtime smoke outcome.",
        ),
        "export.runtime_smoke.metadata_read" => (
            "The smoke job could not read package metadata needed for runtime evidence.",
            "Inspect bundle metadata files, encoding, and filesystem permissions.",
            "The smoke report cannot include complete bundle integrity evidence.",
        ),
        _ => (
            "The runtime smoke check failed for an unspecified package/runtime condition.",
            "Inspect the check code, message, and trace_id in the smoke report.",
            "The package cannot be treated as fully smoke-verified.",
        ),
    }
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stable_trace_id(code: &str, target: &str, subject: &str) -> String {
    let digest = sha256_hex(format!("{code}:{target}:{subject}").as_bytes());
    format!("export-smoke-{}", &digest[..16])
}

fn normalize_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn report_relative_path(path: &Path, bundle_root: Option<&Path>) -> String {
    bundle_root
        .and_then(|root| path.strip_prefix(root).ok())
        .map(normalize_path_display)
        .unwrap_or_else(|| normalize_path_display(path))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LaunchPaths {
    script_path: PathBuf,
    bundle_root: Option<PathBuf>,
    assets_root: PathBuf,
    manifest_path: Option<PathBuf>,
    require_manifest: bool,
}

fn resolve_launch_paths(
    script_path: Option<PathBuf>,
    assets_root: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
    require_manifest: bool,
) -> LaunchPaths {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    resolve_launch_paths_for_exe_dir(
        script_path,
        assets_root,
        manifest_path,
        require_manifest,
        exe_dir.as_deref(),
    )
}

fn resolve_launch_paths_for_exe_dir(
    script_path: Option<PathBuf>,
    assets_root: Option<PathBuf>,
    manifest_path: Option<PathBuf>,
    require_manifest: bool,
    exe_dir: Option<&Path>,
) -> LaunchPaths {
    let script_was_explicit = script_path.is_some();
    let (script_path, discovered_from_exe_dir) = match script_path {
        Some(path) => (path, false),
        None => default_script_path_for_exe_dir(exe_dir),
    };
    let bundle_root = bundle_root_for_script(&script_path);
    let assets_root =
        assets_root.unwrap_or_else(|| default_assets_root(&script_path, bundle_root.as_deref()));
    let inferred_manifest = bundle_root
        .as_ref()
        .map(|root| root.join("meta").join("assets_manifest.json"));
    let manifest_path = manifest_path.or_else(|| {
        inferred_manifest
            .filter(|path| path.is_file() || require_manifest || discovered_from_exe_dir)
    });
    let require_manifest = require_manifest || (!script_was_explicit && discovered_from_exe_dir);

    LaunchPaths {
        script_path,
        bundle_root,
        assets_root,
        manifest_path,
        require_manifest,
    }
}

fn next_value(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    iter.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn next_path(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(next_value(iter, flag)?))
}

fn default_script_path() -> PathBuf {
    PathBuf::from("scripts").join("compiled.vnscript.json")
}

fn default_script_path_for_exe_dir(exe_dir: Option<&Path>) -> (PathBuf, bool) {
    let relative = default_script_path();
    if let Some(exe_dir) = exe_dir {
        let bundled = exe_dir.join(&relative);
        if bundled.is_file() {
            return (bundled, true);
        }
    }
    (relative, false)
}

fn bundle_root_for_script(script_path: &Path) -> Option<PathBuf> {
    let parent = script_path.parent()?;
    if parent.file_name()?.to_string_lossy() != "scripts" {
        return None;
    }
    let root = parent.parent().unwrap_or_else(|| Path::new("."));
    if root.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(root.to_path_buf())
    }
}

fn default_assets_root(script_path: &Path, bundle_root: Option<&Path>) -> PathBuf {
    if let Some(root) = bundle_root {
        return root.to_path_buf();
    }
    script_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("assets"))
}

fn print_help() {
    println!(
        "Usage: vn_player [script.json] [--assets-root DIR] [--manifest FILE] [--menu-config FILE] [--preferences-path FILE] [--require-manifest] [--title TITLE]"
    );
}

fn load_player_menu_config(path: &Path) -> Result<PlayerMenuConfig, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)?;
    let config = match path.extension().and_then(|value| value.to_str()) {
        Some("toml") | Some("vnm") => toml::from_str(&raw)?,
        _ => serde_json::from_str(&raw)?,
    };
    Ok(config)
}

fn load_packaged_project_manifest(
    bundle_root: &Path,
) -> Result<Option<ProjectManifest>, Box<dyn std::error::Error>> {
    let path = bundle_root.join("meta").join("project.vnm");
    if !path.is_file() {
        return Ok(None);
    }
    ProjectManifest::load(&path).map(Some).map_err(|err| {
        format!(
            "failed to load packaged project manifest '{}': {err}",
            path.display()
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn packaged_project_manifest_parse_errors_are_not_ignored(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir_all(dir.path().join("meta"))?;
        std::fs::write(dir.path().join("meta").join("project.vnm"), "not = [toml")?;

        let err = load_packaged_project_manifest(dir.path())
            .expect_err("existing packaged project manifest must not be silently ignored");
        assert!(err.to_string().contains("project.vnm"));
        Ok(())
    }

    #[test]
    fn smoke_mode_loads_packaged_flow_and_updates_reports() -> Result<(), Box<dyn std::error::Error>>
    {
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
    fn failed_smoke_artifacts_record_metadata_read_errors() -> Result<(), Box<dyn std::error::Error>>
    {
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
    fn smoke_mode_persists_failed_result_and_updates_reports(
    ) -> Result<(), Box<dyn std::error::Error>> {
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
}
