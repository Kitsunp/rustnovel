use vnengine_runtime::{force_wgpu_failure_from_env_value, RuntimeRenderBackendPreference};

#[test]
fn render_backend_env_accepts_auto_software_and_wgpu() {
    assert_eq!(
        RuntimeRenderBackendPreference::from_env_value(None).expect("default"),
        RuntimeRenderBackendPreference::Auto
    );
    assert_eq!(
        RuntimeRenderBackendPreference::from_env_value(Some("auto")).expect("auto"),
        RuntimeRenderBackendPreference::Auto
    );
    assert_eq!(
        RuntimeRenderBackendPreference::from_env_value(Some("software")).expect("software"),
        RuntimeRenderBackendPreference::Software
    );
    assert_eq!(
        RuntimeRenderBackendPreference::from_env_value(Some("pixels")).expect("pixels"),
        RuntimeRenderBackendPreference::Software
    );
    assert_eq!(
        RuntimeRenderBackendPreference::from_env_value(Some("wgpu")).expect("wgpu"),
        RuntimeRenderBackendPreference::Wgpu
    );
}

#[test]
fn render_backend_env_rejects_unknown_backend_names() {
    let err = RuntimeRenderBackendPreference::from_env_value(Some("mystery"))
        .expect_err("unknown backend must be rejected");
    assert!(err.contains("VNENGINE_RENDER_BACKEND"));
}

#[test]
fn force_wgpu_failure_env_parses_truthy_values() {
    for value in ["1", "true", "yes", "on"] {
        assert!(
            force_wgpu_failure_from_env_value(Some(value)),
            "{value} should force WGPU failure"
        );
    }
    for value in [None, Some("0"), Some("false"), Some("off")] {
        assert!(!force_wgpu_failure_from_env_value(value));
    }
}
