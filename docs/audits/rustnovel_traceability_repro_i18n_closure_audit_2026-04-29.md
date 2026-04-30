# RustNovel Traceability/Repro/I18n Closure Audit

Date: 2026-04-29

Scope: closure of `rustnovel_auditoria_trazabilidad_repro_i18n.md`, focused on diagnostics, reports, fingerprints, dry-run/repro, quick-fix audit, CLI, Python bindings, and local verification.

## Executive Result

The traceability audit is closed for the requested implementation pass. Diagnostics now have a structured catalog, real docs references, versioned diagnostic IDs, envelopes shared by core/GUI/CLI/Python, semantic fingerprints separated from build metadata, stronger imported-report trust handling, SHA-256 quick-fix audit hashes, repro diagnostic context, and visible ExtCall simulation warnings.

The main extra bug found during closure was in the local Python CI path on Windows: `maturin develop` could report success while the old installed `.pyd` stayed in `site-packages`, causing new binding tests to skip critical APIs. `scripts/ci-local.ps1` now runs Python tests against the freshly built local artifact first on Windows.

## Audit Points

| ID | Status | Closure |
|---|---|---|
| T-01 generic diagnostic explanations | Closed | `DiagnosticCatalog` provides per-code ES/EN title, what, root cause, why, consequence, fixes, expected/actual, and action steps. |
| T-02 broken docs_ref | Closed | Diagnostics point to `docs/diagnostics/authoring.md#...`; tests verify referenced anchors exist. |
| T-03 missing fingerprint not stale | Closed | GUI imports reports without fingerprints as untrusted/stale and blocks fixes while keeping issues readable. |
| T-04 fingerprint mixed with environment | Closed | Semantic fingerprint is separate from build profile/OS/arch; stale checks compare semantic data. |
| T-05 selected fix on stale report | Closed | Central GUI guard blocks selected, automatic, and batch fixes for stale/untrusted reports. |
| T-06 unstable quick-fix hash | Closed | Quick-fix audit uses SHA-256 over canonical authoring documents and records operation ids. |
| T-07 ReproCase lacks diagnostic/project refs | Closed | Repro cases include diagnostic id, semantic fingerprint, operation id, capabilities, plugin list, asset manifest hash, seed, and validation profile. |
| T-08 dry-run hides current_event errors | Closed | Dry-run treats only `EndOfScript` as clean finish; other errors become runtime diagnostics. |
| T-09 SceneProfile node_id=0 | Closed | SceneProfile asset validation no longer emits a fake node id. |
| T-10 imported report loses explanation | Closed | Report export/import preserves diagnostic envelope fields and localized actual messages. |
| T-11 diagnostic_id collision risk | Closed | Diagnostic ids are versioned and include phase, code, node, event ip, edge, asset, and blocked-flow context. |
| T-12 ExtCall simulation only note | Closed | Dry-run emits `DRY_EXTCALL_SIMULATED` warning plus simulation step metadata. |
| T-13 CLI report poorer than GUI | Closed | CLI uses core `AuthoringValidationReport` with `DiagnosticEnvelopeV2`. |
| T-14 Python generic/no dynamic locale | Closed | Python exposes envelope data and `localized(locale)`. |
| T-15 diagnostic localization separate/hard-coded | Closed | Diagnostic catalog is separate from narrative localization and uses stable message keys. |

## Tests Added Or Strengthened

- Core traceability tests cover catalog specificity, docs references, ExtCall simulation diagnostics, and verification-run introduced/resolved diagnostics.
- GUI report tests cover untrusted missing-fingerprint imports, semantic fingerprint comparison across build metadata, and stale report fix blocking.
- GUI diagnostic report tests now assert the v2 diagnostic id contract.
- CLI tests assert enriched authoring report envelopes and authoring-aware commands.
- Python tests now discover all `tests/python/test_*.py` files, exercise NodeGraph/StoryNode bindings, localized diagnostic envelopes, authoring save/load, JumpIf two-port roundtrip, and project-root validation.
- Repro tests assert diagnostic context, capabilities, seed, validation profile, plugins, and asset-manifest hash.

## Extra Fixes Found During Closure

- `scripts/ci-local.ps1` now discovers all Python tests instead of only two modules.
- Windows local Python tests prefer the freshly built `target/debug/visual_novel_engine.pyd`, avoiding stale global installs.
- Python binding tests now write temporary files under `target/python-test-tmp`, keeping generated test artifacts ignored by Git and inside the workspace.
- `scripts/ci-local.ps1` moved the cargo-audit database to `target/ci-local/audit-db` to avoid inherited Git ownership issues.

## Verification

Passed:

- `ruff format --check .`
- `ruff check .`
- `cargo fmt --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked --verbose`
- `cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --locked --verbose`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job python-tests`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job matrix-smoke`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job sbom-policy -SkipToolInstall`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job reproducible-smoke`
- `cargo build -p vnengine_py --profile python --features extension-module --locked --verbose`
- `cargo bench -p visual_novel_engine --bench core_benches --locked -- --warm-up-time 0.1 --measurement-time 0.1 --sample-size 10`
- Source file size audit: no checked source file over 500 lines.

Not fully verified locally:

- `cargo audit` reached the correct RustSec fetch path after the cache fix, but network access was blocked by the sandbox. Escalation was requested and rejected by the environment usage limit, so this specific online fetch could not be completed locally.

## Residual Risks

- `cargo bench` completed successfully, but some smoke measurements reported small regressions versus local prior baselines. The bench job is non-gating and noisy with 10 samples; no functional failure was observed.
- Full SARIF export and Fluent/FTL migration remain future enhancements. The current implementation keeps stable envelopes and message keys so those can be added without changing existing diagnostic semantics.
