use visual_novel_engine::AssetId128;

#[test]
fn asset_id_128_is_deterministic() {
    let a = AssetId128::from_path("bg/room.png");
    let b = AssetId128::from_path("bg/room.png");
    assert_eq!(a, b);
}

#[test]
fn asset_id_128_distinguishes_different_paths() {
    let a = AssetId128::from_path("bg/room.png");
    let b = AssetId128::from_path("bg/forest.png");
    assert_ne!(a, b);
}
