# Matriz de Aceptacion QA + KPI V2

Owner: QA Lead

## KPI funcionales

1. F01 ruido import story_first: reduccion >=80%.
2. F02 degradacion import: <=25% en subset soportado.
3. F03 paridad audio runtime: >=95%.
4. F04 bloqueos action flow: 0.
5. F05 independencia de CWD para paths: 100%.
6. F06 package windows smoke pass: 100%.

## KPI calidad

1. Q01 tipado dominio sin opacidad critica.
2. Q02 >=95% diagnosticos con `root_cause/how_to_fix/docs_ref/trace_id`.
3. Q03 colisiones de `diagnostic_id`: 0.

## KPI performance

1. P01 auto-fix >=3x vs baseline.
2. P02 input lag p95 <120ms.
3. P03 lint panel 10k <100ms.

## Gates obligatorios

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --verbose`

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| ACC-001 | Todos los gates tecnicos en verde | QA Lead | 100% pass | CI mandatory gates | CI summary |
| ACC-002 | KPIs funcionales criticos cumplidos | Product Eng | 6/6 criticos ok | KPI gate | KPI dashboard |
| ACC-003 | Seguridad sin fallos abiertos criticos | Security Lead | 0 Sev-0 abiertos | security signoff gate | security report |
| ACC-004 | Reproducibilidad de package validada | Release Eng | 100% runbook reproducible | packaging gate | reproducibility logs |
| ACC-005 | Trazabilidad import->runtime->package | Observability Lead | >=95% trazas correlables | observability gate | trace correlation report |

## Criterios de rechazo automatico

1. falla de cualquier gate obligatorio.
2. falla de cualquier prueba de path security.
3. bloqueo de flujo por action policy.
4. package smoke fail.
5. incumplimiento KPI critico sin waiver aprobado.
