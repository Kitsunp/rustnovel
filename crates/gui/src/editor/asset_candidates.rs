use std::path::Path;

pub(crate) fn candidate_asset_paths(asset_path: &str, extensions: &[&str]) -> Vec<String> {
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

pub(crate) fn resolve_existing_asset_path(
    project_root: &Path,
    asset_path: &str,
    extensions: &[&str],
) -> Option<String> {
    candidate_asset_paths(asset_path, extensions)
        .into_iter()
        .find(|candidate| project_root.join(candidate).exists())
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

    #[test]
    fn candidate_paths_include_normalized_and_assets_prefix() {
        let candidates = candidate_asset_paths(" bg\\theme ", &["ogg"]);
        assert_eq!(
            candidates,
            vec![
                "bg/theme".to_string(),
                "assets/bg/theme".to_string(),
                "bg/theme.ogg".to_string(),
                "assets/bg/theme.ogg".to_string(),
            ]
        );
    }

    #[test]
    fn candidate_paths_append_extensions_without_duplicates() {
        let candidates = candidate_asset_paths("audio/theme", &["ogg", "wav"]);
        assert_eq!(
            candidates,
            vec![
                "audio/theme".to_string(),
                "assets/audio/theme".to_string(),
                "audio/theme.ogg".to_string(),
                "audio/theme.wav".to_string(),
                "assets/audio/theme.ogg".to_string(),
                "assets/audio/theme.wav".to_string(),
            ]
        );
    }
}
