# RFC Index V2 - Identidad del Motor y Adaptadores Externos

Fecha de corte: 2026-03-04
Estado: Normativo
Owner global: Architecture Council

## Decisiones bloqueadas

1. API publica V2 adopta breaking directo (sin capa legacy larga).
2. Dependencias centralizadas por `workspace.dependencies`, con excepciones justificadas por crate.
3. Packaging ejecutable funcional fase inicial: Windows x64 primero.
4. El motor es contrato canonico; Ren'Py es fuente de entrada adaptada.

## Objetivo del pack V2

1. Convertir la auditoria en reglas ejecutables con gates medibles.
2. Remover ambiguedad en contratos, errores, import y packaging.
3. Establecer crecimiento escalable para core/gui/cli/py.

## Alcance

1. Contratos V2 para audio, acciones custom, import, diagnosticos y bundle.
2. Criterios de seguridad, trazabilidad, tipado y performance.
3. Plan de pruebas y matriz de aceptacion KPI.

## No alcance

1. Implementacion total de runtime V2 en este documento.
2. Packaging Linux/macOS final en esta fase.

## Mapa de documentos

1. 00_index.md: indice RFC y decisiones bloqueadas.
2. 01_metodologia_y_dataset.md: metodologia de evidencia y reproducibilidad.
3. 02_bug_catalog_audio.md: contrato Audio V2 y brechas.
4. 03_bug_catalog_import_renpy.md: import adaptativo Ren'Py -> motor.
5. 04_bug_catalog_grafo_generalizacion.md: contrato grafo y ActionRegistry.
6. 05_bug_catalog_tipado_interpretabilidad.md: tipado fuerte y explicabilidad.
7. 06_bug_catalog_ui_perf_autofix.md: SLO UI/performance y batch.
8. 07_bug_catalog_seguridad_y_paths.md: politica unica de paths.
9. 08_bug_catalog_export_bundle.md: contrato packaging Windows v1.
10. 09_refactor_blueprint_metaprogramacion.md: blueprint de macros/traits.
11. 10_test_plan_riguroso.md: suites/gates por severidad.
12. 11_acceptance_matrix_qa_kpi.md: KPIs y gates de release.
13. 12_quality_security_architecture_criteria.md: criterios transversales.
14. 13_public_api_v2_contract.md: contrato API publica V2.
15. 14_dependency_governance_and_component_reuse.md: governance y reuse.
16. 15_error_traceability_flow_contract.md: flujo de error/traza.
17. 16_windows_executable_release_contract.md: release funcional Windows.

## Norma de formato

1. Ningun archivo supera 500 lineas.
2. Toda regla normativa debe incluir: owner, metric, gate, evidence.
3. Toda regla debe poder validarse via test o comando reproducible.

## Reglas normativas base

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| IDX-001 | Mantener identidad canonica del motor en todo contrato | Architecture Council | 100% docs V2 declaran canon interno | Review checklist V2 | RFC review signed |
| IDX-002 | Bloquear APIs opacas sin schema estable | Core Lead | 0 contratos dominio con payload opaco sin schema | `cargo clippy -D warnings` + API lint | `api-validate` report |
| IDX-003 | Mantener trazabilidad extremo a extremo | Observability Lead | >=95% diagnosticos con `trace_id` | test de reportes gui/cli/py | snapshots de envelope |
| IDX-004 | Publicar gates ejecutables por fase | QA Lead | 100% fases con gate + evidencia | CI pipeline por fase | artifacts CI |

## Referencias normativas externas

1. Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
2. Cargo Workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html
3. SemVer 2.0.0: https://semver.org/
4. CycloneDX: https://cyclonedx.org/specification/overview/
5. SLSA Levels v1.0: https://slsa.dev/spec/v1.0/levels
6. OWASP ASVS: https://github.com/OWASP/ASVS
7. OpenTelemetry Trace API: https://opentelemetry.io/docs/specs/otel/trace/api/
8. PyO3 docs: https://pyo3.rs/
