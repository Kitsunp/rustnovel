//! Headless authoring compile/parity helpers shared by GUI and CLI clients.

mod dry_run;
mod repro;
mod route_sim;
mod signatures;

pub use dry_run::{run_dry_run, DryRunOutcome};
pub use repro::{build_minimal_repro_script, check_preview_runtime_parity};
pub use route_sim::{
    enumerate_choice_routes, enumerate_choice_routes_with_report, simulate_raw_sequence,
    RawStepTrace, RouteEnumerationReport,
};

use std::collections::HashSet;
use std::path::Path;

use crate::{Engine, ResourceLimiter, ScriptRaw, SecurityPolicy, StoryGraph};

use super::{
    default_asset_exists, validate_authoring_graph_with_project_root,
    validate_authoring_graph_with_resolver, LintCode, LintIssue, LintSeverity, NodeGraph,
    ValidationPhase,
};

const REPRO_DEFAULT_RADIUS: usize = 12;
const DRY_RUN_MAX_STEPS: usize = 2048;
const DRY_RUN_EXHAUSTIVE_ROUTE_LIMIT: usize = 32;
const DRY_RUN_EXHAUSTIVE_CHOICE_DEPTH: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChoiceStrategy {
    First,
    Last,
    Alternating,
}

impl ChoiceStrategy {
    pub fn label(self) -> &'static str {
        match self {
            ChoiceStrategy::First => "first",
            ChoiceStrategy::Last => "last",
            ChoiceStrategy::Alternating => "alternating",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChoicePolicy {
    Strategy(ChoiceStrategy),
    Scripted(Vec<usize>),
}

impl ChoicePolicy {
    pub fn label(&self) -> String {
        match self {
            ChoicePolicy::Strategy(strategy) => strategy.label().to_string(),
            ChoicePolicy::Scripted(path) => {
                let route = path
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(".");
                format!("scripted({route})")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationPhase {
    GraphSync,
    GraphValidation,
    ScriptCompile,
    RuntimeInit,
    DryRun,
}

impl CompilationPhase {
    pub fn label(self) -> &'static str {
        match self {
            CompilationPhase::GraphSync => "GRAPH_SYNC",
            CompilationPhase::GraphValidation => "GRAPH_VALIDATION",
            CompilationPhase::ScriptCompile => "SCRIPT_COMPILE",
            CompilationPhase::RuntimeInit => "RUNTIME_INIT",
            CompilationPhase::DryRun => "DRY_RUN",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhaseTrace {
    pub phase: CompilationPhase,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryRunStopReason {
    Finished,
    StepLimit,
    RuntimeError,
}

impl DryRunStopReason {
    pub fn label(self) -> &'static str {
        match self {
            DryRunStopReason::Finished => "finished",
            DryRunStopReason::StepLimit => "step_limit",
            DryRunStopReason::RuntimeError => "runtime_error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DryRunStepTrace {
    pub step: usize,
    pub event_ip: u32,
    pub event_kind: String,
    pub event_signature: String,
    pub visual_background: Option<String>,
    pub visual_music: Option<String>,
    pub character_count: usize,
}

#[derive(Debug, Clone)]
pub struct DryRunReport {
    pub max_steps: usize,
    pub executed_steps: usize,
    pub routes_discovered: usize,
    pub routes_executed: usize,
    pub route_limit_hit: bool,
    pub depth_limit_hit: bool,
    pub stop_reason: DryRunStopReason,
    pub stop_message: String,
    pub failing_event_ip: Option<u32>,
    pub steps: Vec<DryRunStepTrace>,
}

impl DryRunReport {
    pub fn first_event_ip(&self) -> Option<u32> {
        self.steps.first().map(|step| step.event_ip)
    }

    pub fn minimal_repro_script(&self, script: &ScriptRaw, radius: usize) -> Option<ScriptRaw> {
        let candidate_ip = self.failing_event_ip.or_else(|| self.first_event_ip())?;
        build_minimal_repro_script(script, candidate_ip, radius)
    }
}

#[derive(Clone)]
pub struct CompilationResult {
    pub script: ScriptRaw,
    pub engine_result: Result<crate::Engine, String>,
    pub issues: Vec<super::LintIssue>,
    pub phase_trace: Vec<PhaseTrace>,
    pub dry_run_report: Option<DryRunReport>,
}

impl CompilationResult {
    pub fn minimal_repro_script(&self) -> Option<ScriptRaw> {
        self.dry_run_report
            .as_ref()
            .and_then(|report| report.minimal_repro_script(&self.script, REPRO_DEFAULT_RADIUS))
    }
}

pub fn compile_authoring_graph(
    graph: &NodeGraph,
    project_root: Option<&Path>,
) -> CompilationResult {
    let mut phase_trace = Vec::new();

    phase_trace.push(PhaseTrace {
        phase: CompilationPhase::GraphSync,
        ok: true,
        detail: "Graph converted to ScriptRaw".to_string(),
    });
    let script = graph.to_script();

    let mut issues = if let Some(root) = project_root {
        validate_authoring_graph_with_project_root(graph, root)
    } else {
        validate_authoring_graph_with_resolver(graph, default_asset_exists)
    };
    phase_trace.push(PhaseTrace {
        phase: CompilationPhase::GraphValidation,
        ok: !issues
            .iter()
            .any(|issue| issue.severity == LintSeverity::Error),
        detail: format!("{} issue(s) from graph validation", issues.len()),
    });

    let mut dry_run_report = None;
    let engine_result = match script.compile() {
        Ok(compiled) => {
            phase_trace.push(PhaseTrace {
                phase: CompilationPhase::ScriptCompile,
                ok: true,
                detail: "ScriptRaw compiled successfully".to_string(),
            });

            let story_graph = StoryGraph::from_script(&compiled);
            for event_ip in story_graph.unreachable_nodes() {
                let incoming = story_graph
                    .incoming_edges(event_ip)
                    .into_iter()
                    .map(|edge| edge.from.to_string())
                    .collect::<Vec<_>>();
                let blocked_by = if incoming.is_empty() {
                    "no incoming compiled edges".to_string()
                } else {
                    format!("incoming_event_ips={}", incoming.join(","))
                };
                issues.push(
                    LintIssue::warning(
                        graph.node_for_event_ip(event_ip),
                        ValidationPhase::DryRun,
                        LintCode::DryRunUnreachableCompiled,
                        format!("Dry Run detected unreachable compiled event at ip={event_ip}"),
                    )
                    .with_event_ip(Some(event_ip))
                    .with_blocked_by(blocked_by),
                );
            }

            match Engine::from_compiled(
                compiled.clone(),
                SecurityPolicy::default(),
                ResourceLimiter::default(),
            ) {
                Ok(engine) => {
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::RuntimeInit,
                        ok: true,
                        detail: "Engine initialized".to_string(),
                    });

                    let primary_policy = ChoicePolicy::Strategy(ChoiceStrategy::First);
                    let outcome = run_dry_run(engine.clone(), &primary_policy, DRY_RUN_MAX_STEPS);
                    dry_run_report = Some(outcome.report.clone());
                    issues.extend(outcome.issues);

                    let parity_issues =
                        check_preview_runtime_parity(&script, &outcome.report, &primary_policy);
                    if let Some(report) = dry_run_report.as_mut() {
                        report.failing_event_ip = report
                            .failing_event_ip
                            .or_else(|| parity_issues.iter().find_map(|issue| issue.event_ip));
                    }
                    issues.extend(parity_issues);

                    let route_report = enumerate_choice_routes_with_report(
                        &script,
                        DRY_RUN_MAX_STEPS,
                        DRY_RUN_EXHAUSTIVE_ROUTE_LIMIT,
                        DRY_RUN_EXHAUSTIVE_CHOICE_DEPTH,
                    );
                    if let Some(report) = dry_run_report.as_mut() {
                        report.routes_discovered = route_report.routes_discovered;
                        report.route_limit_hit = route_report.route_limit_hit;
                        report.depth_limit_hit = route_report.depth_limit_hit;
                    }

                    let mut route_policies = vec![
                        ChoicePolicy::Strategy(ChoiceStrategy::Last),
                        ChoicePolicy::Strategy(ChoiceStrategy::Alternating),
                    ];
                    for path in route_report.routes {
                        route_policies.push(ChoicePolicy::Scripted(path));
                    }

                    let mut seen_policies = HashSet::new();
                    seen_policies.insert(primary_policy);
                    let mut routes_executed = 1usize;
                    for policy in route_policies {
                        if !seen_policies.insert(policy.clone()) {
                            continue;
                        }
                        routes_executed = routes_executed.saturating_add(1);
                        append_route_dry_run_issues(
                            &script,
                            &compiled,
                            &policy,
                            &mut issues,
                            &mut dry_run_report,
                        );
                    }
                    if let Some(report) = dry_run_report.as_mut() {
                        report.routes_executed = routes_executed;
                    }

                    let dry_run_errors = issues
                        .iter()
                        .filter(|issue| {
                            issue.phase == ValidationPhase::DryRun
                                && issue.severity == LintSeverity::Error
                        })
                        .count();
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::DryRun,
                        ok: dry_run_errors == 0,
                        detail: format!("Dry run complete ({} dry-run error(s))", dry_run_errors),
                    });

                    Ok(engine)
                }
                Err(error) => {
                    issues.push(LintIssue::error(
                        None,
                        ValidationPhase::Runtime,
                        LintCode::RuntimeInitError,
                        format!("Runtime initialization failed: {}", error),
                    ));
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::RuntimeInit,
                        ok: false,
                        detail: error.to_string(),
                    });
                    Err(format!("Runtime Init Error: {}", error))
                }
            }
        }
        Err(error) => {
            issues.push(LintIssue::error(
                None,
                ValidationPhase::Compile,
                LintCode::CompileError,
                format!("Compilation Error: {}", error),
            ));
            phase_trace.push(PhaseTrace {
                phase: CompilationPhase::ScriptCompile,
                ok: false,
                detail: error.to_string(),
            });
            Err(format!("Compilation Failed: {}", error))
        }
    };

    CompilationResult {
        script,
        engine_result,
        issues,
        phase_trace,
        dry_run_report,
    }
}

fn append_route_dry_run_issues(
    script: &ScriptRaw,
    compiled: &crate::ScriptCompiled,
    policy: &ChoicePolicy,
    issues: &mut Vec<LintIssue>,
    dry_run_report: &mut Option<DryRunReport>,
) {
    match Engine::from_compiled(
        compiled.clone(),
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    ) {
        Ok(route_engine) => {
            let route_outcome = run_dry_run(route_engine, policy, DRY_RUN_MAX_STEPS);
            let mut route_issues =
                check_preview_runtime_parity(script, &route_outcome.report, policy);
            if route_outcome.report.stop_reason == DryRunStopReason::RuntimeError {
                let route_label = policy.label();
                route_issues.push(
                    LintIssue::error(
                        None,
                        ValidationPhase::DryRun,
                        LintCode::DryRunRuntimeError,
                        format!(
                            "Dry Run route '{}' runtime error: {}",
                            route_label, route_outcome.report.stop_message
                        ),
                    )
                    .with_event_ip(route_outcome.report.failing_event_ip),
                );
            }

            if let Some(report) = dry_run_report.as_mut() {
                report.failing_event_ip = report
                    .failing_event_ip
                    .or_else(|| route_issues.iter().find_map(|issue| issue.event_ip));
            }
            issues.extend(route_issues);
        }
        Err(error) => {
            let route_label = policy.label();
            issues.push(LintIssue::error(
                None,
                ValidationPhase::Runtime,
                LintCode::RuntimeInitError,
                format!(
                    "Runtime initialization failed for route '{}': {}",
                    route_label, error
                ),
            ));
        }
    }
}
