# Metodologia y Dataset V2

Fecha de corte: 2026-03-04
Owner: QA + Architecture

## Objetivo

1. Definir un metodo repetible para validar cambios V2.
2. Asegurar comparabilidad contra baseline de auditoria.

## Dataset base

1. Fuente Ren'Py: `examples/renpy_the_question`.
2. Import baseline: `examples/imported_the_question/import_report.json`.
3. Metricas baseline conocidas:
4. files=52
5. events=1433
6. degraded=1315
7. issues=12155

## Estrategia de evidencia

1. Todo KPI debe guardar evidencia en artifact JSON/YAML.
2. Cada gate debe tener comando reproducible.
3. Cada comparacion debe incluir baseline y delta.

## Flujo de medicion

1. Import (story_first/full/custom) -> `import_report_v2.json`.
2. Compile/dry-run -> `diagnostic_report_v2.json`.
3. Runtime parity -> `runtime_parity_report.json`.
4. Package -> `package_report.json` + smoke report.

## Comandos de referencia

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `vnengine import-renpy ... --profile story-first`
5. `vnengine package --target windows --output ...`

## Regla de reproducibilidad

1. Registrar hash de commit y fecha de ejecucion.
2. Registrar version de rust/cargo.
3. Registrar SO target para packaging.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| MET-001 | Baseline y delta obligatorios por KPI | QA Lead | 100% KPIs con baseline+delta | QA gate report | `kpi_delta.json` |
| MET-002 | Comandos de validacion reproducibles | Release Eng | 100% gates con comando documentado | CI reproducibility check | CI logs |
| MET-003 | Evidencia persistente por fase | QA Lead | 100% fases con artifact | artifact audit | artifacts index |
| MET-004 | Versionado explicito de reportes | Tooling Lead | 100% reportes incluyen `schema` | schema validator | schema snapshots |

## Definition of Done (documental)

1. Todas las reglas de este documento tienen evidencia asociada.
2. No hay KPI sin baseline.
3. No hay gate sin comando.

## Riesgos abiertos

1. Dataset unico puede sesgar cobertura.
2. Mitigacion: agregar fixtures audio/actions/grafo en fase 3.
