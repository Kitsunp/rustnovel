use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use visual_novel_engine::authoring::{
    build_authoring_document_report_fingerprint, load_authoring_document_or_script,
    validate_authoring_graph_no_io, validate_authoring_graph_with_project_root, AuthoringDocument,
    AuthoringValidationReport, LintSeverity, NodeGraph, OperationKind, OperationLogEntry,
    VerificationRun,
};
use visual_novel_engine::{run_repro_case, ReproCase};

#[derive(Subcommand)]
pub enum AuthoringCommand {
    /// Validate an authoring document or runtime script and emit report V2.
    Validate(ValidateArgs),
    /// Explain one diagnostic from a report V2 JSON file.
    Explain(ExplainArgs),
    /// Manage graph fragments/subgraphs.
    Fragments {
        #[command(subcommand)]
        command: FragmentCommand,
    },
    /// Inspect persisted authoring operation logs.
    Operations {
        #[command(subcommand)]
        command: OperationCommand,
    },
    /// Work with authoring validation reports.
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Create and run repro cases from diagnostics.
    Repro {
        #[command(subcommand)]
        command: ReproCommand,
    },
}

#[derive(Args)]
pub struct ValidateArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExplainArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long)]
    pub diagnostic_id: String,
}

#[derive(Subcommand)]
pub enum FragmentCommand {
    List { project: PathBuf },
    Create(FragmentCreateArgs),
    Refresh(FragmentRefreshArgs),
    Validate { project: PathBuf },
}

#[derive(Args)]
pub struct FragmentCreateArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub title: String,
    #[arg(long, value_delimiter = ',')]
    pub nodes: Vec<u32>,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub in_place: bool,
}

#[derive(Args)]
pub struct FragmentRefreshArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub id: String,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub in_place: bool,
}

#[derive(Subcommand)]
pub enum OperationCommand {
    List {
        project: PathBuf,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ReportCommand {
    Compare {
        before: PathBuf,
        after: PathBuf,
    },
    Sarif {
        report: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum ReproCommand {
    FromDiagnostic(ReproFromDiagnosticArgs),
    Run {
        repro: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Args)]
pub struct ReproFromDiagnosticArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long)]
    pub diagnostic_id: String,
    #[arg(short, long)]
    pub output: PathBuf,
}

pub fn run_authoring_command(command: AuthoringCommand) -> Result<()> {
    match command {
        AuthoringCommand::Validate(args) => validate_authoring_script(
            &args.project,
            args.project_root.as_deref(),
            args.output.as_deref(),
        ),
        AuthoringCommand::Explain(args) => explain_diagnostic(&args),
        AuthoringCommand::Fragments { command } => run_fragment_command(command),
        AuthoringCommand::Operations { command } => run_operation_command(command),
        AuthoringCommand::Report { command } => run_report_command(command),
        AuthoringCommand::Repro { command } => run_repro_command(command),
    }
}

pub fn validate_authoring_script(
    path: &Path,
    project_root: Option<&Path>,
    output: Option<&Path>,
) -> Result<()> {
    let report = build_validation_report(path, project_root)?;

    if let Some(output) = output {
        write_report(output, &report)?;
    } else {
        print_report(&report);
    }

    if report.error_count > 0 {
        anyhow::bail!(
            "authoring validation failed with {} error(s)",
            report.error_count
        );
    }
    Ok(())
}

fn explain_diagnostic(args: &ExplainArgs) -> Result<()> {
    let _ =
        load_authoring_document_or_script(&args.project).context("load authoring/script entry")?;
    let report = read_report(&args.report)?;
    let Some(issue) = report.explain(&args.diagnostic_id) else {
        anyhow::bail!("diagnostic '{}' not found", args.diagnostic_id);
    };
    println!("{}", serde_json::to_string_pretty(issue)?);
    Ok(())
}

fn run_fragment_command(command: FragmentCommand) -> Result<()> {
    match command {
        FragmentCommand::List { project } => {
            let graph = load_authoring_document_or_script(&project).context("load project")?;
            println!("{}", serde_json::to_string_pretty(&graph.list_fragments())?);
            Ok(())
        }
        FragmentCommand::Create(args) => {
            let mut document = load_authoring_document(&args.project)?;
            let before_graph = document.graph.clone();
            if !document.graph.create_fragment(
                args.id.clone(),
                args.title.clone(),
                args.nodes.clone(),
            ) {
                anyhow::bail!("could not create fragment '{}'", args.id);
            }
            append_document_operation(
                &mut document,
                &before_graph,
                OperationKind::FragmentCreated,
                format!("Created fragment '{}'", args.id),
                Some(format!("graph.fragments[{}]", args.id)),
            );
            write_mutated_document(
                &args.project,
                args.output.as_deref(),
                args.in_place,
                &document,
            )
        }
        FragmentCommand::Refresh(args) => {
            let mut document = load_authoring_document(&args.project)?;
            let before_graph = document.graph.clone();
            if !document.graph.refresh_fragment_ports(&args.id) {
                anyhow::bail!("fragment '{}' not found", args.id);
            }
            append_document_operation(
                &mut document,
                &before_graph,
                OperationKind::FieldEdited,
                format!("Refreshed ports for fragment '{}'", args.id),
                Some(format!("graph.fragments[{}].ports", args.id)),
            );
            write_mutated_document(
                &args.project,
                args.output.as_deref(),
                args.in_place,
                &document,
            )
        }
        FragmentCommand::Validate { project } => {
            let graph = load_authoring_document_or_script(&project).context("load project")?;
            let issues = graph.validate_fragments();
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &issues
                        .iter()
                        .map(|issue| issue.envelope_v2())
                        .collect::<Vec<_>>()
                )?
            );
            if issues
                .iter()
                .any(|issue| issue.severity == LintSeverity::Error)
            {
                anyhow::bail!("fragment validation failed");
            }
            Ok(())
        }
    }
}

fn run_operation_command(command: OperationCommand) -> Result<()> {
    match command {
        OperationCommand::List { project, json } => {
            let document = load_authoring_document(&project)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&document.operation_log)?);
            } else {
                for entry in &document.operation_log {
                    println!(
                        "{} {} {}",
                        entry.operation_id, entry.created_unix_ms, entry.operation_kind
                    );
                }
            }
            Ok(())
        }
    }
}

fn run_report_command(command: ReportCommand) -> Result<()> {
    match command {
        ReportCommand::Compare { before, after } => {
            let before = read_report(&before)?;
            let after = read_report(&after)?;
            let summary = ReportCompareSummary {
                before_issue_count: before.issue_count,
                after_issue_count: after.issue_count,
                before_error_count: before.error_count,
                after_error_count: after.error_count,
                semantic_changed: before.fingerprints.story_semantic_sha256
                    != after.fingerprints.story_semantic_sha256,
                layout_changed: before.fingerprints.layout_sha256
                    != after.fingerprints.layout_sha256,
                assets_changed: before.fingerprints.assets_sha256
                    != after.fingerprints.assets_sha256,
            };
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        ReportCommand::Sarif { report, output } => {
            let report = read_report(&report)?;
            let sarif = sarif_from_report(&report);
            write_json(&output, &sarif)
        }
    }
}

fn run_repro_command(command: ReproCommand) -> Result<()> {
    match command {
        ReproCommand::FromDiagnostic(args) => {
            let graph = load_authoring_document_or_script(&args.project).context("load project")?;
            let report = read_report(&args.report)?;
            let Some(issue) = report.explain(&args.diagnostic_id) else {
                anyhow::bail!("diagnostic '{}' not found", args.diagnostic_id);
            };
            let operation_id = issue
                .operation_id
                .clone()
                .or_else(|| issue.message_args.get("operation_id").cloned())
                .or_else(|| issue.typed_message_args.get("operation_id").cloned())
                .unwrap_or_else(|| "operation:unknown".to_string());
            let case = ReproCase::new(
                format!("diagnostic {}", args.diagnostic_id),
                graph.to_script_lossy_for_diagnostics(),
            )
            .with_diagnostic_context(
                args.diagnostic_id,
                report.fingerprints.story_semantic_sha256,
                operation_id,
            );
            write_json(&args.output, &case)
        }
        ReproCommand::Run { repro, output } => {
            let raw = std::fs::read_to_string(&repro)
                .with_context(|| format!("read {}", repro.display()))?;
            let case = ReproCase::from_json(&raw).context("parse repro")?;
            let result = run_repro_case(&case);
            if let Some(output) = output {
                write_json(&output, &result)?;
            } else {
                println!("{}", result.to_json()?);
            }
            Ok(())
        }
    }
}

fn build_validation_report(
    path: &Path,
    project_root: Option<&Path>,
) -> Result<AuthoringValidationReport> {
    let document = load_authoring_document(path).context("load authoring/script entry")?;
    let script = document.graph.to_script_lossy_for_diagnostics();
    let project_root = project_root
        .or_else(|| path.parent())
        .unwrap_or_else(|| Path::new("."));
    let issues = validate_authoring_graph_with_project_root(&document.graph, project_root);
    Ok(AuthoringValidationReport::from_document_and_issues(
        &document, &script, &issues,
    ))
}

fn load_authoring_document(path: &Path) -> Result<AuthoringDocument> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    match AuthoringDocument::from_json(&source) {
        Ok(document) => Ok(document),
        Err(_) => Ok(AuthoringDocument::new(load_authoring_document_or_script(
            path,
        )?)),
    }
}

fn write_mutated_document(
    input: &Path,
    output: Option<&Path>,
    in_place: bool,
    document: &AuthoringDocument,
) -> Result<()> {
    let path = if in_place {
        input
    } else {
        output.ok_or_else(|| anyhow::anyhow!("mutating command requires --output or --in-place"))?
    };
    write_json(path, document)
}

fn append_document_operation(
    document: &mut AuthoringDocument,
    before_graph: &NodeGraph,
    operation_kind: OperationKind,
    details: impl Into<String>,
    field_path: Option<String>,
) {
    let before_script = before_graph.to_script_lossy_for_diagnostics();
    let after_script = document.graph.to_script_lossy_for_diagnostics();
    let mut before_document = document.clone();
    before_document.graph = before_graph.clone();
    let before_fingerprint =
        build_authoring_document_report_fingerprint(&before_document, &before_script);
    let after_fingerprint = build_authoring_document_report_fingerprint(document, &after_script);
    let before_issues = validate_authoring_graph_no_io(before_graph);
    let after_issues = validate_authoring_graph_no_io(&document.graph);
    let mut entry = OperationLogEntry::new_typed(operation_kind, "applied", details)
        .with_before_after_fingerprints(&before_fingerprint, &after_fingerprint);
    if let Some(field_path) = field_path {
        entry = entry.with_field_path(field_path);
    }
    let verification = VerificationRun::from_diagnostics(
        entry.operation_id.clone(),
        "cli_authoring_no_io",
        &after_fingerprint,
        &before_issues,
        &after_issues,
    );
    document.operation_log.push(entry);
    document.verification_runs.push(verification);
}

fn write_report(output: &Path, report: &AuthoringValidationReport) -> Result<()> {
    write_json(output, report)
}

fn read_report(path: &Path) -> Result<AuthoringValidationReport> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    AuthoringValidationReport::from_json(&source).context("parse authoring report")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    std::fs::write(path, json).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn print_report(report: &AuthoringValidationReport) {
    println!(
        "authoring validation => issues={} errors={} warnings={} infos={}",
        report.issue_count, report.error_count, report.warning_count, report.info_count
    );
    for issue in &report.issues {
        println!(
            "{} [{}:{}] {}",
            issue.diagnostic_id, issue.severity, issue.code, issue.text_en.actual
        );
    }
}

#[derive(Serialize)]
struct ReportCompareSummary {
    before_issue_count: usize,
    after_issue_count: usize,
    before_error_count: usize,
    after_error_count: usize,
    semantic_changed: bool,
    layout_changed: bool,
    assets_changed: bool,
}

fn sarif_from_report(report: &AuthoringValidationReport) -> serde_json::Value {
    let results = report
        .issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "ruleId": issue.code,
                "level": sarif_level(&issue.severity),
                "message": { "text": issue.text_en.actual },
                "partialFingerprints": {
                    "diagnosticId": issue.diagnostic_id,
                    "traceId": issue.trace_id,
                    "storySemanticSha256": report.fingerprints.story_semantic_sha256,
                },
                "properties": {
                    "target": issue.target,
                    "fieldPath": issue.field_path,
                    "typedMessageArgs": issue.typed_message_args,
                    "evidenceTrace": issue.evidence_trace,
                }
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "vnengine authoring",
                    "informationUri": "https://github.com/Kitsunp/rustnovel"
                }
            },
            "results": results
        }]
    })
}

fn sarif_level(severity: &str) -> &'static str {
    match severity.to_ascii_lowercase().as_str() {
        "error" => "error",
        "warning" => "warning",
        _ => "note",
    }
}
