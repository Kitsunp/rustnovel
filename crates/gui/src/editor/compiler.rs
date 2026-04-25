use std::collections::HashSet;
use std::path::Path;

use crate::editor::{
    node_graph::NodeGraph,
    script_sync,
    validator::{self, LintCode, LintIssue, LintSeverity, ValidationPhase},
};
use visual_novel_engine::{Engine, ScriptRaw, StoryGraph};

const DRY_RUN_MAX_STEPS: usize = 2048;
const DRY_RUN_EXHAUSTIVE_ROUTE_LIMIT: usize = 32;
const DRY_RUN_EXHAUSTIVE_CHOICE_DEPTH: usize = 12;
const REPRO_DEFAULT_RADIUS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ChoiceStrategy {
    First,
    Last,
    Alternating,
}

impl ChoiceStrategy {
    fn label(self) -> &'static str {
        match self {
            ChoiceStrategy::First => "first",
            ChoiceStrategy::Last => "last",
            ChoiceStrategy::Alternating => "alternating",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ChoicePolicy {
    Strategy(ChoiceStrategy),
    Scripted(Vec<usize>),
}

impl ChoicePolicy {
    fn label(&self) -> String {
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
    pub engine_result: Result<Engine, String>,
    pub issues: Vec<LintIssue>,
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

pub fn compile_project(graph: &NodeGraph) -> CompilationResult {
    compile_project_with_project_root(graph, None)
}

pub fn compile_project_with_project_root(
    graph: &NodeGraph,
    project_root: Option<&Path>,
) -> CompilationResult {
    let mut phase_trace = Vec::new();

    phase_trace.push(PhaseTrace {
        phase: CompilationPhase::GraphSync,
        ok: true,
        detail: "Graph converted to ScriptRaw".to_string(),
    });
    let script = script_sync::to_script(graph);

    let mut issues = if let Some(root) = project_root {
        validator::validate_with_project_root(graph, root)
    } else {
        validator::validate(graph)
    };
    phase_trace.push(PhaseTrace {
        phase: CompilationPhase::GraphValidation,
        ok: !issues.iter().any(|i| i.severity == LintSeverity::Error),
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
            let unreachable = story_graph.unreachable_nodes();
            if !unreachable.is_empty() {
                for event_ip in unreachable {
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
                    let node_id = graph.node_for_event_ip(event_ip);
                    issues.push(
                        LintIssue::warning(
                            node_id,
                            ValidationPhase::DryRun,
                            LintCode::DryRunUnreachableCompiled,
                            format!("Dry Run detected unreachable compiled event at ip={event_ip}"),
                        )
                        .with_event_ip(Some(event_ip))
                        .with_blocked_by(blocked_by),
                    );
                }
            }

            match Engine::from_compiled(
                compiled.clone(),
                visual_novel_engine::SecurityPolicy::default(),
                visual_novel_engine::ResourceLimiter::default(),
            ) {
                Ok(engine) => {
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::RuntimeInit,
                        ok: true,
                        detail: "Engine initialized".to_string(),
                    });

                    let primary_policy = ChoicePolicy::Strategy(ChoiceStrategy::First);
                    let outcome = run_dry_run(engine.clone(), &primary_policy);
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

                    let mut route_policies = vec![
                        ChoicePolicy::Strategy(ChoiceStrategy::Last),
                        ChoicePolicy::Strategy(ChoiceStrategy::Alternating),
                    ];
                    for path in enumerate_choice_routes(
                        &script,
                        DRY_RUN_MAX_STEPS,
                        DRY_RUN_EXHAUSTIVE_ROUTE_LIMIT,
                        DRY_RUN_EXHAUSTIVE_CHOICE_DEPTH,
                    ) {
                        route_policies.push(ChoicePolicy::Scripted(path));
                    }

                    let mut seen_policies = HashSet::new();
                    seen_policies.insert(primary_policy.clone());
                    for policy in route_policies {
                        if !seen_policies.insert(policy.clone()) {
                            continue;
                        }

                        match Engine::from_compiled(
                            compiled.clone(),
                            visual_novel_engine::SecurityPolicy::default(),
                            visual_novel_engine::ResourceLimiter::default(),
                        ) {
                            Ok(route_engine) => {
                                let route_outcome = run_dry_run(route_engine, &policy);
                                let mut route_issues = check_preview_runtime_parity(
                                    &script,
                                    &route_outcome.report,
                                    &policy,
                                );
                                if route_outcome.report.stop_reason
                                    == DryRunStopReason::RuntimeError
                                {
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
                                    report.failing_event_ip =
                                        report.failing_event_ip.or_else(|| {
                                            route_issues.iter().find_map(|issue| issue.event_ip)
                                        });
                                }
                                issues.extend(route_issues);
                            }
                            Err(err) => {
                                let route_label = policy.label();
                                issues.push(LintIssue::error(
                                    None,
                                    ValidationPhase::Runtime,
                                    LintCode::RuntimeInitError,
                                    format!(
                                        "Runtime initialization failed for route '{}': {}",
                                        route_label, err
                                    ),
                                ));
                            }
                        }
                    }

                    let dry_run_errors = issues
                        .iter()
                        .filter(|i| {
                            i.phase == ValidationPhase::DryRun && i.severity == LintSeverity::Error
                        })
                        .count();
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::DryRun,
                        ok: dry_run_errors == 0,
                        detail: format!("Dry run complete ({} dry-run error(s))", dry_run_errors),
                    });

                    Ok(engine)
                }
                Err(e) => {
                    issues.push(LintIssue::error(
                        None,
                        ValidationPhase::Runtime,
                        LintCode::RuntimeInitError,
                        format!("Runtime initialization failed: {}", e),
                    ));
                    phase_trace.push(PhaseTrace {
                        phase: CompilationPhase::RuntimeInit,
                        ok: false,
                        detail: e.to_string(),
                    });
                    Err(format!("Runtime Init Error: {}", e))
                }
            }
        }
        Err(e) => {
            issues.push(LintIssue::error(
                None,
                ValidationPhase::Compile,
                LintCode::CompileError,
                format!("Compilation Error: {}", e),
            ));
            phase_trace.push(PhaseTrace {
                phase: CompilationPhase::ScriptCompile,
                ok: false,
                detail: e.to_string(),
            });
            Err(format!("Compilation Failed: {}", e))
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

mod parity;

use parity::{
    build_minimal_repro_script, check_preview_runtime_parity, enumerate_choice_routes, run_dry_run,
};

#[cfg(test)]
use parity::simulate_raw_sequence;

#[cfg(test)]
#[path = "tests/compiler_tests.rs"]
mod tests;
