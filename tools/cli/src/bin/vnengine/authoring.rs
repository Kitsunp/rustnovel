use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use visual_novel_engine::authoring::{
    apply_authoring_document_command_headless, composer, load_authoring_document_or_script,
    source_looks_like_authoring_document, validate_authoring_graph_with_project_root,
    AuthoringCommand as CoreAuthoringCommand, AuthoringDocument, AuthoringDocumentCommand,
    AuthoringValidationReport, LintSeverity,
};
use visual_novel_engine::{run_repro_case, ReproCase};

#[path = "authoring/report.rs"]
mod report;
use report::{print_report, ReportCompareSummary};

#[derive(Subcommand)]
pub enum AuthoringCommand {
    /// Validate an authoring document or runtime script and emit report V2.
    Validate(ValidateArgs),
    /// Apply a document command JSON and emit the core outcome.
    ApplyCommand(ApplyCommandArgs),
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

#[derive(Args)]
pub struct ApplyCommandArgs {
    pub project: PathBuf,
    #[arg(long)]
    pub command: PathBuf,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub in_place: bool,
    #[arg(long, default_value_t = false)]
    pub json: bool,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Serialize)]
struct ApplyCommandReport {
    dry_run: bool,
    wrote: bool,
    output: Option<String>,
    outcome: visual_novel_engine::authoring::AuthoringDocumentCommandOutcome,
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
        AuthoringCommand::ApplyCommand(args) => run_apply_command(args),
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

fn run_apply_command(args: ApplyCommandArgs) -> Result<()> {
    let document = load_authoring_document(&args.project)?;
    let command = read_document_command(&args.command)?;
    let result =
        apply_authoring_document_command_headless(document, command).map_err(anyhow::Error::msg)?;
    let document = result.document;
    let outcome = result.outcome;
    let output_path = target_mutation_path(&args.project, args.output.as_deref(), args.in_place);
    if !args.dry_run {
        write_mutated_document(
            &args.project,
            args.output.as_deref(),
            args.in_place,
            &document,
        )?;
    }
    if args.json {
        if args.dry_run {
            let report = ApplyCommandReport {
                dry_run: true,
                wrote: false,
                output: output_path.map(|path| path.display().to_string()),
                outcome,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        }
    } else {
        println!(
            "{} {}{}",
            outcome.operation.operation_id,
            outcome.operation.operation_kind,
            if args.dry_run { " dry-run" } else { "" }
        );
    }
    Ok(())
}

fn explain_diagnostic(args: &ExplainArgs) -> Result<()> {
    let _document =
        load_authoring_document_or_script(&args.project).context("load authoring/script entry")?;
    let report = read_report(&args.report)?;
    let Some(issue) = report.explain(&args.diagnostic_id) else {
        anyhow::bail!("diagnostic '{}' not found", args.diagnostic_id);
    };
    println!("{}", serde_json::to_string_pretty(issue)?);
    Ok(())
}

fn read_document_command(path: &Path) -> Result<AuthoringDocumentCommand> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let command: CliDocumentCommand =
        serde_json::from_str(&source).context("parse document command json")?;
    Ok(command.into())
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
            apply_document_command(
                &mut document,
                CoreAuthoringCommand::CreateFragment {
                    fragment_id: args.id.clone(),
                    title: args.title.clone(),
                    node_ids: args.nodes.clone(),
                },
            )
            .with_context(|| format!("could not create fragment '{}'", args.id))?;
            write_mutated_document(
                &args.project,
                args.output.as_deref(),
                args.in_place,
                &document,
            )
        }
        FragmentCommand::Refresh(args) => {
            let mut document = load_authoring_document(&args.project)?;
            apply_document_command(
                &mut document,
                CoreAuthoringCommand::RefreshFragmentPorts {
                    fragment_id: args.id.clone(),
                },
            )
            .with_context(|| format!("could not refresh fragment '{}'", args.id))?;
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
            let sarif = report::sarif_from_report(&report);
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
    if source_looks_like_authoring_document(&source) {
        return AuthoringDocument::from_json(&source).with_context(|| {
            format!(
                "parse authoring document {} (legacy fallback disabled because authoring markers were found)",
                path.display()
            )
        });
    }
    match AuthoringDocument::from_json(&source) {
        Ok(document) => Ok(document),
        Err(err) => {
            let graph = load_authoring_document_or_script(path).with_context(|| {
                format!("load legacy runtime script after authoring parse failed ({err})")
            })?;
            Ok(AuthoringDocument::new(graph))
        }
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

fn target_mutation_path(input: &Path, output: Option<&Path>, in_place: bool) -> Option<PathBuf> {
    if in_place {
        Some(input.to_path_buf())
    } else {
        output.map(Path::to_path_buf)
    }
}

fn apply_document_command(
    document: &mut AuthoringDocument,
    command: CoreAuthoringCommand,
) -> Result<()> {
    let result = apply_authoring_document_command_headless(
        document.clone(),
        AuthoringDocumentCommand::Graph(command),
    )
    .map_err(anyhow::Error::msg)?;
    *document = result.document;
    Ok(())
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
    super::atomic_write(path, json.as_bytes())
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum CliDocumentCommand {
    SetLayerVisible {
        object_id: String,
        visible: bool,
    },
    SetLayerLocked {
        object_id: String,
        locked: bool,
    },
    SetBackgroundFitOverride {
        node_id: u32,
        fit: composer::BackgroundFit,
    },
    ClearBackgroundFitOverride {
        node_id: u32,
    },
}

impl From<CliDocumentCommand> for AuthoringDocumentCommand {
    fn from(command: CliDocumentCommand) -> Self {
        match command {
            CliDocumentCommand::SetLayerVisible { object_id, visible } => {
                Self::SetLayerVisible { object_id, visible }
            }
            CliDocumentCommand::SetLayerLocked { object_id, locked } => {
                Self::SetLayerLocked { object_id, locked }
            }
            CliDocumentCommand::SetBackgroundFitOverride { node_id, fit } => {
                Self::SetBackgroundFitOverride { node_id, fit }
            }
            CliDocumentCommand::ClearBackgroundFitOverride { node_id } => {
                Self::ClearBackgroundFitOverride { node_id }
            }
        }
    }
}
