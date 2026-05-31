use std::path::{Component, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
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
                if result_tx
                    .send(LoadResult {
                        id: request.id,
                        data,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            sender,
            receiver,
            inflight,
            _thread_handle: Some(handle),
        }
    }

    pub fn enqueue(&self, id: AssetId, path: PathBuf) -> Result<(), String> {
        if !is_safe_path(&path) {
            return Err("Security violation: path traversal or absolute path".to_string());
        }
        self.inflight.fetch_add(1, Ordering::Release);
        // Blocks if too many requests are inflight (Backpressure)
        if let Err(err) = self.sender.send(LoadRequest { id, path }) {
            self.inflight.fetch_sub(1, Ordering::Release);
            return Err(format!("async loader enqueue failed: {err}"));
        }
        Ok(())
    }

    pub fn try_recv(&self) -> Result<Option<LoadResult>, String> {
        match self.receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("async loader result channel disconnected".to_string())
            }
        }
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
