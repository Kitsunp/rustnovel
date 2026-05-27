# Core API V2 Refactor Audit - 2026-05-27

## Scope

This audit records the implemented state of the Core API V2 refactor. The target architecture is:

- `visual_novel_engine::authoring` is the canonical authoring, traceability, validation, command, presentation, and export planning API.
- `visual_novel_engine::runtime` is the canonical runtime facade for raw script events, compiled scripts, engine execution, trace, UI state, and execution contracts.
- Root-level legacy aliases and ambiguous legacy fallbacks are removed.
- Strict schema, path, report, operation log, and save contracts are enforced at the boundary.

## Executive Status

| Area | Status | Evidence |
| --- | --- | --- |
| API V2 facade | Closed | Root `Event`/`Script` aliases removed; runtime exports moved behind `runtime`. |
| Legacy report v1 import | Closed | `vneditor.diagnostic_report.v1` is rejected; only `vnengine.authoring_validation_report.v2` is accepted. |
| Operation log legacy states | Closed | `OperationKind::Legacy` and `OperationStatus::Legacy` removed; entries require typed status. |
| Runtime script legacy schema | Closed | `script_schema_version` is required and must match the current version. |
| Manifest legacy alias | Closed | `schema_version` alias is rejected; `manifest_schema_version` is required. |
| Save legacy payload loading | Closed | Plain unauthenticated legacy payload fallback removed. |
| Shared helper contracts | Closed | Shared modules added for clock, event signatures, asset references, and asset path policy. |
| File size below 500 lines | Closed | Final line scan found no Rust/Python source or test file above 500 lines. |
| Tests outside `src` | Closed | `rg` finds no `#[cfg(test)]`, inline `mod tests`, or `#[path = "...tests..."]` test modules under crate/tool `src` trees. |
| Command bus migration | Closed for graph mutations | Python, CLI, and GUI graph edits route through `AuthoringCommandBus` directly or through the GUI `NodeGraph` wrapper backed by it; remaining pushes are central operation-log sinks. |
| Dependency convergence | Partial | `cargo tree -d` was measured; direct convergence was not forced where transitive constraints are owned by GUI/runtime stacks. |

## Implemented Architecture Changes

### Public API

- Added `crates/core/src/runtime.rs` as the runtime facade.
- Updated root `lib.rs` so editor-facing code no longer receives raw runtime types through root aliases.
- Updated core tests, runtime crate, Python bindings, GUI, CLI, examples, and benches to import runtime entities through `visual_novel_engine::runtime`.
- Removed root `pub type Event` and `pub type Script`.

### Legacy Boundary Removal

- `AuthoringValidationReport::from_json` now rejects v1 reports instead of adapting them.
- GUI report import now accepts only the v2 report schema.
- `ScriptRaw` rejects missing, old, or incompatible `script_schema_version`.
- Runtime migration rejects legacy `0.9` payloads.
- `ProjectManifest` rejects `schema_version` alias payloads.
- Save loading rejects unauthenticated legacy plain payloads.
- Operation logs no longer preserve arbitrary legacy kinds/statuses.

### Shared Core Helpers

- Added `clock::now_unix_ms` and replaced duplicate timestamp helpers.
- Added shared event signature helpers used by compiler/repro contracts.
- Added shared asset reference collection used by bundle/report fingerprint paths.
- Hardened shared asset reference policy:
  - rejects absolute paths,
  - rejects traversal,
  - rejects Windows drive paths,
  - rejects UNC-like/backslash forms,
  - rejects encoded traversal tokens such as `%2e`, `%2f`, and `%5c`.
- Asset existence validation now requires an explicit project root and canonicalizes both root and candidate.

### Refactor Splits

- Split `authoring/composer.rs` into focused modules for types, presentation snapshot, preview, overlays, objects, and runtime objects.
- Split `bundle.rs` into export facade plus `spec`, `plan`, and `materialize` modules.
- Split large CLI command/test files into domain-specific files.
- Moved the large authoring test module out of `src` into integration tests.
- Moved remaining core/assets/runtime/gui/Python embedded tests out of `src`.
- Split export bundle HMAC tests into a separate integration test file.
- Split `authoring/command_bus.rs` into focused `types`, `apply`, `choice`, `fragments`, `inverse`, and `node_edits` modules.

## Tests Added Or Strengthened

Key contracts now covered include:

- `api_v2_rejects_legacy_payloads`
- `public_api_has_no_legacy_aliases`
- `operation_log_status_is_typed_and_rejects_legacy_status`
- `label_out_of_range_contract`
- `extcall_simulation_fidelity_contract`
- `presentation_snapshot_parity`
- `duplicated_character_pose_parity`
- `long_choices_transition_snapshot_contract`
- `asset_cache_fingerprint_invalidation`
- `audio_metadata_invalidation`
- `asset_cache_multiview_stress`
- `command_bus_replay_headless`
- `command_bus_covers_python_semantic_edits_and_fragment_navigation`
- `undo_delta_memory_contract`
- `operation_log_replay_create_connect_edit_import_move_revert`
- `evidence_trace_asset_resolver_chain`
- `export_plan_cli_py_gui_parity`
- `fragment_nested_namespace_stress`
- `repeated_fragment_calls_do_not_collide`
- `deleting_internal_node_invalidates_ports_and_blocks_strict_export`
- `contract_fixture_cli_py_gui`
- `path_traversal_encoded_windows_unc_contract`
- `cli_trace_json_contract`
- `example_entrypoint_authoring_validate`
- `python_native_module_origin_healthcheck`

## Bugs Found And Fixed

- Example script validation failed because `examples/scripts/demo_story.json` did not declare the current script schema version.
- GUI project/config tests still accepted legacy manifest/save shapes; those assertions now reject legacy payloads.
- Python report fixture omitted typed operation status; the fixture now uses `status_v2`.
- CLI trace contract is explicitly parseable for JSON and YAML output.
- Quick-fix and validation path checks were inconsistent for Windows drive paths; both now share the stricter core policy.
- Export bundle test file exceeded the line policy; HMAC tests were split into their own integration test.
- Script sync export exceeded the line policy; end-label detection was extracted to a helper module.
- `authoring/command_bus.rs` grew past the 500-line policy after migration; it was split into focused submodules.
- Python graph editing methods and GUI Visual Composer/asset/fragment edits still bypassed the command bus in several paths; those now use typed `AuthoringCommand` variants while preserving existing operation hints.

## Validation

Final local commands:

| Command | Result |
| --- | --- |
| `cargo fmt` | Pass |
| `cargo fmt --check` | Pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass |
| `cargo test --workspace --all-targets --no-fail-fast` | Pass |
| `cargo audit -D warnings --ignore RUSTSEC-2024-0436` | Pass |
| `py -m ruff format --check .` | Pass |
| `py -m ruff check .` | Pass |
| `py -m mypy` | Pass |
| isolated venv `maturin develop --manifest-path crates\py\Cargo.toml --features extension-module` | Pass when `VIRTUAL_ENV` is explicitly set |
| isolated venv `python -m pytest -q` | Pass |
| CLI `validate examples\scripts\demo_story.json` | Pass |
| CLI `authoring validate examples\scripts\demo_story.json --project-root examples\scripts` | Pass |
| CLI `trace examples\scripts\script.json --format json` | Pass and JSON parses |
| CLI `trace examples\scripts\script.json --format yaml` | Pass and YAML parses |
| `cargo tree -d` | Measured; duplicates remain in transitive GUI/runtime stacks. |

Static checks:

- Legacy markers removed for `from_legacy_v1`, `vneditor.diagnostic_report.v1`, `OperationKind::Legacy`, `OperationStatus::Legacy`, root `pub type Event`, root `pub type Script`, and `default_asset_exists`.
- No Rust/Python file under the checked repo code/test paths exceeded 500 lines after the final split.
- No embedded Rust tests remain under `crates/**/src/**` or `tools/cli/src/**`.
- Remaining manual `operation_log.push` sites are central sinks: Python `record_python_operation` and GUI `append_editor_operation`.

## Remaining Work

### Document-Level Layer Overrides

Graph-level mutations are now routed through `AuthoringCommandBus`. Composer layer visibility/lock overrides are document-local UI state, not `NodeGraph` state, and still use core composer helpers plus typed operation-log entries.

Recommended next slice:

1. Decide whether to introduce a separate `AuthoringDocumentCommandBus` for document-level metadata such as layer overrides and background-fit overrides.
2. If introduced, migrate Python and GUI layer override handlers to that document bus while preserving current operation fingerprints.
3. Keep central operation-log sinks as sinks only; do not reintroduce per-feature synthetic log construction.

### Dependency Convergence

`cargo tree -d` still reports duplicate dependency families such as image/toml and transitive Windows/wgpu/rodio/cpal chains. These were not forced because several duplicates are constrained by upstream GUI/runtime stacks.

Recommended next slice:

1. Converge only direct dependencies first.
2. Leave transitive platform/audio/GPU duplicates unless Cargo constraints prove a safe single version.
3. Store an intentional `dependency_convergence_snapshot` when the duplicate set is understood.

## Human Review Points

- Confirm that removing root raw/runtime aliases is acceptable as a breaking public API change.
- Confirm that rejecting all legacy report/script/manifest/save payloads is intentional for downstream users.
- Decide whether document-level layer/background-fit overrides should get a separate command bus or remain typed UI document operations.
- Review dependency duplicate families before forcing convergence across GUI/runtime platform, audio, and GPU stacks.
