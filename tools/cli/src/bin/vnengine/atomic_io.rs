use super::*;

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output parent {}", parent.display()))?;
    }
    let had_existing = existing_regular_output(path)?;
    let (temp_path, mut temp_file) = create_unique_sibling_file(path, "tmp")?;
    temp_file
        .write_all(content)
        .with_context(|| format!("write temp output {}", temp_path.display()))?;
    temp_file
        .sync_all()
        .with_context(|| format!("sync temp output {}", temp_path.display()))?;
    drop(temp_file);

    if had_existing {
        let rollback_path = unique_sibling_path(path, "rollback");
        reject_existing_output_path(&rollback_path, "rollback output")?;
        fs::rename(path, &rollback_path).with_context(|| {
            format!(
                "prepare rollback {} -> {}",
                path.display(),
                rollback_path.display()
            )
        })?;
        match fs::rename(&temp_path, path) {
            Ok(()) => {
                if let Err(err) = remove_file_with_diagnostic(
                    &rollback_path,
                    "remove rollback after atomic write",
                ) {
                    eprintln!("{err:#}");
                }
            }
            Err(err) => {
                let restore_result = fs::rename(&rollback_path, path).with_context(|| {
                    format!(
                        "restore rollback {} -> {} after publish failure",
                        rollback_path.display(),
                        path.display()
                    )
                });
                cleanup_temp_with_diagnostic(&temp_path);
                restore_result?;
                return Err(err).with_context(|| {
                    format!(
                        "publish temp output {} -> {}",
                        temp_path.display(),
                        path.display()
                    )
                });
            }
        }
    } else if let Err(err) = fs::rename(&temp_path, path) {
        cleanup_temp_with_diagnostic(&temp_path);
        return Err(err).with_context(|| {
            format!(
                "publish temp output {} -> {}",
                temp_path.display(),
                path.display()
            )
        });
    }

    Ok(())
}

fn existing_regular_output(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => anyhow::bail!("output path is not a regular file: {}", path.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("inspect output {}", path.display())),
    }
}

fn reject_existing_output_path(path: &Path, role: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("{role} already exists: {}", path.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("inspect {role} {}", path.display())),
    }
}

fn create_unique_sibling_file(path: &Path, role: &str) -> Result<(PathBuf, fs::File)> {
    for _ in 0..16 {
        let candidate = unique_sibling_path(path, role);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create temp output {}", candidate.display()));
            }
        }
    }
    anyhow::bail!(
        "could not allocate unique {role} output for {}",
        path.display()
    )
}

fn unique_sibling_path(path: &Path, role: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    parent.join(format!(".{name}.{role}-{}-{nanos}", process::id()))
}

fn remove_file_with_diagnostic(path: &Path, context: &str) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("{context}: {}", path.display())),
    }
}

fn cleanup_temp_with_diagnostic(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => eprintln!("failed to remove temp output {}: {err}", path.display()),
    }
}
