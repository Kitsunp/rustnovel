use std::cell::Cell;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread;

use visual_novel_engine::AssetId;
use vnengine_assets::{read_asset_bytes, sanitize_rel_path, AssetLimits};

const MAX_PENDING: usize = 32;

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
    pending: Cell<usize>,
}

impl Default for AsyncLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncLoader {
    /// Starts a loader with the default asset byte limit.
    pub fn new() -> Self {
        Self::with_max_asset_bytes(AssetLimits::default().max_bytes)
    }

    /// Starts a loader with a custom limit per result. At most 32 requests can
    /// be pending, including completed results that have not been received.
    pub fn with_max_asset_bytes(max_bytes: u64) -> Self {
        let (sender, request_rx) = mpsc::sync_channel::<LoadRequest>(MAX_PENDING);
        let (result_tx, receiver) = mpsc::sync_channel::<LoadResult>(MAX_PENDING);

        thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                let data = read_asset_bytes(&request.path, max_bytes)
                    .map_err(|err| format!("failed to load '{}': {err}", request.path.display()));

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
            pending: Cell::new(0),
        }
    }

    /// Queues a relative asset path. When full, receive a result before retrying.
    pub fn enqueue(&self, id: AssetId, path: PathBuf) -> Result<(), String> {
        let path = sanitize_rel_path(&path)
            .map_err(|err| format!("Security violation for '{}': {err}", path.display()))?;
        if self.pending.get() == MAX_PENDING {
            return Err(format!(
                "async loader queue full ({MAX_PENDING} pending); receive a result before retrying"
            ));
        }
        let path = std::env::current_dir()
            .map_err(|err| format!("async loader cannot resolve '{}': {err}", path.display()))?
            .join(path);
        self.sender
            .try_send(LoadRequest { id, path })
            .map_err(|err| format!("async loader enqueue failed: {err}"))?;
        self.pending.set(self.pending.get() + 1);
        Ok(())
    }

    pub fn try_recv(&self) -> Result<Option<LoadResult>, String> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.pending.set(self.pending.get() - 1);
                Ok(Some(result))
            }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => {
                Err("async loader result channel disconnected".to_string())
            }
        }
    }

    /// Returns true while an accepted request has a result still to be received.
    pub fn is_loading(&self) -> bool {
        self.pending.get() > 0
    }
}
