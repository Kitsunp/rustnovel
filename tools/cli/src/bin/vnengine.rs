use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use sha2::{Digest, Sha256};
use visual_novel_engine::{
    compute_script_id, export_bundle, run_repro_case, BundleIntegrity, Engine, ExportBundleSpec,
    ExportTargetPlatform, ImportFallbackPolicy, ImportProfile, ReproCase, ResourceLimiter,
    SaveData, ScriptCompiled, ScriptRaw, SecurityPolicy, UiTrace, AUTH_SAVE_KEY,
    SCRIPT_SCHEMA_VERSION,
};
use vnengine_assets::{AssetEntry, AssetManifest};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about = "Visual Novel Engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a script JSON file.
    Validate { script: PathBuf },
    /// Compile a script JSON file into binary form.
    Compile {
        script: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Produce an execution trace for a script JSON file.
    Trace {
        script: PathBuf,
        #[arg(long, default_value_t = 100)]
        steps: usize,
        #[arg(short, long)]
        output: PathBuf,
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
    },
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate { script } => validate_script(&script),
        Command::Compile { script, output } => compile_script(&script, &output),
        Command::Trace {
            script,
            steps,
            output,
        } => trace_script(&script, steps, &output),
        Command::VerifySave { save, script } => verify_save(&save, &script),
        Command::Manifest { assets, output } => build_manifest(&assets, &output),
        Command::ReproRun {
            repro,
            output,
            strict,
        } => run_repro_bundle(&repro, output.as_deref(), strict),
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
        } => import_renpy(ImportRenpyCliOptions {
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
        }),
        Command::Package {
            project,
            output,
            target,
            entry_script,
            runtime_artifact,
            integrity,
            hmac_key,
            layout_version,
        } => package_project(ExportBundleSpec {
            project_root: project,
            output_root: output,
            target_platform: target.into(),
            entry_script,
            runtime_artifact,
            integrity: integrity.into(),
            output_layout_version: layout_version,
            hmac_key,
        }),
    }
}

fn validate_script(path: &Path) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let script = ScriptRaw::from_json(&raw).context("parse script")?;
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    policy.validate_raw(&script, limits)?;
    let compiled = script.compile()?;
    policy.validate_compiled(&compiled, limits)?;
    Ok(())
}

fn compile_script(path: &Path, output: &Path) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let script = ScriptRaw::from_json(&raw).context("parse script")?;
    let compiled = script.compile()?;
    let bytes = compiled.to_binary()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, bytes).with_context(|| format!("write {}", output.display()))?;
    Ok(())
}

fn trace_script(path: &Path, steps: usize, output: &Path) -> Result<()> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let script = ScriptRaw::from_json(&raw).context("parse script")?;
    let mut engine = Engine::new(
        script,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    let mut trace = UiTrace::new();
    for step in 0..steps {
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(_) => break,
        };
        let view = visual_novel_engine::TraceUiView::from_event(&event);
        let state = visual_novel_engine::StateDigest::from_state(
            engine.state(),
            engine.script().flag_count as usize,
        );
        trace.push(step as u32, view, state);
        match &event {
            visual_novel_engine::EventCompiled::Choice(_) => {
                let _ = engine.choose(0);
            }
            visual_novel_engine::EventCompiled::ExtCall { .. } => {
                let _ = engine.resume();
            }
            _ => {
                let _ = engine.step();
            }
        }
    }
    let envelope = TraceEnvelope {
        trace_format_version: 1,
        script_schema_version: SCRIPT_SCHEMA_VERSION.to_string(),
        trace,
    };
    let yaml = serde_yaml::to_string(&envelope)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, yaml).with_context(|| format!("write {}", output.display()))?;
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
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
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
        let bytes = fs::read(&canonical_path)
            .with_context(|| format!("read {}", canonical_path.display()))?;
        let size = bytes.len() as u64;
        let sha256 = sha256_hex(&bytes);
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
    fs::write(output, json).with_context(|| format!("write {}", output.display()))?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
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

struct ImportRenpyCliOptions<'a> {
    project: &'a Path,
    output: &'a Path,
    profile: ImportProfile,
    include_patterns: Vec<String>,
    exclude_patterns: Vec<String>,
    include_tl: Option<bool>,
    include_ui: Option<bool>,
    strict_mode: bool,
    fallback_policy: ImportFallbackPolicy,
    entry_label: &'a str,
    report: Option<&'a Path>,
}

fn import_renpy(options: ImportRenpyCliOptions<'_>) -> Result<()> {
    let report_result =
        visual_novel_engine::import_renpy_project(visual_novel_engine::ImportRenpyOptions {
            project_root: options.project.to_path_buf(),
            output_root: options.output.to_path_buf(),
            entry_label: options.entry_label.to_string(),
            report_path: options.report.map(Path::to_path_buf),
            profile: options.profile,
            include_tl: options.include_tl,
            include_ui: options.include_ui,
            include_patterns: options.include_patterns,
            exclude_patterns: options.exclude_patterns,
            strict_mode: options.strict_mode,
            fallback_policy: options.fallback_policy,
        })?;

    println!(
        "imported Ren'Py project => profile={} files={} events={} labels={} degraded={} issues={}",
        report_result.profile,
        report_result.files_parsed,
        report_result.events_generated,
        report_result.labels_generated,
        report_result.degraded_events,
        report_result.issues.len()
    );

    Ok(())
}

fn package_project(spec: ExportBundleSpec) -> Result<()> {
    let report = export_bundle(spec)?;

    println!(
        "packaged project => target={} assets={} integrity={} launcher={} report=meta/package_report.json",
        report.target_platform, report.assets_copied, report.integrity, report.launcher
    );
    if let Some(runtime) = report.runtime_artifact {
        println!("runtime_artifact={runtime}");
    }
    if let Some(signature) = report.bundle_hmac_sha256 {
        println!("bundle_hmac_sha256={signature}");
    }
    Ok(())
}
