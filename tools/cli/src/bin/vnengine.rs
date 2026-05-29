use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use sha2::{Digest, Sha256};
use visual_novel_engine::{
    compute_script_id, load_runtime_script_from_entry, resolve_layout, run_repro_case,
    runtime::{Engine, ScriptCompiled, UiTrace},
    validate_ui_theme, BundleIntegrity, ExportBundleSpec, ExportService, ExportTargetPlatform,
    ImportFallbackPolicy, ImportProfile, LayoutPolicy, ReproCase, ResourceLimiter, SaveData,
    SecurityPolicy, StageProfile, UiTheme, AUTH_SAVE_KEY, SCRIPT_SCHEMA_VERSION,
};
use vnengine_assets::{AssetEntry, AssetManifest};
use walkdir::WalkDir;

#[path = "vnengine/authoring.rs"]
mod authoring;
#[path = "vnengine/package.rs"]
mod package;

#[derive(Parser)]
#[command(author, version, about = "Visual Novel Engine CLI")]
struct Cli {
    /// Emit a stable JSON envelope on stdout.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a script JSON file.
    Validate { script: PathBuf },
    /// Validate editor/authoring graph semantics from a script JSON file.
    AuthoringValidate {
        script: PathBuf,
        #[arg(long)]
        project_root: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Authoring workflows: reports, fragments, operations and repros.
    Authoring {
        #[command(subcommand)]
        command: authoring::AuthoringCommand,
    },
    /// Compile a script JSON file into binary form.
    Compile {
        script: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Migrate a legacy script JSON to the current schema.
    MigrateScript {
        script: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    /// Produce an execution trace for a script JSON file.
    Trace {
        script: PathBuf,
        #[arg(long, default_value_t = 100)]
        steps: usize,
        #[arg(long, value_enum)]
        format: Option<TraceFormat>,
        #[arg(short, long)]
        output: PathBuf,
        /// Return a failing exit code when engine step/choose/resume fails.
        #[arg(long, default_value_t = false)]
        fail_on_engine_error: bool,
    },
    /// Print the native route tree for a script.
    RouteTree { script: PathBuf },
    /// Print read/progress snapshots from a script or save.
    ReadModel { input: PathBuf },
    /// Theme tooling.
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
    /// Resolve a display/stage/theme layout.
    Layout {
        #[command(subcommand)]
        command: LayoutCommand,
    },
    /// Common export service commands.
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    /// Verify a save file against a compiled script.
    VerifySave {
        save: PathBuf,
        #[arg(long)]
        script: PathBuf,
    },
    /// Build an asset manifest with sha256 hashes.
    Manifest {
        assets: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Run a local repro-case JSON and evaluate its oracle/monitors.
    ReproRun {
        repro: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        strict: bool,
    },
    /// Import a Ren'Py project into vnengine project files.
    ImportRenpy {
        /// Ren'Py project folder path.
        project: PathBuf,
        /// Output folder to write project.vnm/main.json/report.
        #[arg(short, long)]
        output: PathBuf,
        /// Import profile. `story-first` keeps the engine model as source of truth.
        #[arg(long, value_enum, default_value_t = ImportProfileArg::StoryFirst)]
        profile: ImportProfileArg,
        /// Include paths matching this pattern (repeatable).
        #[arg(long = "include-pattern")]
        include_pattern: Vec<String>,
        /// Exclude paths matching this pattern (repeatable).
        #[arg(long = "exclude-pattern")]
        exclude_pattern: Vec<String>,
        /// Include `game/tl/**` files even in story-first mode.
        #[arg(long)]
        include_tl: bool,
        /// Include UI DSL files (`gui.rpy`, `screens.rpy`, `options.rpy`) even in story-first mode.
        #[arg(long)]
        include_ui: bool,
        /// Fail import when unsupported/degraded constructs are found.
        #[arg(long)]
        strict_mode: bool,
        /// Fallback policy for unsupported statements.
        #[arg(long, value_enum, default_value_t = ImportFallbackArg::DegradeWithTrace)]
        fallback_policy: ImportFallbackArg,
        /// Entry label to map as `start` in generated script.
        #[arg(long, default_value = "start")]
        entry_label: String,
        /// Optional custom report path.
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Package a project into a reproducible bundle layout.
    Package {
        /// Project root containing `project.vnm` and entry script.
        project: PathBuf,
        /// Output folder for bundle artifacts.
        #[arg(short, long)]
        output: PathBuf,
        /// Target platform profile.
        #[arg(long, value_enum, default_value_t = PackageTargetArg::Windows)]
        target: PackageTargetArg,
        /// Optional entry script path relative to project root.
        #[arg(long)]
        entry_script: Option<PathBuf>,
        /// Optional runtime artifact (absolute or project-relative) to embed in bundle.
        #[arg(long)]
        runtime_artifact: Option<PathBuf>,
        /// Bundle integrity mode.
        #[arg(long, value_enum, default_value_t = PackageIntegrityArg::None)]
        integrity: PackageIntegrityArg,
        /// HMAC key when `--integrity hmac-sha256`.
        #[arg(long)]
        hmac_key: Option<String>,
        /// Output layout version stamped in report.
        #[arg(long, default_value_t = 1)]
        layout_version: u16,
        /// Fail unless the bundle produces a top-level native executable (game.exe on Windows, game on Linux/macOS).
        #[arg(long)]
        require_executable: bool,
        /// Build and validate the export plan without writing bundle artifacts.
        #[arg(long)]
        dry_run: bool,
        /// Alias for --dry-run that emphasizes the planned JSON/report output.
        #[arg(long)]
        plan: bool,
        /// Explicitly execute the package operation (default when no plan/dry-run flag is set).
        #[arg(long)]
        execute: bool,
    },
}

#[derive(Subcommand)]
enum ThemeCommand {
    /// Validate a UiTheme JSON file.
    Validate { theme: PathBuf },
}

#[derive(Subcommand)]
enum LayoutCommand {
    /// Resolve layout JSON from display/stage/policy files.
    Resolve {
        #[arg(long)]
        display: PathBuf,
        #[arg(long)]
        stage: Option<PathBuf>,
        #[arg(long)]
        policy: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum ExportCommand {
    /// Build and validate an export plan without writing bundle artifacts.
    Plan(ExportCliArgs),
    /// Execute an export plan/spec through the shared ExportService.
    Execute(ExportCliArgs),
}

#[derive(clap::Args)]
struct ExportCliArgs {
    /// Project root containing `project.vnm` and entry script.
    project: PathBuf,
    /// Output folder for bundle artifacts.
    #[arg(short, long)]
    output: PathBuf,
    /// Target platform profile.
    #[arg(long, value_enum, default_value_t = PackageTargetArg::Windows)]
    target: PackageTargetArg,
    /// Optional entry script path relative to project root.
    #[arg(long)]
    entry_script: Option<PathBuf>,
    /// Optional runtime artifact (absolute or project-relative) to embed in bundle.
    #[arg(long)]
    runtime_artifact: Option<PathBuf>,
    /// Bundle integrity mode.
    #[arg(long, value_enum, default_value_t = PackageIntegrityArg::None)]
    integrity: PackageIntegrityArg,
    /// HMAC key when `--integrity hmac-sha256`.
    #[arg(long)]
    hmac_key: Option<String>,
    /// Output layout version stamped in report.
    #[arg(long, default_value_t = 1)]
    layout_version: u16,
    /// Fail unless the bundle produces a top-level native executable.
    #[arg(long)]
    require_executable: bool,
}

#[derive(Serialize)]
struct CliEnvelope {
    ok: bool,
    code: String,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TraceFormat {
    Json,
    Yaml,
}

impl TraceFormat {
    fn inferred_from_output(output: &Path) -> Self {
        match output
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("yaml" | "yml") => Self::Yaml,
            _ => Self::Json,
        }
    }

    fn matches_output_extension(self, output: &Path) -> bool {
        match output
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("json") => self == Self::Json,
            Some("yaml" | "yml") => self == Self::Yaml,
            _ => true,
        }
    }
}

#[derive(Serialize)]
struct TraceEnvelope {
    trace_format_version: u16,
    script_schema_version: String,
    trace: UiTrace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ImportProfileArg {
    StoryFirst,
    Full,
    Custom,
}

impl From<ImportProfileArg> for ImportProfile {
    fn from(value: ImportProfileArg) -> Self {
        match value {
            ImportProfileArg::StoryFirst => ImportProfile::StoryFirst,
            ImportProfileArg::Full => ImportProfile::Full,
            ImportProfileArg::Custom => ImportProfile::Custom,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ImportFallbackArg {
    Strict,
    DegradeWithTrace,
}

impl From<ImportFallbackArg> for ImportFallbackPolicy {
    fn from(value: ImportFallbackArg) -> Self {
        match value {
            ImportFallbackArg::Strict => ImportFallbackPolicy::Strict,
            ImportFallbackArg::DegradeWithTrace => ImportFallbackPolicy::DegradeWithTrace,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PackageTargetArg {
    Windows,
    Linux,
    Macos,
}

impl From<PackageTargetArg> for ExportTargetPlatform {
    fn from(value: PackageTargetArg) -> Self {
        match value {
            PackageTargetArg::Windows => ExportTargetPlatform::Windows,
            PackageTargetArg::Linux => ExportTargetPlatform::Linux,
            PackageTargetArg::Macos => ExportTargetPlatform::Macos,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PackageIntegrityArg {
    None,
    HmacSha256,
}

impl From<PackageIntegrityArg> for BundleIntegrity {
    fn from(value: PackageIntegrityArg) -> Self {
        match value {
            PackageIntegrityArg::None => BundleIntegrity::None,
            PackageIntegrityArg::HmacSha256 => BundleIntegrity::HmacSha256,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    let suppress_envelope = matches!(&cli.command, Command::Authoring { .. });
    match run_command(cli, json) {
        Ok(data) => {
            if json && !suppress_envelope {
                print_json_envelope(CliEnvelope {
                    ok: true,
                    code: "ok".to_string(),
                    data,
                    error: None,
                });
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            let (code, exit_code) = classify_error(&err);
            if json && !suppress_envelope {
                print_json_envelope(CliEnvelope {
                    ok: false,
                    code: code.to_string(),
                    data: None,
                    error: Some(err.to_string()),
                });
            } else {
                eprintln!("{err:#}");
            }
            ExitCode::from(exit_code)
        }
    }
}

fn classify_error(err: &anyhow::Error) -> (&'static str, u8) {
    if err.chain().any(|cause| {
        cause
            .downcast_ref::<visual_novel_engine::VnError>()
            .is_some()
    }) {
        ("engine_error", 2)
    } else if err
        .chain()
        .any(|cause| cause.downcast_ref::<std::io::Error>().is_some())
    {
        ("io_error", 3)
    } else {
        ("error", 1)
    }
}

fn run_command(cli: Cli, json_output: bool) -> Result<Option<serde_json::Value>> {
    match cli.command {
        Command::Validate { script } => {
            validate_script(&script)?;
            Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.validate.v1",
                "script": normalize_cli_path(&script),
                "valid": true
            })))
        }
        Command::AuthoringValidate {
            script,
            project_root,
            output,
        } => {
            authoring::validate_authoring_script(
                &script,
                project_root.as_deref(),
                output.as_deref(),
            )?;
            Ok(None)
        }
        Command::Authoring { command } => {
            authoring::run_authoring_command(command)?;
            Ok(None)
        }
        Command::Compile { script, output } => {
            let report = compile_script(&script, &output)?;
            Ok(Some(report))
        }
        Command::MigrateScript {
            script,
            output,
            in_place,
        } => migrate_script_command(&script, output.as_deref(), in_place),
        Command::Trace {
            script,
            steps,
            format,
            output,
            fail_on_engine_error,
        } => {
            trace_script(&script, steps, format, &output, fail_on_engine_error)?;
            Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.trace.v1",
                "script": normalize_cli_path(&script),
                "output": normalize_cli_path(&output),
                "steps_requested": steps,
                "fail_on_engine_error": fail_on_engine_error
            })))
        }
        Command::RouteTree { script } => route_tree_command(&script),
        Command::ReadModel { input } => read_model_command(&input),
        Command::Theme { command } => match command {
            ThemeCommand::Validate { theme } => theme_validate_command(&theme),
        },
        Command::Layout { command } => match command {
            LayoutCommand::Resolve {
                display,
                stage,
                policy,
            } => layout_resolve_command(&display, stage.as_deref(), policy.as_deref()),
        },
        Command::Export { command } => match command {
            ExportCommand::Plan(args) => export_plan_command(args),
            ExportCommand::Execute(args) => export_execute_command(args),
        },
        Command::VerifySave { save, script } => {
            verify_save(&save, &script)?;
            Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.verify_save.v1",
                "save": normalize_cli_path(&save),
                "script": normalize_cli_path(&script),
                "valid": true
            })))
        }
        Command::Manifest { assets, output } => {
            build_manifest(&assets, &output)?;
            Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.manifest.v1",
                "assets": normalize_cli_path(&assets),
                "output": normalize_cli_path(&output)
            })))
        }
        Command::ReproRun {
            repro,
            output,
            strict,
        } => {
            run_repro_bundle(&repro, output.as_deref(), strict)?;
            Ok(None)
        }
        Command::ImportRenpy {
            project,
            output,
            profile,
            include_pattern,
            exclude_pattern,
            include_tl,
            include_ui,
            strict_mode,
            fallback_policy,
            entry_label,
            report,
        } => {
            package::import_renpy(package::ImportRenpyCliOptions {
                project: &project,
                output: &output,
                profile: profile.into(),
                include_patterns: include_pattern,
                exclude_patterns: exclude_pattern,
                include_tl: include_tl.then_some(true),
                include_ui: include_ui.then_some(true),
                strict_mode,
                fallback_policy: fallback_policy.into(),
                entry_label: &entry_label,
                report: report.as_deref(),
            })?;
            Ok(None)
        }
        Command::Package {
            project,
            output,
            target,
            entry_script,
            runtime_artifact,
            integrity,
            hmac_key,
            layout_version,
            require_executable,
            dry_run,
            plan,
            execute,
        } => {
            let mode_count = [dry_run, plan, execute]
                .into_iter()
                .filter(|flag| *flag)
                .count();
            if mode_count > 1 {
                anyhow::bail!("package accepts only one of --dry-run, --plan or --execute");
            }
            let spec = ExportBundleSpec {
                project_root: project,
                output_root: output,
                target_platform: target.into(),
                entry_script,
                runtime_artifact,
                integrity: integrity.into(),
                output_layout_version: layout_version,
                hmac_key,
            };
            if dry_run || plan {
                let plan = ExportService::new().plan_export(&spec)?;
                ExportService::new().validate_export_plan(&plan)?;
                if require_executable {
                    let expected = spec.target_platform.expected_executable_name();
                    if plan.executable.as_deref() != Some(expected) {
                        anyhow::bail!(
                            "{} executable export requires runtime_artifact and must produce {}",
                            spec.target_platform.as_str(),
                            expected
                        );
                    }
                }
                if json_output {
                    Ok(Some(serde_json::to_value(plan)?))
                } else {
                    println!(
                        "package plan => target={} layout={} files={} integrity={} executable={}",
                        plan.target_platform,
                        plan.output_layout_version,
                        plan.layout.len(),
                        plan.integrity,
                        plan.executable.as_deref().unwrap_or("none")
                    );
                    Ok(None)
                }
            } else if json_output {
                let report = if require_executable {
                    visual_novel_engine::export_executable_bundle(spec)?
                } else {
                    ExportService::new().execute_export(spec)?
                };
                Ok(Some(serde_json::to_value(report)?))
            } else {
                package::package_project(spec, require_executable)?;
                Ok(None)
            }
        }
    }
}

fn print_json_envelope(envelope: CliEnvelope) {
    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            println!(
                "{{\"ok\":false,\"code\":\"envelope_serialization_error\",\"data\":null,\"error\":\"{}\"}}",
                err
            );
        }
    }
}

fn normalize_cli_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output parent {}", parent.display()))?;
    }
    let temp_path = unique_sibling_path(path, "tmp");
    let mut temp_file = fs::File::create(&temp_path)
        .with_context(|| format!("create temp output {}", temp_path.display()))?;
    temp_file
        .write_all(content)
        .with_context(|| format!("write temp output {}", temp_path.display()))?;
    temp_file
        .sync_all()
        .with_context(|| format!("sync temp output {}", temp_path.display()))?;
    drop(temp_file);

    if path.exists() {
        let rollback_path = unique_sibling_path(path, "rollback");
        fs::rename(path, &rollback_path).with_context(|| {
            format!(
                "prepare rollback {} -> {}",
                path.display(),
                rollback_path.display()
            )
        })?;
        match fs::rename(&temp_path, path) {
            Ok(()) => {
                if let Err(err) = remove_file_with_diagnostic(
                    &rollback_path,
                    "remove rollback after atomic write",
                ) {
                    eprintln!("{err:#}");
                }
            }
            Err(err) => {
                let restore_result = fs::rename(&rollback_path, path).with_context(|| {
                    format!(
                        "restore rollback {} -> {} after publish failure",
                        rollback_path.display(),
                        path.display()
                    )
                });
                cleanup_temp_with_diagnostic(&temp_path);
                restore_result?;
                return Err(err).with_context(|| {
                    format!(
                        "publish temp output {} -> {}",
                        temp_path.display(),
                        path.display()
                    )
                });
            }
        }
    } else if let Err(err) = fs::rename(&temp_path, path) {
        cleanup_temp_with_diagnostic(&temp_path);
        return Err(err).with_context(|| {
            format!(
                "publish temp output {} -> {}",
                temp_path.display(),
                path.display()
            )
        });
    }

    Ok(())
}

fn unique_sibling_path(path: &Path, role: &str) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    parent.join(format!(".{name}.{role}-{}-{nanos}", process::id()))
}

fn remove_file_with_diagnostic(path: &Path, context: &str) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("{context}: {}", path.display())),
    }
}

fn cleanup_temp_with_diagnostic(path: &Path) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => eprintln!("failed to remove temp output {}: {err}", path.display()),
    }
}

fn route_tree_command(script: &Path) -> Result<Option<serde_json::Value>> {
    let script_raw = load_runtime_script_from_entry(script).context("load script")?;
    let engine = Engine::new(
        script_raw,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    Ok(Some(serde_json::to_value(engine.route_tree())?))
}

fn read_model_command(input: &Path) -> Result<Option<serde_json::Value>> {
    if let Ok(bytes) = fs::read(input) {
        if let Ok(save) = SaveData::from_any_binary(&bytes, AUTH_SAVE_KEY) {
            return Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.read_model.v1",
                "source": "save",
                "read_model": save.state.read_model,
                "route_progress": save.state.route_progress
            })));
        }
    }

    let script_raw = load_runtime_script_from_entry(input).context("load script or save")?;
    let max_steps = script_raw.events.len().saturating_mul(4).saturating_add(32);
    let mut engine = Engine::new(
        script_raw,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    for _ in 0..max_steps {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(visual_novel_engine::VnError::EndOfScript) => break,
            Err(err) => return Err(err).context("read-model current_event"),
        };
        match event {
            visual_novel_engine::runtime::EventCompiled::Choice(choice) => {
                if choice.options.is_empty() {
                    break;
                }
                engine.choose(0)?;
            }
            visual_novel_engine::runtime::EventCompiled::ExtCall { .. } => engine.resume()?,
            _ => {
                let (_audio, _change) = engine.step()?;
            }
        }
    }
    Ok(Some(serde_json::json!({
        "schema": "vnengine.cli.read_model.v1",
        "source": "script",
        "read_model": engine.read_model_snapshot(),
        "route_progress": engine.route_progress_snapshot()
    })))
}

fn theme_validate_command(theme_path: &Path) -> Result<Option<serde_json::Value>> {
    let raw =
        fs::read_to_string(theme_path).with_context(|| format!("read {}", theme_path.display()))?;
    let theme: UiTheme = serde_json::from_str(&raw)
        .with_context(|| format!("parse theme {}", theme_path.display()))?;
    let report = validate_ui_theme(&theme);
    Ok(Some(serde_json::to_value(report)?))
}

fn layout_resolve_command(
    display: &Path,
    stage: Option<&Path>,
    policy: Option<&Path>,
) -> Result<Option<serde_json::Value>> {
    let display_raw =
        fs::read_to_string(display).with_context(|| format!("read {}", display.display()))?;
    let display = serde_json::from_str(&display_raw)
        .with_context(|| format!("parse display {}", display.display()))?;
    let stage = match stage {
        Some(path) => {
            let raw =
                fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
            serde_json::from_str(&raw).with_context(|| format!("parse stage {}", path.display()))?
        }
        None => StageProfile::default(),
    };
    let policy = match policy {
        Some(path) => {
            let raw =
                fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("parse policy {}", path.display()))?
        }
        None => LayoutPolicy::default(),
    };
    Ok(Some(serde_json::to_value(resolve_layout(
        display, stage, policy,
    ))?))
}

fn export_plan_command(args: ExportCliArgs) -> Result<Option<serde_json::Value>> {
    let spec = export_spec_from_args(args);
    let plan = ExportService::new().plan_export(&spec)?;
    Ok(Some(serde_json::to_value(plan)?))
}

fn export_execute_command(args: ExportCliArgs) -> Result<Option<serde_json::Value>> {
    let require_executable = args.require_executable;
    let spec = export_spec_from_args(args);
    let report = if require_executable {
        visual_novel_engine::export_executable_bundle(spec)?
    } else {
        ExportService::new().execute_export(spec)?
    };
    Ok(Some(serde_json::to_value(report)?))
}

fn export_spec_from_args(args: ExportCliArgs) -> ExportBundleSpec {
    ExportBundleSpec {
        project_root: args.project,
        output_root: args.output,
        target_platform: args.target.into(),
        entry_script: args.entry_script,
        runtime_artifact: args.runtime_artifact,
        integrity: args.integrity.into(),
        output_layout_version: args.layout_version,
        hmac_key: args.hmac_key,
    }
}

fn validate_script(path: &Path) -> Result<()> {
    let script = load_runtime_script_from_entry(path).context("load script")?;
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    policy.validate_raw(&script, limits)?;
    let compiled = script.compile()?;
    policy.validate_compiled(&compiled, limits)?;
    Ok(())
}

fn migrate_script_command(
    script: &Path,
    output: Option<&Path>,
    in_place: bool,
) -> Result<Option<serde_json::Value>> {
    if !in_place && output.is_none() {
        anyhow::bail!("migrate-script requires --output or --in-place");
    }
    let source =
        fs::read_to_string(script).with_context(|| format!("read {}", script.display()))?;
    let (migrated, report) =
        visual_novel_engine::migrate_script_json_to_current(&source).context("migrate script")?;
    let output_path = if in_place {
        script
    } else {
        output.ok_or_else(|| anyhow::anyhow!("migrate-script requires --output or --in-place"))?
    };
    atomic_write(output_path, migrated.as_bytes())?;
    Ok(Some(serde_json::json!({
        "schema": "vnengine.cli.migrate_script.v1",
        "input": normalize_cli_path(script),
        "output": normalize_cli_path(output_path),
        "changed": report.changed(),
        "migration": report
    })))
}

fn compile_script(path: &Path, output: &Path) -> Result<serde_json::Value> {
    let script = load_runtime_script_from_entry(path).context("load script")?;
    let compiled = script.compile()?;
    let bytes = compiled.to_binary()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(output, &bytes)?;
    Ok(serde_json::json!({
        "schema": "vnengine.cli.compile.v1",
        "script": normalize_cli_path(path),
        "output": normalize_cli_path(output),
        "bytes_written": fs::metadata(output).map(|metadata| metadata.len()).unwrap_or(0)
    }))
}

fn trace_script(
    path: &Path,
    steps: usize,
    requested_format: Option<TraceFormat>,
    output: &Path,
    fail_on_engine_error: bool,
) -> Result<()> {
    let format = requested_format.unwrap_or_else(|| TraceFormat::inferred_from_output(output));
    if !format.matches_output_extension(output) {
        anyhow::bail!(
            "trace output extension is incompatible with --format {format:?}: {}",
            output.display()
        );
    }
    let script = load_runtime_script_from_entry(path).context("load script")?;
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    let mut trace = UiTrace::new();
    for step in 0..steps {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(err) => {
                if fail_on_engine_error {
                    return Err(err).context("trace current_event");
                }
                eprintln!("trace stopped at step {step}: current_event failed: {err}");
                break;
            }
        };
        let view = visual_novel_engine::runtime::TraceUiView::from_event(&event);
        let state = visual_novel_engine::runtime::StateDigest::from_state(
            engine.state(),
            engine.script().flag_count as usize,
        );
        trace.push(step as u32, view, state);
        let advance_result = match &event {
            visual_novel_engine::runtime::EventCompiled::Choice(_) => engine.choose(0).map(|_| ()),
            visual_novel_engine::runtime::EventCompiled::ExtCall { .. } => engine.resume(),
            _ => engine.step().map(|_| ()),
        };
        if let Err(err) = advance_result {
            if fail_on_engine_error {
                return Err(err).with_context(|| format!("trace advance step {step}"));
            }
            eprintln!("trace stopped at step {step}: engine advance failed: {err}");
            break;
        }
    }
    let envelope = TraceEnvelope {
        trace_format_version: 1,
        script_schema_version: SCRIPT_SCHEMA_VERSION.to_string(),
        trace,
    };
    let content = match format {
        TraceFormat::Json => serde_json::to_string_pretty(&envelope)?,
        TraceFormat::Yaml => serde_norway::to_string(&envelope)?,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(output, content.as_bytes())?;
    Ok(())
}

fn verify_save(save_path: &Path, script_path: &Path) -> Result<()> {
    let save_bytes =
        fs::read(save_path).with_context(|| format!("read {}", save_path.display()))?;
    let save = SaveData::from_any_binary(&save_bytes, AUTH_SAVE_KEY)?;
    let script_bytes =
        fs::read(script_path).with_context(|| format!("read {}", script_path.display()))?;
    let compiled = ScriptCompiled::from_binary(&script_bytes)?;
    let compiled_bytes = compiled.to_binary()?;
    let script_id = compute_script_id(&compiled_bytes);
    save.validate_script_id(&script_id)?;
    Ok(())
}

fn build_manifest(root: &Path, output: &Path) -> Result<()> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))?;
    let mut assets = std::collections::BTreeMap::new();
    for entry in WalkDir::new(root) {
        let entry = entry.with_context(|| format!("walk {}", root.display()))?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let canonical_path = path
            .canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))?;
        if !canonical_path.starts_with(&canonical_root) {
            anyhow::bail!("manifest asset escapes root: {}", path.display());
        }
        let (sha256, size) = sha256_file_hex(&canonical_path)?;
        assets.insert(rel_str, AssetEntry { sha256, size });
    }
    let manifest = AssetManifest {
        manifest_version: 1,
        assets,
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(output, json.as_bytes())?;
    Ok(())
}

fn sha256_file_hex(path: &Path) -> Result<(String, u64)> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        size = size.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok((digest_hex(&digest), size))
}

fn digest_hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn run_repro_bundle(path: &Path, output: Option<&Path>, strict: bool) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let case = ReproCase::from_json(&raw).context("parse repro case")?;
    let report = run_repro_case(&case);

    println!(
        "repro '{}' => stop_reason={} oracle_triggered={} matched_monitors={}",
        case.title,
        report.stop_reason.label(),
        report.oracle_triggered,
        report.matched_monitors.join(",")
    );
    if let Some(event_ip) = report.failing_event_ip {
        println!("failing_event_ip={event_ip}");
    }
    println!("stop_message={}", report.stop_message);

    if let Some(out) = output {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = report.to_json().context("serialize repro report")?;
        fs::write(out, payload).with_context(|| format!("write {}", out.display()))?;
    }

    if strict && !report.oracle_triggered {
        anyhow::bail!("repro oracle was not triggered");
    }
    Ok(())
}
