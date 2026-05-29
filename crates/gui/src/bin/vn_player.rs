#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use visual_novel_engine::{PlayerMenuConfig, ProjectManifest};
use visual_novel_gui::{run_app, SecurityMode, VnConfig};

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
    let script_path = launch_paths.script_path;
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
}
