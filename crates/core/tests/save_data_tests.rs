use visual_novel_engine::{EngineState, SaveData, SaveError, SAVE_FORMAT_VERSION};

#[test]
fn save_data_roundtrip_binary_v2() {
    let mut state = EngineState::new(7, 64);
    state.set_flag(2, true);
    state.set_var(1, 42);

    let script_id = [1u8; 32];
    let save = SaveData::new(script_id, state);
    let encoded = save.to_binary().expect("encode save data");
    let decoded = SaveData::from_binary(&encoded).expect("decode save data");

    assert_eq!(decoded.script_id, script_id);
    assert_eq!(decoded.state.position, 7);
    assert!(decoded.state.get_flag(2));
    assert_eq!(decoded.state.get_var(1), 42);
}

#[test]
fn save_data_rejects_old_header_version() {
    let state = EngineState::new(0, 1);
    let save = SaveData::new([0u8; 32], state);
    let mut encoded = save.to_binary().expect("encode save data");
    encoded[4..6].copy_from_slice(&(SAVE_FORMAT_VERSION - 1).to_le_bytes());

    let err = SaveData::from_binary(&encoded).expect_err("must reject stale version");
    assert_eq!(
        err,
        SaveError::IncompatibleVersion {
            found: SAVE_FORMAT_VERSION - 1,
            expected: SAVE_FORMAT_VERSION
        }
    );
}
