use super::*;

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    visual_novel_engine::atomic_io::atomic_replace(path, content).map_err(|err| {
        // Keep rejected output paths as CLI input errors, while retaining IO errors.
        if err.kind() == std::io::ErrorKind::InvalidInput {
            anyhow::anyhow!("{err}")
        } else {
            anyhow::Error::new(err).context(format!("write output {}", path.display()))
        }
    })
}
