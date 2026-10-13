use super::*;
use visual_novel_engine::manifest::{
    import_project_asset, AssetImportKind, ProjectManifest, DEFAULT_ASSET_IMPORT_LIMIT,
};

#[derive(Subcommand)]
pub(super) enum AssetCommand {
    /// Import or reuse an image/audio file and atomically update the project manifest.
    Import {
        manifest: PathBuf,
        source: PathBuf,
        #[arg(long)]
        kind: AssetImportKind,
        #[arg(long, default_value_t = DEFAULT_ASSET_IMPORT_LIMIT)]
        max_bytes: u64,
    },
}

pub(super) fn run(command: AssetCommand) -> Result<Option<serde_json::Value>> {
    match command {
        AssetCommand::Import {
            manifest,
            source,
            kind,
            max_bytes,
        } => {
            let root = manifest
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let mut document = ProjectManifest::load(&manifest)?;
            let result =
                import_project_asset(root, &manifest, &mut document, &source, kind, max_bytes)?;
            Ok(Some(serde_json::to_value(result)?))
        }
    }
}
