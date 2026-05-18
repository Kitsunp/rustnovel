use super::*;
use crate::EventRaw;

fn pos(x: f32, y: f32) -> AuthoringPosition {
    AuthoringPosition::new(x, y)
}

#[path = "traceability_tests/diagnostics.rs"]
mod diagnostics;
#[path = "traceability_tests/fragments.rs"]
mod fragments;
#[path = "traceability_tests/reports.rs"]
mod reports;
