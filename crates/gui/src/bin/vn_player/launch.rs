use super::*;

pub(super) fn resolve_launch_paths(
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

pub(super) fn resolve_launch_paths_for_exe_dir(
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

pub(super) fn next_value(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    iter.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

pub(super) fn next_path(
    iter: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(next_value(iter, flag)?))
}

pub(super) fn default_script_path() -> PathBuf {
    PathBuf::from("scripts").join("compiled.vnscript.json")
}

pub(super) fn default_script_path_for_exe_dir(exe_dir: Option<&Path>) -> (PathBuf, bool) {
    let relative = default_script_path();
    if let Some(exe_dir) = exe_dir {
        let bundled = exe_dir.join(&relative);
        if bundled.is_file() {
            return (bundled, true);
        }
    }
    (relative, false)
}

pub(super) fn bundle_root_for_script(script_path: &Path) -> Option<PathBuf> {
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

pub(super) fn default_assets_root(script_path: &Path, bundle_root: Option<&Path>) -> PathBuf {
    if let Some(root) = bundle_root {
        return root.to_path_buf();
    }
    script_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("assets"))
}

pub(super) fn print_help() {
    println!(
        "Usage: vn_player [script.json] [--assets-root DIR] [--manifest FILE] [--menu-config FILE] [--preferences-path FILE] [--require-manifest] [--title TITLE]"
    );
}

pub(super) fn load_player_menu_config(
    path: &Path,
) -> Result<PlayerMenuConfig, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(path)?;
    let config = match path.extension().and_then(|value| value.to_str()) {
        Some("toml") | Some("vnm") => toml::from_str(&raw)?,
        _ => serde_json::from_str(&raw)?,
    };
    Ok(config)
}

pub(super) fn load_packaged_project_manifest(
    bundle_root: &Path,
) -> Result<Option<ProjectManifest>, Box<dyn std::error::Error>> {
    let path = bundle_root.join("meta").join("project.vnm");
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => {
            return Err(format!(
                "packaged project manifest '{}' is not a regular file",
                path.display()
            )
            .into());
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "failed to inspect packaged project manifest '{}': {err}",
                path.display()
            )
            .into());
        }
    }
    ProjectManifest::load(&path).map(Some).map_err(|err| {
        format!(
            "failed to load packaged project manifest '{}': {err}",
            path.display()
        )
        .into()
    })
}
