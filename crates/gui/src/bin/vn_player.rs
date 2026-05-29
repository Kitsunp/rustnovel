#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use visual_novel_engine::{
    runtime::{Engine, ScriptRaw},
    DisplayProfile, ExportRuntimeSmokeCheck, ExportRuntimeSmokeResult, PlayerMenuConfig,
    ProjectManifest, RenderCommand, ResourceLimiter, SceneFramePresenter, SecurityPolicy, UiTheme,
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
    let project_manifest = launch_paths
        .bundle_root
        .as_ref()
        .map(|root| root.join("meta").join("project.vnm"))
        .filter(|path| path.is_file())
        .and_then(|path| ProjectManifest::load(&path).ok());
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
    let package_report = read_json_value(package_report_path.as_deref(), "package_report")?;
    let compat_report = read_json_value(compat_report_path.as_deref(), "compat_report")?;
    let target_platform = package_report
        .as_ref()
        .and_then(|value| value.get("target_platform"))
        .and_then(Value::as_str)
        .or_else(|| {
            compat_report
                .as_ref()
                .and_then(|value| value.get("target_platform"))
                .and_then(Value::as_str)
        })
        .unwrap_or(std::env::consts::OS)
        .to_string();

    let mut checks = Vec::new();
    checks.push(smoke_check(
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
    ));

    let script = ScriptRaw::from_json(script_json).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.script_parse",
            &target_platform,
            format!("script='{}': {err}", launch_paths.script_path.display()),
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

    let asset_store = vnengine_assets::AssetStore::new(
        launch_paths.assets_root.clone(),
        security_mode,
        launch_paths.manifest_path.clone(),
        launch_paths.require_manifest,
    )
    .map_err(|err| {
        smoke_error(
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

    let bundle_file_manifest_sha256 = bundle_root
        .map(|root| root.join("meta").join("bundle_file_manifest.json"))
        .filter(|path| path.is_file())
        .map(|path| sha256_file(&path))
        .transpose()?;
    let bundle_hmac_sha256 = bundle_root
        .map(|root| root.join("meta").join("bundle.hmac_sha256"))
        .filter(|path| path.is_file())
        .map(std::fs::read_to_string)
        .transpose()?
        .map(|value| value.trim().to_string());

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
    let report = PlayerSmokeReport {
        schema: "vnengine.player_runtime_smoke.v1".to_string(),
        status: smoke_result.status.clone(),
        phase: smoke_result.phase.clone(),
        target_platform: target_platform.clone(),
        backend: smoke_result.backend.clone(),
        wgpu_fallback: true,
        trace_id,
        bundle_root: bundle_root.map(normalize_path_display),
        script_path: normalize_path_display(&launch_paths.script_path),
        assets_root: normalize_path_display(&launch_paths.assets_root),
        manifest_path: launch_paths
            .manifest_path
            .as_ref()
            .map(|path| normalize_path_display(path)),
        package_report: package_report_path
            .as_deref()
            .map(|path| report_relative_path(path, bundle_root)),
        compat_report: compat_report_path
            .as_deref()
            .map(|path| report_relative_path(path, bundle_root)),
        bundle_file_manifest_sha256,
        bundle_hmac_sha256,
        checks,
        smoke_result: smoke_result.clone(),
    };

    if let Some(path) = package_report_path.as_deref() {
        update_smoke_result(path, &smoke_result, "package_report")?;
    }
    if let Some(path) = compat_report_path.as_deref() {
        update_smoke_result(path, &smoke_result, "compat_report")?;
    }

    let report_json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = smoke_report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, report_json)?;
    } else {
        println!("{report_json}");
    }
    Ok(())
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
        RuntimeAssetStore::load_bytes(app.assets(), asset).map_err(|err| {
            smoke_error(
                "export.runtime_smoke.asset_load",
                target,
                format!("asset='{asset}': {err}"),
            )
        })?;
        checks.push(smoke_check(
            "export.runtime_smoke.asset_load",
            "passed",
            target,
            &format!("asset='{asset}' loaded from packaged bundle"),
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
    let response = presenter.present(
        app.scene_frame(),
        &DisplayProfile::new(960.0, 540.0),
        &UiTheme::default(),
    );
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
    label: &str,
) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(path).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.report_read",
            label,
            format!("read '{}': {err}", path.display()),
        )
    })?;
    let value = serde_json::from_str(&raw).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.report_parse",
            label,
            format!("parse '{}': {err}", path.display()),
        )
    })?;
    Ok(Some(value))
}

fn update_smoke_result(
    path: &Path,
    smoke_result: &ExportRuntimeSmokeResult,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.report_read",
            label,
            format!("read '{}': {err}", path.display()),
        )
    })?;
    let mut value: Value = serde_json::from_str(&raw).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.report_parse",
            label,
            format!("parse '{}': {err}", path.display()),
        )
    })?;
    value["smoke_result"] = serde_json::to_value(smoke_result)?;
    let json = serde_json::to_string_pretty(&value)?;
    std::fs::write(path, json).map_err(|err| {
        smoke_error(
            "export.runtime_smoke.report_write",
            label,
            format!("write '{}': {err}", path.display()),
        )
    })?;
    Ok(())
}

fn smoke_check(code: &str, status: &str, target: &str, message: &str) -> ExportRuntimeSmokeCheck {
    ExportRuntimeSmokeCheck {
        code: code.to_string(),
        status: status.to_string(),
        phase: "smoke".to_string(),
        target: target.to_string(),
        trace_id: stable_trace_id(code, target, message),
        message: message.to_string(),
    }
}

fn smoke_error(code: &str, target: &str, detail: impl Into<String>) -> Box<dyn std::error::Error> {
    format!(
        "runtime smoke failed code={code} phase=smoke target={target} trace_id={}: {}",
        stable_trace_id(code, target, ""),
        detail.into()
    )
    .into()
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
        assert!(smoke["checks"].as_array().unwrap().iter().any(|check| {
            check["code"] == "export.runtime_smoke.asset_load"
                && check["trace_id"]
                    .as_str()
                    .unwrap()
                    .starts_with("export-smoke-")
        }));

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
}
