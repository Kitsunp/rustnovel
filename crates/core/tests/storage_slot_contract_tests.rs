use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use visual_novel_engine::{
    runtime::EngineState, SaveData, SaveError, SaveSlotStore, SaveStoreError, AUTH_SAVE_KEY,
};

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_millis() as u64
}

fn sample_save(position: u32) -> SaveData {
    let mut state = EngineState::new(position, 8);
    state.set_flag(2, true);
    state.set_var(1, 42);
    SaveData::new([1u8; 32], state)
}

fn create_file_symlink(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = link;
        let _ = target;
        false
    }
}

fn sibling_path(root: &Path, suffix: &str) -> std::path::PathBuf {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("save_store");
    root.with_file_name(format!("{name}_{suffix}"))
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

    assert!(
        !store.has_quicksave().expect("quicksave probe should work"),
        "fresh store must not report a quicksave"
    );

    let entry = store.quicksave(&save).expect("quicksave should succeed");
    assert!(entry.metadata.quick);
    assert_eq!(entry.metadata.slot_id, 0);
    assert!(
        store
            .has_quicksave()
            .expect("quicksave probe should work after save"),
        "saved quicksave must be visible to menu availability checks"
    );
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
fn from_any_binary_accepts_authenticated_and_rejects_legacy_payloads() {
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
    let err = SaveData::from_any_binary(&legacy, key)
        .expect_err("legacy save must not decode through generic loader");
    assert_eq!(err, SaveError::InvalidMagic);
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
fn load_slot_rejects_symlink_primary_instead_of_external_save() {
    let root = std::env::temp_dir().join(format!("vn_slot_symlink_load_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout");

    let outside_path = sibling_path(&root, "outside.vnsav");
    fs::write(
        &outside_path,
        sample_save(44)
            .to_authenticated_binary(AUTH_SAVE_KEY)
            .expect("outside save"),
    )
    .expect("write outside save");
    let slot_path = root.join("slots").join("slot_001.vnsav");
    if !create_file_symlink(&slot_path, &outside_path) {
        eprintln!("file symlink creation not supported on this platform");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_file(outside_path);
        return;
    }

    let err = store
        .load_slot(1)
        .expect_err("slot loader must not follow external save symlinks");

    match err {
        SaveStoreError::Io(io_err) => {
            assert_eq!(io_err.kind(), std::io::ErrorKind::InvalidInput);
            assert!(
                io_err.to_string().contains("not a regular file"),
                "unexpected error: {io_err}"
            );
        }
        other => panic!("expected invalid input for symlinked slot, got {other:?}"),
    }
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside_path);
}

#[test]
fn save_slot_does_not_write_through_preexisting_tmp_symlink() {
    let root = std::env::temp_dir().join(format!("vn_slot_tmp_symlink_{}", now_unix_ms()));
    let store = SaveSlotStore::new(root.clone());
    store.ensure_layout().expect("layout");

    let outside_path = sibling_path(&root, "outside.tmp-target");
    fs::write(&outside_path, b"sentinel").expect("write sentinel");
    let tmp_link = root.join("slots").join("slot_001.tmp");
    if !create_file_symlink(&tmp_link, &outside_path) {
        eprintln!("file symlink creation not supported on this platform");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_file(outside_path);
        return;
    }

    let entry = store
        .save_slot(1, &sample_save(77))
        .expect("save should use an unpredictable temporary file");

    assert_eq!(
        fs::read(&outside_path).expect("read sentinel"),
        b"sentinel",
        "save writes must not follow a pre-existing tmp symlink"
    );
    assert!(
        !fs::symlink_metadata(&entry.path)
            .expect("slot metadata")
            .file_type()
            .is_symlink(),
        "published slot must be a regular file, not the pre-existing symlink"
    );
    let loaded = store.load_slot(1).expect("load stored slot");
    assert_eq!(loaded.state.position, 77);

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(outside_path);
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
fn slot_store_rejects_legacy_plain_payloads() {
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

    let err = store
        .load_slot(1)
        .expect_err("legacy plain slot must be rejected");
    assert!(matches!(
        err,
        SaveStoreError::RecoveryFailed {
            primary: SaveError::InvalidMagic,
            backup: None
        }
    ));

    let _ = fs::remove_dir_all(root);
}
