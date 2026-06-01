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
