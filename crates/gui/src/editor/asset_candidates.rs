use std::path::Path;

pub fn candidate_asset_paths(asset_path: &str, extensions: &[&str]) -> Vec<String> {
    let normalized = asset_path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    push_unique_candidate(&mut candidates, &normalized);
    if !normalized.starts_with("assets/") {
        push_unique_candidate(&mut candidates, &format!("assets/{normalized}"));
    }

    if Path::new(&normalized).extension().is_none() {
        let base_candidates = candidates.clone();
        for base in base_candidates {
            for extension in extensions {
                push_unique_candidate(&mut candidates, &format!("{base}.{extension}"));
            }
        }
    }

    candidates
}

pub fn resolve_existing_asset_path(
    project_root: &Path,
    asset_path: &str,
    extensions: &[&str],
) -> Option<String> {
    let canonical_root = project_root.canonicalize().ok()?;
    candidate_asset_paths(asset_path, extensions)
        .into_iter()
        .find(|candidate| {
            candidate_is_regular_project_file(project_root, &canonical_root, candidate)
        })
}

fn candidate_is_regular_project_file(
    project_root: &Path,
    canonical_root: &Path,
    candidate: &str,
) -> bool {
    let candidate_path = project_root.join(candidate);
    let Ok(canonical_candidate) = candidate_path.canonicalize() else {
        return false;
    };
    canonical_candidate.starts_with(canonical_root) && canonical_candidate.is_file()
}

fn push_unique_candidate(candidates: &mut Vec<String>, value: &str) {
    if candidates.iter().any(|existing| existing == value) {
        return;
    }
    candidates.push(value.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_file_symlink(link: &Path, target: &Path) -> bool {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(target, link).is_ok()
        }

        #[cfg(not(any(unix, windows)))]
        {
            let _ = link;
            let _ = target;
            false
        }
    }

    #[test]
    fn resolve_existing_asset_path_accepts_regular_project_file(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        std::fs::create_dir_all(dir.path().join("assets/audio"))?;
        std::fs::write(dir.path().join("assets/audio/theme.ogg"), b"audio")?;

        assert_eq!(
            resolve_existing_asset_path(dir.path(), "audio/theme", &["ogg"]),
            Some("assets/audio/theme.ogg".to_string())
        );
        Ok(())
    }

    #[test]
    fn resolve_existing_asset_path_rejects_symlink_escape() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        std::fs::create_dir_all(dir.path().join("assets/audio"))?;
        let outside_audio = outside.path().join("escape.ogg");
        std::fs::write(&outside_audio, b"outside audio")?;
        let link = dir.path().join("assets/audio/escape.ogg");
        if !create_file_symlink(&link, &outside_audio) {
            eprintln!("file symlink creation not supported on this platform");
            return Ok(());
        }

        assert_eq!(
            resolve_existing_asset_path(dir.path(), "audio/escape", &["ogg"]),
            None
        );
        Ok(())
    }
}
