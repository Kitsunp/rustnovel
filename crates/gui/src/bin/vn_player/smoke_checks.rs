use super::*;

pub(super) fn smoke_check(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(code, status, target, message, SmokeCheckScope::default())
}

pub(super) fn smoke_check_with_file(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    file: &str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(
        code,
        status,
        target,
        message,
        SmokeCheckScope {
            file: Some(file),
            ..SmokeCheckScope::default()
        },
    )
}

pub(super) fn smoke_check_with_asset(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    asset: &str,
) -> ExportRuntimeSmokeCheck {
    smoke_check_with_scope(
        code,
        status,
        target,
        message,
        SmokeCheckScope {
            asset: Some(asset),
            ..SmokeCheckScope::default()
        },
    )
}

pub(super) fn smoke_metadata_check(
    target: &str,
    bundle_root: Option<&Path>,
    path: &Path,
    field: &'static str,
    err: &dyn std::error::Error,
) -> ExportRuntimeSmokeCheck {
    let file = report_relative_path(path, bundle_root);
    smoke_check_with_scope(
        "export.runtime_smoke.metadata_read",
        "failed",
        target,
        &format!("{field} '{}': {err}", path.display()),
        SmokeCheckScope {
            file: Some(&file),
            field: Some(field),
            ..SmokeCheckScope::default()
        },
    )
}

pub(super) fn smoke_check_with_scope(
    code: &str,
    status: &str,
    target: &str,
    message: &str,
    scope: SmokeCheckScope<'_>,
) -> ExportRuntimeSmokeCheck {
    let (probable_cause, suggested_action, consequence) = smoke_metadata(code);
    ExportRuntimeSmokeCheck {
        code: code.to_string(),
        status: status.to_string(),
        severity: smoke_severity(status).to_string(),
        phase: "smoke".to_string(),
        target: target.to_string(),
        trace_id: stable_trace_id(code, target, message),
        message: message.to_string(),
        probable_cause: probable_cause.to_string(),
        suggested_action: suggested_action.to_string(),
        consequence: consequence.to_string(),
        blocking_release: status != "passed",
        file: scope.file.map(ToOwned::to_owned),
        asset: scope.asset.map(ToOwned::to_owned),
        node: scope.node.map(ToOwned::to_owned),
        field: scope.field.map(ToOwned::to_owned),
    }
}

pub(super) fn smoke_scope_summary(check: &ExportRuntimeSmokeCheck) -> String {
    let mut scope = Vec::new();
    if let Some(file) = &check.file {
        scope.push(format!("file={file}"));
    }
    if let Some(asset) = &check.asset {
        scope.push(format!("asset={asset}"));
    }
    if let Some(node) = &check.node {
        scope.push(format!("node={node}"));
    }
    if let Some(field) = &check.field {
        scope.push(format!("field={field}"));
    }
    if scope.is_empty() {
        "scope=runtime".to_string()
    } else {
        scope.join(" ")
    }
}

pub(super) fn smoke_error(
    code: &str,
    target: &str,
    detail: impl Into<String>,
) -> Box<dyn std::error::Error> {
    smoke_error_with_scope(code, target, detail, SmokeCheckScope::default())
}

pub(super) fn smoke_error_with_scope(
    code: &str,
    target: &str,
    detail: impl Into<String>,
    scope: SmokeCheckScope<'_>,
) -> Box<dyn std::error::Error> {
    let detail = detail.into();
    let check = smoke_check_with_scope(code, "failed", target, &detail, scope);
    Box::new(SmokeFailure { check, detail })
}

pub(super) fn smoke_severity(status: &str) -> &'static str {
    match status {
        "passed" => "info",
        "not_run" => "warning",
        _ => "error",
    }
}

pub(super) fn smoke_metadata(code: &str) -> (&'static str, &'static str, &'static str) {
    match code {
        "export.runtime_smoke.launch_paths" => (
            "The launcher, script path, assets root, or manifest path may not match the exported bundle layout.",
            "Check the package report paths, launcher arguments, and bundle root used by the runtime.",
            "The packaged game cannot start from the exported layout.",
        ),
        "export.runtime_smoke.script_parse" => (
            "The packaged runtime script may be corrupt, missing schema data, or incompatible with the core schema policy.",
            "Rebuild the package from the current script and inspect scripts/compiled.vnscript.json.",
            "The runtime cannot initialize the story from the package.",
        ),
        "export.runtime_smoke.engine_init" => (
            "The packaged script may contain invalid labels, choices, resources, or security-sensitive content.",
            "Inspect the script diagnostics and rebuild after fixing the authored flow.",
            "The runtime engine cannot load the exported story.",
        ),
        "export.runtime_smoke.asset_manifest" => (
            "The asset manifest may be missing, malformed, or inconsistent with the packaged assets root.",
            "Compare meta/assets_manifest.json with the package report and exported asset files.",
            "The runtime cannot verify packaged assets before rendering.",
        ),
        "export.runtime_smoke.runtime_app" => (
            "The runtime app could not connect the engine, input, audio, or asset store for the packaged bundle.",
            "Inspect runtime initialization errors and the selected backend/audio configuration.",
            "The game cannot open a playable runtime session.",
        ),
        "export.runtime_smoke.asset_load" => (
            "A referenced packaged asset may be missing, unsafe, or inconsistent with the asset manifest.",
            "Inspect the asset path in the script, package report hashes, and meta/assets_manifest.json.",
            "A rendered frame may be missing required visual content.",
        ),
        "export.runtime_smoke.render_frame" => (
            "The software renderer contract reported invalid commands, layout, image, or button state.",
            "Inspect the scene frame diagnostics, renderer backend, and visual commands for the failing scene.",
            "The package cannot prove it can render a frame on the target backend.",
        ),
        "export.runtime_smoke.advance_scene" => (
            "The runtime engine returned an error while advancing the packaged scene.",
            "Inspect the current event, authored flow, choices, external calls, and runtime error message.",
            "The package cannot prove the player can progress through the exported story.",
        ),
        "export.runtime_smoke.close" => (
            "The runtime did not accept or complete the close action cleanly.",
            "Inspect runtime input handling and shutdown state for the packaged app.",
            "The smoke job cannot prove the game closes without panic or stuck state.",
        ),
        "export.runtime_smoke.report_read" => (
            "A package, compat, or smoke report path could not be read from disk.",
            "Check report paths, staging output, and file permissions.",
            "CI cannot connect runtime smoke evidence back into export artifacts.",
        ),
        "export.runtime_smoke.report_parse" => (
            "A package, compat, or smoke report is not valid JSON for the expected schema.",
            "Inspect the report content and the writer that produced it.",
            "CI cannot safely compare or update export artifacts.",
        ),
        "export.runtime_smoke.report_write" => (
            "The smoke job could not write updated smoke_result back to an export report.",
            "Check output permissions and whether the report path points inside the bundle.",
            "The export artifacts may not record the runtime smoke outcome.",
        ),
        "export.runtime_smoke.metadata_read" => (
            "The smoke job could not read package metadata needed for runtime evidence.",
            "Inspect bundle metadata files, encoding, and filesystem permissions.",
            "The smoke report cannot include complete bundle integrity evidence.",
        ),
        _ => (
            "The runtime smoke check failed for an unspecified package/runtime condition.",
            "Inspect the check code, message, and trace_id in the smoke report.",
            "The package cannot be treated as fully smoke-verified.",
        ),
    }
}

pub(super) fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn stable_trace_id(code: &str, target: &str, subject: &str) -> String {
    let digest = sha256_hex(format!("{code}:{target}:{subject}").as_bytes());
    format!("export-smoke-{}", &digest[..16])
}

pub(super) fn normalize_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn report_relative_path(path: &Path, bundle_root: Option<&Path>) -> String {
    bundle_root
        .and_then(|root| path.strip_prefix(root).ok())
        .map(normalize_path_display)
        .unwrap_or_else(|| normalize_path_display(path))
}
