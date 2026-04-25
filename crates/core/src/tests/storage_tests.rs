use super::*;

fn sample_save(position: u32) -> SaveData {
    let mut state = EngineState::new(position, 8);
    state.set_flag(2, true);
    state.set_var(1, 42);
    SaveData::new([1u8; 32], state)
}

#[test]
fn slot_store_roundtrip_and_list() {
    let root = std::env::temp_dir().join(format!("vn_slot_store_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());

    let save = sample_save(7);
    let entry = store.save_slot(1, &save).expect("slot save should succeed");
    assert_eq!(entry.metadata.slot_id, 1);
    assert!(!entry.metadata.quick);
    assert!(entry.path.exists());
    let stored = fs::read(&entry.path).expect("read stored slot");
    assert!(
        SaveData::from_binary(&stored).is_err(),
        "slot store should emit authenticated payloads"
    );
    assert_eq!(entry.metadata.chapter_label.as_deref(), None);
    assert_eq!(entry.metadata.summary_line.as_deref(), None);

    let loaded = store.load_slot(1).expect("slot load should succeed");
    assert_eq!(loaded.state.position, 7);
    assert!(loaded.state.get_flag(2));
    assert_eq!(loaded.state.get_var(1), 42);

    let slots = store.list_slots().expect("list slots should succeed");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].metadata.slot_id, 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn quicksave_roundtrip() {
    let root = std::env::temp_dir().join(format!("vn_quicksave_store_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    let save = sample_save(11);

    let entry = store.quicksave(&save).expect("quicksave should succeed");
    assert!(entry.metadata.quick);
    assert_eq!(entry.metadata.slot_id, 0);
    let stored = fs::read(&entry.path).expect("read stored quicksave");
    assert!(
        SaveData::from_binary(&stored).is_err(),
        "quicksave should emit authenticated payloads"
    );

    let loaded = store.quickload().expect("quickload should succeed");
    assert_eq!(loaded.state.position, 11);
    assert_eq!(loaded.state.get_var(1), 42);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn authenticated_save_roundtrip_and_tamper_detection() {
    let key = b"phase10-auth-key";
    let save = sample_save(21);
    let encoded = save
        .to_authenticated_binary(key)
        .expect("authenticated save should encode");
    let decoded = SaveData::from_authenticated_binary(&encoded, key)
        .expect("authenticated save should decode");
    assert_eq!(decoded.state.position, 21);

    let mut tampered = encoded.clone();
    let idx = tampered.len() - 1;
    tampered[idx] ^= 0xFF;
    let err = SaveData::from_authenticated_binary(&tampered, key)
        .expect_err("tampered save must fail auth");
    assert_eq!(err, SaveError::AuthenticationFailed);
}

#[test]
fn from_any_binary_accepts_authenticated_and_legacy_payloads() {
    let key = b"phase10-auth-key";
    let save = sample_save(31);

    let authenticated = save
        .to_authenticated_binary(key)
        .expect("authenticated save should encode");
    let decoded = SaveData::from_any_binary(&authenticated, key)
        .expect("authenticated save should decode through generic loader");
    assert_eq!(decoded.state.position, 31);
    assert_eq!(decoded.state.get_var(1), 42);

    let legacy = save.to_binary().expect("legacy save should encode");
    let decoded_legacy = SaveData::from_any_binary(&legacy, key)
        .expect("legacy save should still decode through generic loader");
    assert_eq!(decoded_legacy.state.position, 31);
    assert!(decoded_legacy.state.get_flag(2));
}

#[test]
fn slot_load_recovers_from_corrupted_primary() {
    let root = std::env::temp_dir().join(format!("vn_slot_recovery_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());

    let save_old = sample_save(3);
    let save_new = sample_save(9);
    store
        .save_slot(7, &save_old)
        .expect("first save should succeed");
    store
        .save_slot(7, &save_new)
        .expect("second save should succeed");

    let primary_path = root.join("slots").join("slot_007.vnsav");
    fs::write(&primary_path, [0u8, 1, 2, 3]).expect("corrupt primary");

    let recovered = store
        .load_slot(7)
        .expect("loader should recover from backup");
    assert_eq!(recovered.state.position, 3);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn quickload_reports_recovery_failure_when_no_backup() {
    let root = std::env::temp_dir().join(format!("vn_quick_recovery_fail_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout");
    let primary_path = root.join("slots").join("quicksave.vnsav");
    fs::write(&primary_path, [9u8, 9, 9]).expect("write invalid quicksave");

    let err = store
        .quickload()
        .expect_err("invalid quicksave without backup must fail");
    assert!(matches!(
        err,
        SaveStoreError::RecoveryFailed { backup: None, .. }
    ));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn list_slots_accepts_legacy_metadata_without_new_fields() {
    let root = std::env::temp_dir().join(format!("vn_slot_legacy_meta_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout");

    let save = sample_save(13);
    let encoded = save.to_binary().expect("encode save");
    fs::write(root.join("slots").join("slot_003.vnsav"), encoded).expect("write save");
    let legacy_meta = serde_json::json!({
        "slot_id": 3,
        "quick": false,
        "updated_unix_ms": 123,
        "script_id_hex": "11",
        "position": 13,
        "flags_words": 1,
        "vars_count": 2
    });
    fs::write(
        root.join("meta").join("slot_003.json"),
        serde_json::to_vec(&legacy_meta).expect("serialize legacy metadata"),
    )
    .expect("write metadata");

    let slots = store.list_slots().expect("list slots");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].metadata.slot_id, 3);
    assert_eq!(slots[0].metadata.chapter_label, None);
    assert_eq!(slots[0].metadata.summary_line, None);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn slot_store_loads_legacy_plain_payloads() {
    let root = std::env::temp_dir().join(format!("vn_slot_legacy_save_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout");

    let save = sample_save(5);
    fs::write(
        root.join("slots").join("slot_001.vnsav"),
        save.to_binary().expect("legacy payload"),
    )
    .expect("write legacy slot");
    let metadata = serde_json::json!({
        "slot_id": 1,
        "quick": false,
        "updated_unix_ms": 123,
        "script_id_hex": "11",
        "position": 5,
        "flags_words": 1,
        "vars_count": 2
    });
    fs::write(
        root.join("meta").join("slot_001.json"),
        serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
    )
    .expect("write metadata");

    let loaded = store
        .load_slot(1)
        .expect("legacy plain slot should still load");
    assert_eq!(loaded.state.position, 5);

    let _ = fs::remove_dir_all(root);
}
