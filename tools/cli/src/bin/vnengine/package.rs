use std::path::Path;

use anyhow::Result;
use visual_novel_engine::{
    export_bundle, ExportBundleSpec, ImportFallbackPolicy, ImportProfile, ImportRenpyOptions,
};

pub(super) struct ImportRenpyCliOptions<'a> {
    pub(super) project: &'a Path,
    pub(super) output: &'a Path,
    pub(super) profile: ImportProfile,
    pub(super) include_patterns: Vec<String>,
    pub(super) exclude_patterns: Vec<String>,
    pub(super) include_tl: Option<bool>,
    pub(super) include_ui: Option<bool>,
    pub(super) strict_mode: bool,
    pub(super) fallback_policy: ImportFallbackPolicy,
    pub(super) entry_label: &'a str,
    pub(super) report: Option<&'a Path>,
}

pub(super) fn import_renpy(options: ImportRenpyCliOptions<'_>) -> Result<()> {
    let report_result = visual_novel_engine::import_renpy_project(ImportRenpyOptions {
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

pub(super) fn package_project(spec: ExportBundleSpec) -> Result<()> {
    let report = export_bundle(spec)?;

    println!(
        "packaged project => target={} assets={} integrity={} launcher={} report=meta/package_report.json",
        report.target_platform, report.assets_copied, report.integrity, report.launcher
    );
    if let Some(runtime) = report.runtime_artifact {
        println!("runtime_artifact={runtime}");
    }
    if let Some(executable) = report.executable {
        println!("executable={executable}");
    }
    if let Some(signature) = report.bundle_hmac_sha256 {
        println!("bundle_hmac_sha256={signature}");
    }
    Ok(())
}
