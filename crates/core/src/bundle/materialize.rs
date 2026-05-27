use std::fs;
use std::path::{Path, PathBuf};

use crate::error::VnResult;

use super::helpers::{
    canonicalize_within_root, invalid_bundle, normalize_path_display, sanitize_relative_path,
};
use super::ExportTargetPlatform;

#[derive(Debug, Clone)]
pub(super) struct CopiedRuntimeArtifact {
    pub(super) rel_path: String,
    pub(super) output_path: PathBuf,
}

pub(super) fn copy_runtime_artifact(
    runtime_artifact: Option<&Path>,
    project_root: &Path,
    runtime_output_root: &Path,
) -> VnResult<Option<CopiedRuntimeArtifact>> {
    let Some(raw_path) = runtime_artifact else {
        return Ok(None);
    };

    let source = if raw_path.is_absolute() {
        raw_path
            .canonicalize()
            .map_err(|e| invalid_bundle(format!("canonicalize runtime artifact: {e}")))?
    } else {
        let safe_rel = sanitize_relative_path(raw_path, "runtime_artifact")?;
        canonicalize_within_root(project_root, &safe_rel, "runtime_artifact")?
    };
    if !source.is_file() {
        return Err(invalid_bundle(format!(
            "runtime artifact is not a file '{}'",
            source.display()
        )));
    }

    let file_name = source
        .file_name()
        .ok_or_else(|| invalid_bundle("runtime artifact has no filename"))?;
    let destination = runtime_output_root.join(file_name);
    fs::copy(&source, &destination).map_err(|e| {
        invalid_bundle(format!(
            "copy runtime artifact '{}' -> '{}': {e}",
            source.display(),
            destination.display()
        ))
    })?;

    Ok(Some(CopiedRuntimeArtifact {
        rel_path: normalize_path_display(Path::new("runtime").join(file_name).as_path()),
        output_path: destination,
    }))
}

pub(super) fn materialize_executable(
    target: ExportTargetPlatform,
    output_root: &Path,
    runtime: Option<&CopiedRuntimeArtifact>,
) -> VnResult<Option<String>> {
    let Some(runtime) = runtime else {
        return Ok(None);
    };
    if target != ExportTargetPlatform::Windows || !has_extension(&runtime.output_path, "exe") {
        return Ok(None);
    }

    let executable_rel = "game.exe";
    let executable_out = output_root.join(executable_rel);
    fs::copy(&runtime.output_path, &executable_out).map_err(|e| {
        invalid_bundle(format!(
            "copy windows executable '{}' -> '{}': {e}",
            runtime.output_path.display(),
            executable_out.display()
        ))
    })?;
    Ok(Some(executable_rel.to_string()))
}

pub(super) fn write_launcher(
    target: ExportTargetPlatform,
    output_root: &Path,
    runtime_rel: Option<&str>,
) -> VnResult<String> {
    match target {
        ExportTargetPlatform::Windows => {
            let launcher_path = output_root.join("launch.bat");
            let content = if let Some(runtime) = runtime_rel {
                format!("@echo off\r\nsetlocal\r\n\"%~dp0{runtime}\" %*\r\n")
            } else {
                "@echo off\r\necho Runtime artifact missing in bundle\r\nexit /b 1\r\n".to_string()
            };
            fs::write(&launcher_path, content).map_err(|e| {
                invalid_bundle(format!("write launcher '{}': {e}", launcher_path.display()))
            })?;
            Ok("launch.bat".to_string())
        }
        ExportTargetPlatform::Linux | ExportTargetPlatform::Macos => {
            let launcher_path = output_root.join("launch.sh");
            let content = if let Some(runtime) = runtime_rel {
                format!(
                    "#!/usr/bin/env sh\nset -eu\nDIR=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\"\nexec \"$DIR/{runtime}\" \"$@\"\n"
                )
            } else {
                "#!/usr/bin/env sh\nset -eu\necho \"Runtime artifact missing in bundle\"\nexit 1\n"
                    .to_string()
            };
            fs::write(&launcher_path, content).map_err(|e| {
                invalid_bundle(format!("write launcher '{}': {e}", launcher_path.display()))
            })?;
            Ok("launch.sh".to_string())
        }
    }
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}
