use super::*;

const fn support_when(enabled: bool, support: BehaviorSupport) -> BehaviorSupport {
    if enabled {
        support
    } else {
        BehaviorSupport::Unsupported
    }
}

pub(super) const fn runtime_real_cap(
    visual_state: bool,
    scene_frame: bool,
    interactions: bool,
    asset_refs: bool,
    quick_fixes: bool,
) -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: true,
        headless_supported: true,
        export_supported: true,
        python_supported: true,
        cli_supported: true,
        fidelity: FidelityClass::RuntimeReal,
        compile: BehaviorSupport::Native,
        execute: BehaviorSupport::Native,
        preview: BehaviorSupport::Native,
        visual_state: support_when(visual_state, BehaviorSupport::Native),
        scene_frame: support_when(scene_frame, BehaviorSupport::Native),
        interactions: support_when(interactions, BehaviorSupport::Native),
        asset_refs: support_when(asset_refs, BehaviorSupport::Native),
        validation: BehaviorSupport::Native,
        quick_fixes: support_when(quick_fixes, BehaviorSupport::Native),
    }
}

pub(super) const fn host_required_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: true,
        headless_supported: false,
        export_supported: false,
        python_supported: true,
        cli_supported: false,
        fidelity: FidelityClass::HostRequired,
        compile: BehaviorSupport::Native,
        execute: BehaviorSupport::HostRequired,
        preview: BehaviorSupport::HostRequired,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::Unsupported,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Fallback,
    }
}

pub(super) const fn preview_only_cap(export_supported: bool) -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported,
        python_supported: false,
        cli_supported: export_supported,
        fidelity: FidelityClass::PreviewOnly,
        compile: BehaviorSupport::PreviewOnly,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::PreviewOnly,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::PreviewOnly,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}

pub(super) const fn subgraph_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported: true,
        python_supported: false,
        cli_supported: true,
        fidelity: FidelityClass::PreviewOnly,
        compile: BehaviorSupport::Fallback,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::PreviewOnly,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::PreviewOnly,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Unsupported,
        validation: BehaviorSupport::Native,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}

pub(super) const fn fallback_cap() -> EventCapabilities {
    EventCapabilities {
        editor_supported: true,
        preview_supported: true,
        runtime_supported: false,
        headless_supported: false,
        export_supported: false,
        python_supported: false,
        cli_supported: false,
        fidelity: FidelityClass::FallbackDegraded,
        compile: BehaviorSupport::Fallback,
        execute: BehaviorSupport::Unsupported,
        preview: BehaviorSupport::Fallback,
        visual_state: BehaviorSupport::Unsupported,
        scene_frame: BehaviorSupport::Fallback,
        interactions: BehaviorSupport::Unsupported,
        asset_refs: BehaviorSupport::Fallback,
        validation: BehaviorSupport::Fallback,
        quick_fixes: BehaviorSupport::Unsupported,
    }
}
