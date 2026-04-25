use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use visual_novel_engine::{
    EngineState, SaveData, SaveSlotStore, SaveStoreError, SAVE_FORMAT_VERSION,
};

fn unique_root(prefix: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{unique}"))
}

fn sample_save(position: u32) -> SaveData {
    let mut state = EngineState::new(position, 16);
    state.set_flag(1, true);
    state.set_var(0, position as i32);
    SaveData::new([3u8; 32], state)
}

#[test]
fn save_slot_compat_matrix() {
    let root = unique_root("vn_slot_compat");
    let store = SaveSlotStore::new(root.clone());

    let initial = sample_save(4);
    let latest = sample_save(11);
    store
        .save_slot(2, &initial)
        .expect("initial save must succeed");
    store
        .save_slot(2, &latest)
        .expect("latest save must succeed");

    let slot_path = root.join("slots").join("slot_002.vnsav");
    let encoded = fs::read(&slot_path).expect("slot bytes should exist");
    assert!(
        SaveData::from_binary(&encoded).is_err(),
        "slot store should persist authenticated payloads"
    );

    let mut bytes = encoded;
    let incompatible_version = SAVE_FORMAT_VERSION.saturating_add(1);
    bytes[4..6].copy_from_slice(&incompatible_version.to_le_bytes());
    fs::write(&slot_path, bytes).expect("write corrupted version");

    let recovered = store
        .load_slot(2)
        .expect("loader should recover from backup with compatible version");
    assert_eq!(recovered.state.position, 4);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn quicksave_recovery() {
    let root = unique_root("vn_quicksave_recovery");
    let store = SaveSlotStore::new(root.clone());

    store
        .quicksave(&sample_save(6))
        .expect("first quicksave should succeed");
    store
        .quicksave(&sample_save(9))
        .expect("second quicksave should succeed");

    let slot_path = root.join("slots").join("quicksave.vnsav");
    fs::write(&slot_path, [0u8, 1, 2, 3, 4]).expect("corrupt quicksave");

    let recovered = store
        .quickload()
        .expect("quickload should recover from quicksave backup");
    assert_eq!(recovered.state.position, 6);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupted_save_handling() {
    let root = unique_root("vn_corrupted_save_handling");
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout must be creatable");

    fs::write(root.join("slots").join("slot_001.vnsav"), [7u8, 7, 7]).expect("write invalid save");
    let err = store
        .load_slot(1)
        .expect_err("invalid save without backup should fail explicitly");
    assert!(matches!(
        err,
        SaveStoreError::RecoveryFailed { backup: None, .. }
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_slot_accepts_legacy_plain_payloads() {
    let root = unique_root("vn_legacy_slot_payload");
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout must be creatable");

    let save = sample_save(12);
    fs::write(
        root.join("slots").join("slot_001.vnsav"),
        save.to_binary().expect("legacy save"),
    )
    .expect("write legacy save");
    fs::write(
        root.join("meta").join("slot_001.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "slot_id": 1,
            "quick": false,
            "updated_unix_ms": 123,
            "script_id_hex": "11",
            "position": 12,
            "flags_words": 1,
            "vars_count": 1
        }))
        .expect("serialize metadata"),
    )
    .expect("write metadata");

    let loaded = store.load_slot(1).expect("legacy slot should still load");
    assert_eq!(loaded.state.position, 12);

    let _ = fs::remove_dir_all(root);
}
