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

    let executable_rel = match target {
        ExportTargetPlatform::Windows
            if has_extension(&runtime.output_path, "exe")
                && runtime_artifact_matches_target(&runtime.output_path, target)? =>
        {
            "game.exe"
        }
        ExportTargetPlatform::Linux | ExportTargetPlatform::Macos
            if runtime_artifact_matches_target(&runtime.output_path, target)? =>
        {
            "game"
        }
        ExportTargetPlatform::Windows => return Ok(None),
        ExportTargetPlatform::Linux | ExportTargetPlatform::Macos => return Ok(None),
    };
    let executable_out = output_root.join(executable_rel);
    fs::copy(&runtime.output_path, &executable_out).map_err(|e| {
        invalid_bundle(format!(
            "copy executable '{}' -> '{}': {e}",
            runtime.output_path.display(),
            executable_out.display()
        ))
    })?;
    make_executable(&executable_out)?;
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
                let runtime = runtime.replace('/', "\\");
                format!(
                    "@echo off\r\nsetlocal\r\n\"%~dp0{runtime}\" \"%~dp0scripts\\compiled.vnscript.json\" --assets-root \"%~dp0.\" --manifest \"%~dp0meta\\assets_manifest.json\" --require-manifest %*\r\n"
                )
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
                    "#!/usr/bin/env sh\nset -eu\nDIR=\"$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\"\nexec \"$DIR/{runtime}\" \"$DIR/scripts/compiled.vnscript.json\" --assets-root \"$DIR\" --manifest \"$DIR/meta/assets_manifest.json\" --require-manifest \"$@\"\n"
                )
            } else {
                "#!/usr/bin/env sh\nset -eu\necho \"Runtime artifact missing in bundle\"\nexit 1\n"
                    .to_string()
            };
            fs::write(&launcher_path, content).map_err(|e| {
                invalid_bundle(format!("write launcher '{}': {e}", launcher_path.display()))
            })?;
            make_executable(&launcher_path)?;
            Ok("launch.sh".to_string())
        }
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> VnResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|e| invalid_bundle(format!("read permissions '{}': {e}", path.display())))?
        .permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions)
        .map_err(|e| invalid_bundle(format!("set executable '{}': {e}", path.display())))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> VnResult<()> {
    Ok(())
}

pub(super) fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

pub(super) fn runtime_artifact_matches_target(
    path: &Path,
    target: ExportTargetPlatform,
) -> VnResult<bool> {
    let bytes = fs::read(path)
        .map_err(|e| invalid_bundle(format!("read runtime artifact '{}': {e}", path.display())))?;
    Ok(match target {
        ExportTargetPlatform::Windows => is_windows_pe(&bytes),
        ExportTargetPlatform::Linux => is_linux_elf(&bytes),
        ExportTargetPlatform::Macos => is_macos_mach_o(&bytes),
    })
}

fn is_windows_pe(bytes: &[u8]) -> bool {
    if bytes.len() < 0x40 || &bytes[..2] != b"MZ" {
        return false;
    }
    let pe_offset =
        u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    let Some(signature) = bytes.get(pe_offset..pe_offset + 4) else {
        return false;
    };
    if signature != b"PE\0\0" {
        return false;
    }
    let Some(machine) = bytes.get(pe_offset + 4..pe_offset + 6) else {
        return false;
    };
    matches!(
        u16::from_le_bytes([machine[0], machine[1]]),
        0x014c | 0x8664 | 0xaa64
    )
}

fn is_linux_elf(bytes: &[u8]) -> bool {
    if bytes.len() < 20 || &bytes[..4] != b"\x7fELF" {
        return false;
    }
    let elf_type = u16::from_le_bytes([bytes[16], bytes[17]]);
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    matches!(elf_type, 2 | 3) && matches!(machine, 0x03 | 0x3e | 0xb7)
}

fn is_macos_mach_o(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0xFE, 0xED, 0xFA, 0xCE])
            | Some([0xFE, 0xED, 0xFA, 0xCF])
            | Some([0xCE, 0xFA, 0xED, 0xFE])
            | Some([0xCF, 0xFA, 0xED, 0xFE])
            | Some([0xCA, 0xFE, 0xBA, 0xBE])
            | Some([0xCA, 0xFE, 0xBA, 0xBF])
    )
}
