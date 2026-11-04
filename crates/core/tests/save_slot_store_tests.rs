use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use visual_novel_engine::{
    runtime::EngineState, SaveData, SaveSlotStore, SaveStoreError, SAVE_FORMAT_VERSION,
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

    fs::remove_file(&slot_path).unwrap();
    assert!(store.has_quicksave().expect("backup availability"));
    assert_eq!(
        store
            .quickload()
            .expect("recover missing quicksave")
            .state
            .position,
        6
    );
    assert!(store
        .list_slots()
        .unwrap()
        .iter()
        .any(|entry| entry.metadata.quick));

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
    assert!(std::error::Error::source(&err).is_some());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn load_slot_rejects_legacy_plain_payloads() {
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

    let err = store
        .load_slot(1)
        .expect_err("legacy slot payload must be rejected");
    assert!(matches!(
        err,
        SaveStoreError::RecoveryFailed { backup: None, .. }
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn saving_after_corruption_preserves_valid_compatible_backup() {
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::new(root.path().to_path_buf());
    store.save_slot(1, &sample_save(1)).unwrap();
    store.save_slot(1, &sample_save(2)).unwrap();
    let primary = root.path().join("slots/slot_001.vnsav");
    let backup = root.path().join("slots/slot_001.vnsav.bak");
    fs::write(&primary, b"corrupt").unwrap();
    let recovered = store.load_slot_with_recovery(1).unwrap();
    assert_eq!(recovered.data.state.position, 1);
    assert!(recovered.recovery.unwrap().primary_error.is_some());
    store.save_slot(1, &sample_save(3)).unwrap();
    assert_eq!(
        store
            .load_slot_with_recovery(1)
            .unwrap()
            .data
            .state
            .position,
        3
    );
    let backup_save = SaveData::from_any_binary(
        &fs::read(&backup).unwrap(),
        visual_novel_engine::AUTH_SAVE_KEY,
    )
    .unwrap();
    assert_eq!(backup_save.state.position, 1);
    fs::write(&primary, b"corrupt again").unwrap();
    fs::write(&backup, b"also corrupt").unwrap();
    assert!(store.save_slot(1, &sample_save(4)).is_err());
    assert_eq!(fs::read(&primary).unwrap(), b"corrupt again");
    assert_eq!(fs::read(&backup).unwrap(), b"also corrupt");
}

#[test]
fn authenticated_reader_and_slot_store_enforce_explicit_limits() {
    use visual_novel_engine::{SaveError, AUTH_SAVE_KEY};
    struct Fragmented<'a>(&'a [u8]);
    impl std::io::Read for Fragmented<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output.len().min(1);
            self.0.read(&mut output[..count])
        }
    }
    let bytes = sample_save(8)
        .to_authenticated_binary(AUTH_SAVE_KEY)
        .unwrap();
    let limit = bytes.len();
    let loaded =
        SaveData::read_authenticated(std::io::Cursor::new(&bytes), AUTH_SAVE_KEY, limit).unwrap();
    assert_eq!(loaded.state.position, 8);
    assert_eq!(
        SaveData::read_authenticated(Fragmented(&bytes), AUTH_SAVE_KEY, limit)
            .unwrap()
            .state
            .position,
        8
    );
    let mut oversized = bytes.clone();
    oversized.extend_from_slice(&[0; 32]);
    let mut reader = std::io::Cursor::new(oversized);
    assert!(matches!(
        SaveData::read_authenticated(&mut reader, AUTH_SAVE_KEY, limit),
        Err(SaveStoreError::Save(SaveError::TooLarge))
    ));
    assert_eq!(reader.position(), limit as u64 + 1);
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::with_max_save_bytes(root.path().to_path_buf(), limit);
    store.save_slot(1, &sample_save(8)).unwrap();
    store.save_slot(1, &sample_save(8)).unwrap();
    fs::write(root.path().join("slots/slot_001.vnsav"), vec![1; limit + 1]).unwrap();
    let loaded = store.load_slot_with_recovery(1).unwrap();
    assert_eq!(loaded.data.state.position, 8);
    assert_eq!(
        loaded.recovery.unwrap().primary_error,
        Some(SaveError::TooLarge)
    );
    fs::remove_file(root.path().join("slots/slot_001.vnsav")).unwrap();
    assert!(store
        .load_slot_with_recovery(1)
        .unwrap()
        .recovery
        .unwrap()
        .primary_error
        .is_none());
    let tiny = SaveSlotStore::with_max_save_bytes(root.path().to_path_buf(), limit - 1);
    assert!(matches!(
        tiny.save_slot(2, &sample_save(8)),
        Err(SaveStoreError::Save(SaveError::TooLarge))
    ));
    assert!(!root.path().join("slots/slot_002.vnsav").exists());
}

#[test]
fn mismatched_primary_does_not_replace_matching_backup() {
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::new(root.path().to_path_buf());
    store.save_slot(1, &sample_save(1)).unwrap();
    store.save_slot(1, &sample_save(2)).unwrap();
    let primary = root.path().join("slots/slot_001.vnsav");
    let backup = root.path().join("slots/slot_001.vnsav.bak");
    let mut other = sample_save(9);
    other.script_id = [7; 32];
    fs::write(
        &primary,
        other
            .to_authenticated_binary(visual_novel_engine::AUTH_SAVE_KEY)
            .unwrap(),
    )
    .unwrap();
    store.save_slot(1, &sample_save(3)).unwrap();
    let recovered = SaveData::from_any_binary(
        &fs::read(backup).unwrap(),
        visual_novel_engine::AUTH_SAVE_KEY,
    )
    .unwrap();
    assert_eq!(recovered.state.position, 1);
    assert_eq!(recovered.script_id, [3; 32]);
}

#[test]
fn different_script_can_replace_a_valid_primary_when_no_matching_backup_exists() {
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::new(root.path().to_path_buf());
    store.save_slot(1, &sample_save(1)).unwrap();
    let mut other = sample_save(9);
    other.script_id = [7; 32];
    store.save_slot(1, &other).unwrap();
    assert_eq!(store.load_slot(1).unwrap().script_id, [7; 32]);
    let backup = root.path().join("slots/slot_001.vnsav.bak");
    let recovered = SaveData::from_any_binary(
        &fs::read(backup).unwrap(),
        visual_novel_engine::AUTH_SAVE_KEY,
    )
    .unwrap();
    assert_eq!(recovered.script_id, [3; 32]);
}

#[test]
fn listing_reports_inconsistent_or_orphaned_metadata_and_bounds_its_size() {
    use visual_novel_engine::MAX_SAVE_METADATA_BYTES;
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::new(root.path().to_path_buf());
    store.save_slot(1, &sample_save(1)).unwrap();
    store.save_slot(2, &sample_save(2)).unwrap();
    fs::remove_file(root.path().join("slots/slot_002.vnsav")).unwrap();
    fs::copy(
        root.path().join("meta/slot_001.json"),
        root.path().join("meta/slot_003.json"),
    )
    .unwrap();
    fs::write(
        root.path().join("meta/slot_004.json"),
        vec![b' '; MAX_SAVE_METADATA_BYTES + 1],
    )
    .unwrap();
    let listing = store.list_slots_with_issues().unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].metadata.slot_id, 1);
    assert_eq!(listing.issues.len(), 3);
    for (file, message) in [
        ("slot_002.json", "missing"),
        ("slot_003.json", "does not match"),
        ("slot_004.json", "exceeds"),
    ] {
        let issue = listing
            .issues
            .iter()
            .find(|issue| issue.path.file_name().unwrap() == file)
            .unwrap();
        assert!(issue.error.to_string().contains(message), "{}", issue.error);
    }
    assert!(store.list_slots().is_err());
}

#[test]
fn metadata_preflight_failure_does_not_publish_save() {
    let root = tempfile::tempdir().unwrap();
    let store = SaveSlotStore::new(root.path().to_path_buf());
    store.save_slot(1, &sample_save(1)).unwrap();
    let primary = root.path().join("slots/slot_001.vnsav");
    let before = fs::read(&primary).unwrap();
    fs::create_dir(root.path().join("meta/slot_001.json.bak")).unwrap();
    assert!(store.save_slot(1, &sample_save(2)).is_err());
    assert_eq!(fs::read(&primary).unwrap(), before);
    assert!(!root.path().join("slots/slot_001.vnsav.bak").exists());
    fs::remove_dir(root.path().join("meta/slot_001.json.bak")).unwrap();
    let mut oversized = sample_save(2);
    oversized.state.visual.background = Some(
        "x".repeat(visual_novel_engine::MAX_SAVE_METADATA_BYTES + 1)
            .into(),
    );
    assert!(store
        .save_slot(1, &oversized)
        .unwrap_err()
        .to_string()
        .contains("metadata exceeds"));
    assert_eq!(fs::read(&primary).unwrap(), before);
}
