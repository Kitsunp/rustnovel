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
fn async_loader_reports_success_and_inflight_state() {
    let loader = AsyncLoader::new();
    let id = AssetId::from_path("test_asset");

    assert!(!loader.is_loading());
    loader
        .enqueue(id, PathBuf::from("Cargo.toml"))
        .expect("enqueue cargo manifest");
    assert!(loader.is_loading());

    let result = wait_for_result(&loader);
    assert_eq!(result.id, id);
    assert!(!result.data.expect("should load file").is_empty());
    assert!(!loader.is_loading(), "should update inflight count");
}

#[test]
fn async_loader_returns_result_for_missing_file() {
    let loader = AsyncLoader::new();
    let id = AssetId::from_path("missing");

    loader
        .enqueue(id, PathBuf::from("missing.txt"))
        .expect("enqueue missing file request");

    let result = wait_for_result(&loader);
    assert!(result.data.is_err());
}
