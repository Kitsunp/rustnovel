use super::*;
use std::io::{BufReader, Read};

fn load_engine(path: &Path) -> Result<Engine> {
    Ok(load_prepared_runtime_entry(path)
        .with_context(|| format!("load script '{}'", path.display()))?
        .into_engine(SecurityPolicy::default(), ResourceLimiter::default())?)
}

fn load_compiled_script(path: &Path) -> Result<ScriptCompiled> {
    Ok(load_prepared_runtime_entry(path)
        .with_context(|| format!("load script '{}'", path.display()))?
        .into_compiled(&SecurityPolicy::default(), ResourceLimiter::default())?)
}

pub(super) fn route_tree_command(script: &Path) -> Result<Option<serde_json::Value>> {
    let engine = load_engine(script)?;
    Ok(Some(serde_json::to_value(engine.route_tree())?))
}

pub(super) fn read_model_command(input: &Path) -> Result<Option<serde_json::Value>> {
    let read_error = |error| {
        visual_novel_engine::VnError::invalid_script(format!(
            "read input '{}': {error}",
            input.display()
        ))
    };
    let file = fs::File::open(input)
        .map_err(read_error)
        .context("load script or save")?;
    let mut reader = BufReader::new(file);
    let mut prefix = Vec::with_capacity(visual_novel_engine::AUTH_SAVE_MAGIC.len());
    reader
        .by_ref()
        .take(visual_novel_engine::AUTH_SAVE_MAGIC.len() as u64)
        .read_to_end(&mut prefix)
        .map_err(read_error)?;
    let is_save = prefix.as_slice() == visual_novel_engine::AUTH_SAVE_MAGIC;
    let reader = prefix.as_slice().chain(reader);
    if is_save {
        let save = SaveData::read_authenticated(reader, AUTH_SAVE_KEY, DEFAULT_MAX_SAVE_BYTES)
            .with_context(|| format!("parse save '{}'", input.display()))?;
        return Ok(Some(serde_json::json!({
            "schema": "vnengine.cli.read_model.v1",
            "source": "save",
            "read_model": save.state.read_model,
            "route_progress": save.state.route_progress
        })));
    }
    let mut engine = read_prepared_runtime_entry(reader)
        .with_context(|| {
            format!(
                "load script or save '{}'; save attempt failed: missing authenticated save header",
                input.display()
            )
        })?
        .into_engine(SecurityPolicy::default(), ResourceLimiter::default())?;
    let max_steps = engine
        .script()
        .events
        .len()
        .saturating_mul(4)
        .saturating_add(32);
    let (executed_steps, stop_reason) = simulate_read_model(&mut engine, max_steps)?;
    Ok(Some(serde_json::json!({
        "schema": "vnengine.cli.read_model.v1",
        "source": "script",
        "read_model": engine.read_model_snapshot(),
        "route_progress": engine.route_progress_snapshot(),
        "simulation": {
            "partial": stop_reason != "finished",
            "stop_reason": stop_reason,
            "executed_steps": executed_steps,
            "max_steps": max_steps,
            "choice_policy": "first"
        }
    })))
}

fn simulate_read_model(engine: &mut Engine, max_steps: usize) -> Result<(usize, &'static str)> {
    let mut executed_steps = 0;
    for _ in 0..max_steps {
        let event = match engine.current_event_ref() {
            Ok(event) => event,
            Err(visual_novel_engine::VnError::EndOfScript) => {
                return Ok((executed_steps, "finished"))
            }
            Err(err) => return Err(err).context("read-model current_event"),
        };
        match event {
            visual_novel_engine::runtime::EventCompiled::Choice(choice) => {
                if choice.options.is_empty() {
                    return Ok((executed_steps, "empty_choice"));
                }
                engine.choose(0)?;
            }
            visual_novel_engine::runtime::EventCompiled::ExtCall { .. } => engine.resume()?,
            _ => {
                let (_audio, _change) = engine.step()?;
            }
        }
        executed_steps += 1;
    }
    match engine.current_event_ref() {
        Ok(_) => Ok((executed_steps, "step_limit")),
        Err(visual_novel_engine::VnError::EndOfScript) => Ok((executed_steps, "finished")),
        Err(err) => Err(err).context("read-model current_event"),
    }
}

pub(super) fn theme_validate_command(theme_path: &Path) -> Result<Option<serde_json::Value>> {
    let raw =
        fs::read_to_string(theme_path).with_context(|| format!("read {}", theme_path.display()))?;
    let theme: UiTheme = serde_json::from_str(&raw)
        .with_context(|| format!("parse theme {}", theme_path.display()))?;
    let report = validate_ui_theme(&theme);
    if !report.valid {
        anyhow::bail!(
            "theme validation failed: {}",
            serde_json::to_string(&report)?
        );
    }
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
    let require_executable = args.require_executable;
    let spec = export_spec_from_args(args);
    let service = ExportService::new();
    let plan = service.plan_export(&spec)?;
    if require_executable {
        service.validate_executable_export_plan(&plan, spec.target_platform)?;
    }
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
    load_compiled_script(path)?;
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
    let compiled = load_compiled_script(path)?;
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
    let mut engine = load_engine(path)?;
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
    if !partial && engine.state().position as usize != engine.script().events.len() {
        partial = true;
        stopped_reason = Some(format!("step limit reached: {steps}"));
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
    let save_file =
        fs::File::open(save_path).with_context(|| format!("read {}", save_path.display()))?;
    let save = SaveData::read_authenticated(save_file, AUTH_SAVE_KEY, DEFAULT_MAX_SAVE_BYTES)
        .with_context(|| format!("load save '{}'", save_path.display()))?;
    let script_bytes =
        fs::read(script_path).with_context(|| format!("read {}", script_path.display()))?;
    let compiled = ScriptCompiled::from_binary(&script_bytes)?;
    save.validate_script_id(&compiled.script_id()?)?;
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
    visual_novel_engine::atomic_io::sha256_file(path)
        .with_context(|| format!("hash {}", path.display()))
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

#[cfg(test)]
mod read_model_tests {
    use super::*;

    #[test]
    fn final_event_at_step_limit_is_finished() {
        let script = visual_novel_engine::runtime::ScriptRaw::from_json(
            r#"{"script_schema_version":"1.0","events":[{"type":"dialogue","speaker":"A","text":"End"}],"labels":{"start":0}}"#,
        )
        .unwrap();
        let mut engine = Engine::new(
            script,
            SecurityPolicy::default(),
            ResourceLimiter::default(),
        )
        .unwrap();
        assert_eq!(
            simulate_read_model(&mut engine, 1).unwrap(),
            (1, "finished")
        );
    }
}
