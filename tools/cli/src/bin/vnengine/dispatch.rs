use super::*;

pub(super) fn run_command(cli: Cli, json_output: bool) -> Result<Option<serde_json::Value>> {
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
            allow_partial_trace,
            fail_on_engine_error: _,
        } => {
            trace_script(&script, steps, format, &output, allow_partial_trace)?;
            Ok(Some(serde_json::json!({
                "schema": "vnengine.cli.trace.v1",
                "script": normalize_cli_path(&script),
                "output": normalize_cli_path(&output),
                "steps_requested": steps,
                "allow_partial_trace": allow_partial_trace,
                "fail_on_engine_error": !allow_partial_trace
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
                        let diagnostics = plan
                            .diagnostics
                            .iter()
                            .filter(|diagnostic| diagnostic.blocking_release)
                            .map(|diagnostic| {
                                format!(
                                    "{} trace_id={} phase={} target={} message={} action={}",
                                    diagnostic.code,
                                    diagnostic.trace_id,
                                    diagnostic.phase,
                                    diagnostic.target,
                                    diagnostic.message,
                                    diagnostic.suggested_action
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        let message = format!(
                            "{} executable export cannot produce '{}'; blocking diagnostics: {}",
                            spec.target_platform.as_str(),
                            expected,
                            diagnostics
                        );
                        return Err(visual_novel_engine::VnError::invalid_script(message).into());
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
