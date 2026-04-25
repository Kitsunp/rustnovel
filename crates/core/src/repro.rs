pub const REPRO_CASE_SCHEMA: &str = "vnengine.repro_case.v1";

mod case;
mod report;
mod run;
mod signatures;

pub use case::ReproCase;
pub use report::{
    ReproMonitor, ReproMonitorResult, ReproOracle, ReproRunReport, ReproStepTrace, ReproStopReason,
};
pub use run::{run_repro_case, run_repro_case_with_limits};

#[cfg(test)]
mod tests;
