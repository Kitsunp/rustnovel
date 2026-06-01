#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::io::ErrorKind;
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

#[path = "vn_player/launch.rs"]
mod launch;
#[path = "vn_player/smoke.rs"]
mod smoke;
#[path = "vn_player/smoke_artifacts.rs"]
mod smoke_artifacts;
#[path = "vn_player/smoke_checks.rs"]
mod smoke_checks;
use launch::*;
use smoke::*;
use smoke_artifacts::*;
use smoke_checks::*;

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

struct LaunchPaths {
    script_path: PathBuf,
    bundle_root: Option<PathBuf>,
    assets_root: PathBuf,
    manifest_path: Option<PathBuf>,
    require_manifest: bool,
}

#[cfg(test)]
#[path = "vn_player/tests/mod.rs"]
mod tests;
