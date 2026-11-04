use std::path::PathBuf;
use std::time::Duration;

use visual_novel_engine::AssetId;
use vnengine_runtime::AsyncLoader;

fn wait_for_result(loader: &AsyncLoader) -> vnengine_runtime::LoadResult {
    for _ in 0..100 {
        if let Some(res) = loader
            .try_recv()
            .expect("loader channel should stay connected")
        {
            return res;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("loader should complete");
}

#[test]
fn async_loader_rejects_parent_dir_traversal() {
    let loader = AsyncLoader::new();
    let id = AssetId::from_path("hax");

    let err = loader
        .enqueue(id, PathBuf::from("../Cargo.toml"))
        .expect_err("unsafe loader request must fail before enqueue");
    assert!(err.contains("Security violation"), "error was {err}");
    assert!(
        !loader.is_loading(),
        "rejected requests must not stay inflight"
    );
}

#[test]
fn async_loader_bounds_unread_results_and_reuses_capacity_after_receiving() {
    let loader = AsyncLoader::new();
    let id = AssetId::from_path("Cargo.toml");

    assert!(!loader.is_loading());
    for _ in 0..32 {
        loader
            .enqueue(id, PathBuf::from("Cargo.toml"))
            .expect("enqueue cargo manifest");
    }
    assert!(loader.is_loading());
    let error = loader
        .enqueue(id, PathBuf::from("Cargo.toml"))
        .expect_err("completed results must still occupy a slot until received");
    assert!(error.contains("queue full"), "{error}");

    let result = wait_for_result(&loader);
    assert_eq!(result.id, id);
    assert!(!result.data.expect("should load file").is_empty());
    loader
        .enqueue(id, PathBuf::from("Cargo.toml"))
        .expect("receiving a result should free a slot");
    for _ in 0..32 {
        assert!(loader.is_loading());
        assert!(wait_for_result(&loader).data.is_ok());
    }
    assert!(!loader.is_loading(), "no accepted results remain");
}

#[test]
fn async_loader_reports_asset_failures_and_releases_their_slots() {
    let loader = AsyncLoader::with_max_asset_bytes(1);
    let id = AssetId::from_path("missing");

    loader
        .enqueue(id, PathBuf::from("missing.txt"))
        .expect("enqueue missing file request");

    let result = wait_for_result(&loader);
    assert!(result
        .data
        .expect_err("missing asset")
        .contains("missing.txt"));
    assert!(!loader.is_loading());

    loader
        .enqueue(id, PathBuf::from("Cargo.toml"))
        .expect("file size is checked by the worker");
    let error = wait_for_result(&loader).data.expect_err("oversized asset");
    assert!(error.contains("asset too large"), "{error}");
    assert!(!loader.is_loading());
}
