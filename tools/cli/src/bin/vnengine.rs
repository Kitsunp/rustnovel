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

#[path = "vnengine/atomic_io.rs"]
mod atomic_io;
#[path = "vnengine/authoring.rs"]
mod authoring;
#[path = "vnengine/commands.rs"]
mod commands;
#[path = "vnengine/dispatch.rs"]
mod dispatch;
#[path = "vnengine/output.rs"]
mod output;
#[path = "vnengine/package.rs"]
mod package;

pub(crate) use atomic_io::atomic_write;
use commands::*;
use dispatch::run_command;
use output::{normalize_cli_path, print_json_envelope};

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
        /// Continue after an engine error and write a trace envelope marked as partial.
        #[arg(long, default_value_t = false)]
        allow_partial_trace: bool,
        /// Deprecated compatibility flag: trace is strict by default now.
        #[arg(long, hide = true, default_value_t = false)]
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
    partial: bool,
    stopped_reason: Option<String>,
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
