use super::*;

pub(super) fn route_tree_command(script: &Path) -> Result<Option<serde_json::Value>> {
    let script_raw = load_runtime_script_from_entry(script).context("load script")?;
    let engine = Engine::new(
        script_raw,
        SecurityPolicy::default(),
        ResourceLimiter::default(),
    )?;
    Ok(Some(serde_json::to_value(engine.route_tree())?))
}

pub(super) fn read_model_command(input: &Path) -> Result<Option<serde_json::Value>> {
    let save_attempt_error = match fs::read(input) {
        Ok(bytes) => match SaveData::from_any_binary(&bytes, AUTH_SAVE_KEY) {
            Ok(save) => {
                return Ok(Some(serde_json::json!({
                    "schema": "vnengine.cli.read_model.v1",
                    "source": "save",
                    "read_model": save.state.read_model,
                    "route_progress": save.state.route_progress
                })));
            }
            Err(err) => Some(format!("parse save '{}': {err}", input.display())),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => Some(format!("read save '{}': {err}", input.display())),
    };

    let context = match save_attempt_error {
        Some(err) => format!("load script or save; save attempt failed: {err}"),
        None => "load script or save".to_string(),
    };
    let script_raw = load_runtime_script_from_entry(input).with_context(|| context)?;
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

pub(super) fn theme_validate_command(theme_path: &Path) -> Result<Option<serde_json::Value>> {
    let raw =
        fs::read_to_string(theme_path).with_context(|| format!("read {}", theme_path.display()))?;
    let theme: UiTheme = serde_json::from_str(&raw)
        .with_context(|| format!("parse theme {}", theme_path.display()))?;
    let report = validate_ui_theme(&theme);
    Ok(Some(serde_json::to_value(report)?))
}

pub(super) fn layout_resolve_command(
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

pub(super) fn export_plan_command(args: ExportCliArgs) -> Result<Option<serde_json::Value>> {
    let spec = export_spec_from_args(args);
    let plan = ExportService::new().plan_export(&spec)?;
    Ok(Some(serde_json::to_value(plan)?))
}

pub(super) fn export_execute_command(args: ExportCliArgs) -> Result<Option<serde_json::Value>> {
    let require_executable = args.require_executable;
    let spec = export_spec_from_args(args);
    let report = if require_executable {
        visual_novel_engine::export_executable_bundle(spec)?
    } else {
        ExportService::new().execute_export(spec)?
    };
    Ok(Some(serde_json::to_value(report)?))
}

pub(super) fn export_spec_from_args(args: ExportCliArgs) -> ExportBundleSpec {
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

pub(super) fn validate_script(path: &Path) -> Result<()> {
    let script = load_runtime_script_from_entry(path).context("load script")?;
    let policy = SecurityPolicy::default();
    let limits = ResourceLimiter::default();
    policy.validate_raw(&script, limits)?;
    let compiled = script.compile()?;
    policy.validate_compiled(&compiled, limits)?;
    Ok(())
}

pub(super) fn migrate_script_command(
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

pub(super) fn compile_script(path: &Path, output: &Path) -> Result<serde_json::Value> {
    let script = load_runtime_script_from_entry(path).context("load script")?;
    let compiled = script.compile()?;
    let bytes = compiled.to_binary()?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(output, &bytes)?;
    let bytes_written = fs::metadata(output)
        .with_context(|| format!("read compiled output metadata {}", output.display()))?
        .len();
    Ok(serde_json::json!({
        "schema": "vnengine.cli.compile.v1",
        "script": normalize_cli_path(path),
        "output": normalize_cli_path(output),
        "bytes_written": bytes_written
    }))
}

pub(super) fn trace_script(
    path: &Path,
    steps: usize,
    requested_format: Option<TraceFormat>,
    output: &Path,
    allow_partial_trace: bool,
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
    let mut partial = false;
    let mut stopped_reason = None;
    for step in 0..steps {
        let position = engine.state().position as usize;
        if position == engine.script().events.len() {
            break;
        }
        let event = match engine.current_event() {
            Ok(event) => event,
            Err(err) if allow_partial_trace => {
                partial = true;
                stopped_reason = Some(format!("current_event failed at step {step}: {err}"));
                break;
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("trace current_event failed at step {step}"));
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
            if allow_partial_trace {
                partial = true;
                stopped_reason = Some(format!("advance failed at step {step}: {err}"));
                break;
            }
            return Err(err).with_context(|| format!("trace advance failed at step {step}"));
        }
    }
    let envelope = TraceEnvelope {
        trace_format_version: 1,
        script_schema_version: SCRIPT_SCHEMA_VERSION.to_string(),
        partial,
        stopped_reason,
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

pub(super) fn verify_save(save_path: &Path, script_path: &Path) -> Result<()> {
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

pub(super) fn build_manifest(root: &Path, output: &Path) -> Result<()> {
    let canonical_root = root
        .canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))?;
    let canonical_output = output.canonicalize().ok();
    let mut assets = std::collections::BTreeMap::new();
    for entry in WalkDir::new(root) {
        let entry = entry.with_context(|| format!("walk {}", root.display()))?;
        let path = entry.path();
        if entry.file_type().is_dir() {
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
        if canonical_output.as_ref() == Some(&canonical_path) {
            continue;
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

pub(super) fn sha256_file_hex(path: &Path) -> Result<(String, u64)> {
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

pub(super) fn digest_hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn run_repro_bundle(path: &Path, output: Option<&Path>, strict: bool) -> Result<()> {
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
