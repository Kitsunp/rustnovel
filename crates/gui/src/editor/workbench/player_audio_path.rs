pub(super) fn resolve_player_audio_asset_path(
    project_root: Option<&std::path::Path>,
    raw_path: &str,
) -> Option<String> {
    let project_root = project_root?;
    const AUDIO_EXTS: [&str; 5] = ["ogg", "opus", "mp3", "wav", "flac"];
    crate::editor::asset_candidates::resolve_existing_asset_path(
        project_root,
        raw_path,
        &AUDIO_EXTS,
    )
}
