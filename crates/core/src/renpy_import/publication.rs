//! Prepare an import separately and restore replaced files on publication errors.
//! This covers reported I/O failures, not process crashes or concurrent writers.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::invalid_import;
use crate::atomic_io::atomic_copy;
use crate::error::VnResult;

pub(super) struct ImportOutput {
    staging: PathBuf,
    retain_for_recovery: bool,
}

impl ImportOutput {
    pub fn new(output: &Path) -> VnResult<Self> {
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|e| invalid_import(format!("create import parent: {e}")))?;
        let staging = parent.join(format!(".renpy-import-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&staging)
            .map_err(|e| invalid_import(format!("create import staging: {e}")))?;
        let prepared = Self {
            staging,
            retain_for_recovery: false,
        };
        fs::create_dir(prepared.root())
            .map_err(|e| invalid_import(format!("prepare import: {e}")))?;
        Ok(prepared)
    }

    pub fn root(&self) -> PathBuf {
        self.staging.join("project")
    }

    pub fn report_path(&self) -> PathBuf {
        self.staging.join("report.json")
    }

    pub fn publish(mut self, output: &Path, report: &Path) -> VnResult<()> {
        let result = self.publish_files(output, report);
        if self.retain_for_recovery {
            return result;
        }
        if let Err(error) = remove_staging(&self.staging) {
            return Err(invalid_import(format!(
                "{}; remove import staging '{}': {error}",
                result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "Import files published".into()),
                self.staging.display()
            )));
        }
        result
    }

    fn publish_files(&mut self, output: &Path, report: &Path) -> VnResult<()> {
        fs::create_dir_all(output)
            .map_err(|e| invalid_import(format!("create output '{}': {e}", output.display())))?;
        let output = output
            .canonicalize()
            .map_err(|e| invalid_import(format!("resolve output: {e}")))?;
        let root = self.root();
        let mut files = Vec::new();
        for entry in WalkDir::new(&root).sort_by_file_name() {
            let entry = entry.map_err(|e| invalid_import(format!("read prepared import: {e}")))?;
            if entry.file_type().is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(&root)
                    .expect("staged file is within root");
                let target = output.join(relative);
                // Imported resources must not follow an existing link out of the project.
                for parent in target
                    .parent()
                    .expect("output file has parent")
                    .ancestors()
                    .take_while(|p| *p != output)
                {
                    if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
                        return Err(invalid_import(format!(
                            "output directory is a symbolic link: '{}'",
                            parent.display()
                        )));
                    }
                }
                files.push((entry.path().to_path_buf(), target));
            }
        }
        files.push((self.report_path(), report.to_path_buf()));

        // Resolve all aliases and check target types before publishing the first file.
        let mut targets = HashSet::new();
        for (_, target) in &mut files {
            let name = target
                .file_name()
                .ok_or_else(|| invalid_import("output file has no name"))?
                .to_os_string();
            let parent = target
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            fs::create_dir_all(parent).map_err(|e| {
                invalid_import(format!("create output parent '{}': {e}", parent.display()))
            })?;
            *target = parent
                .canonicalize()
                .map_err(|e| invalid_import(format!("resolve output parent: {e}")))?
                .join(name);
            match fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.file_type().is_file() => {}
                Ok(_) => {
                    return Err(invalid_import(format!(
                        "output is not a regular file: '{}'",
                        target.display()
                    )))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(invalid_import(format!(
                        "inspect output '{}': {error}",
                        target.display()
                    )))
                }
            }
            #[cfg(windows)]
            let key = target.to_string_lossy().to_lowercase();
            #[cfg(not(windows))]
            let key = target.clone();
            if !targets.insert(key) {
                return Err(invalid_import(format!(
                    "import outputs refer to the same file: '{}'",
                    target.display()
                )));
            }
        }

        let mut published = Vec::new();
        for (index, (source, target)) in files.iter().enumerate() {
            let result = (|| -> std::io::Result<()> {
                let backup = if target.try_exists()? {
                    let backup = self.staging.join("rollback").join(index.to_string());
                    atomic_copy(target, &backup)?;
                    Some(backup)
                } else {
                    None
                };
                atomic_copy(source, target)?;
                published.push((target, backup));
                Ok(())
            })();
            if let Err(error) = result {
                let mut restore_errors = Vec::new();
                for (target, backup) in published.into_iter().rev() {
                    #[cfg(windows)]
                    make_writable(target);
                    let restored = match backup {
                        Some(backup) => atomic_copy(&backup, target),
                        None => fs::remove_file(target),
                    };
                    if let Err(error) = restored {
                        restore_errors.push(format!("{}: {error}", target.display()));
                    }
                }
                let recovery = if restore_errors.is_empty() {
                    String::new()
                } else {
                    self.retain_for_recovery = true;
                    format!(
                        "; restore failed [{}]; backups retained at '{}'",
                        restore_errors.join("; "),
                        self.staging.display()
                    )
                };
                return Err(invalid_import(format!(
                    "publish '{}': {error}{recovery}",
                    target.display()
                )));
            }
        }
        Ok(())
    }
}

impl Drop for ImportOutput {
    fn drop(&mut self) {
        if !self.retain_for_recovery {
            let _ = remove_staging(&self.staging);
        }
    }
}

#[cfg(windows)]
fn make_writable(path: &Path) {
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

fn remove_staging(root: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            make_writable(entry.path());
        }
    }
    match fs::remove_dir_all(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
