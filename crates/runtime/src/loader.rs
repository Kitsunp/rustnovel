use std::path::{Component, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::Arc;
use std::thread;

use visual_novel_engine::AssetId;

#[derive(Debug)]
pub struct LoadRequest {
    pub id: AssetId,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct LoadResult {
    pub id: AssetId,
    pub data: Result<Vec<u8>, String>,
}

pub struct AsyncLoader {
    sender: SyncSender<LoadRequest>,
    receiver: Receiver<LoadResult>,
    inflight: Arc<AtomicUsize>,
    _thread_handle: Option<thread::JoinHandle<()>>,
}

impl Default for AsyncLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncLoader {
    pub fn new() -> Self {
        const MAX_INFLIGHT: usize = 32;
        let (sender, request_rx) = mpsc::sync_channel::<LoadRequest>(MAX_INFLIGHT);
        let (result_tx, receiver) = mpsc::channel::<LoadResult>();
        let inflight = Arc::new(AtomicUsize::new(0));
        let inflight_thread = inflight.clone();

        let handle = thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                let data = if is_safe_path(&request.path) {
                    std::fs::read(&request.path).map_err(|e| format!("{}", e))
                } else {
                    Err("Security violation: path traversal or absolute path".to_string())
                };

                inflight_thread.fetch_sub(1, Ordering::Release);
                drop(result_tx.send(LoadResult {
                    id: request.id,
                    data,
                }));
            }
        });

        Self {
            sender,
            receiver,
            inflight,
            _thread_handle: Some(handle),
        }
    }

    pub fn enqueue(&self, id: AssetId, path: PathBuf) {
        self.inflight.fetch_add(1, Ordering::Release);
        // Blocks if too many requests are inflight (Backpressure)
        let _ = self.sender.send(LoadRequest { id, path });
    }

    pub fn try_recv(&self) -> Option<LoadResult> {
        self.receiver.try_recv().ok()
    }

    pub fn is_loading(&self) -> bool {
        self.inflight.load(Ordering::Acquire) > 0
    }
}

fn is_safe_path(path: &std::path::Path) -> bool {
    if path.is_absolute() {
        return false;
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_path_traversal() {
        let loader = AsyncLoader::new();
        let id = AssetId::from_path("hax");
        let path = PathBuf::from("../Cargo.toml"); // Parent dir attempt

        loader.enqueue(id, path);

        let mut result = None;
        for _ in 0..100 {
            if let Some(res) = loader.try_recv() {
                result = Some(res);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let result = result.expect("Loader should return error result");
        assert!(result.data.is_err(), "Should detect security violation");
        let err = result.data.unwrap_err();
        assert!(
            err.contains("Security violation"),
            "Error should be specific: {err}"
        );
    }

    #[test]
    fn test_async_loading_behavior() {
        // Engineer Manifesto: Air Gapped / Concurrency.
        // Ensure loading happens off-thread and doesn't block immediately.

        let loader = AsyncLoader::new();
        let id = AssetId::from_path("test_asset");
        let path = PathBuf::from("Cargo.toml"); // Use a file that exists

        assert!(!loader.is_loading());

        loader.enqueue(id, path);

        // Should register as loading
        assert!(loader.is_loading());

        // Wait for result (in real engine this happens per frame)
        let mut result = None;
        for _ in 0..100 {
            if let Some(res) = loader.try_recv() {
                result = Some(res);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let result = result.expect("Loader should complete");
        assert_eq!(result.id, id);
        let bytes = result.data.expect("Should load file");
        assert!(!bytes.is_empty(), "Should load file content");
        assert!(!loader.is_loading(), "Should update inflight count");
    }

    #[test]
    fn test_async_loading_failure() {
        let loader = AsyncLoader::new();
        let id = AssetId::from_path("missing");
        let path = PathBuf::from("missing.txt");

        loader.enqueue(id, path);

        let mut result = None;
        for _ in 0..100 {
            if let Some(res) = loader.try_recv() {
                result = Some(res);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let result = result.expect("Should return result even on failure");
        assert!(result.data.is_err());
    }
}
