use visual_novel_engine::{authoring::*, runtime::EventRaw};

pub use visual_novel_engine::{run_repro_case, ReproCase};

mod runtime {
    pub use visual_novel_engine::runtime::*;
}

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

#[path = "authoring_traceability_contract/diagnostics.rs"]
mod diagnostics;
#[path = "authoring_traceability_contract/fragments.rs"]
mod fragments;
#[path = "authoring_traceability_contract/reports.rs"]
mod reports;
