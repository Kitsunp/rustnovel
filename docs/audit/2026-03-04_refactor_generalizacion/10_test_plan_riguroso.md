# Plan de Pruebas Riguroso V2

Owner: QA Lead

## Objetivo

1. Cubrir contratos V2 por capa.
2. Bloquear regresiones de seguridad y trazabilidad.

## Suites obligatorias

### A. Contratos y tipado

1. schema tests (audio/actions/import/errors/package).
2. snapshots de API publica V2.
3. breaking checks controlados.

### B. Seguridad

1. traversal/absolutos/url/symlink.
2. fuzz de argumentos en acciones custom.
3. path policy parity entre capas.

### C. Grafo y runtime

1. roundtrip graph<->script.
2. actions policy (auto/resume/async).
3. no stalls en flujo.

### D. UI/performance

1. auto-fix 100/500/1000 issues.
2. lint panel 10k issues.
3. diff panel p95.

### E. CLI/Python parity

1. envelope de diagnostico equivalente.
2. import report parity.
3. package report parity.

### F. Packaging Windows

1. package layout validation.
2. smoke-run.
3. reproducibilidad hash.

## Politica de severidad

1. Sev-0 (security/data loss): bloquea release siempre.
2. Sev-1 (runtime incorrecto): bloquea release.
3. Sev-2 (degradacion no critica): admite waiver temporal justificado.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| TST-001 | 100% suites obligatorias ejecutadas por fase | QA Lead | cobertura de suites = 100% | CI test matrix | CI artifacts |
| TST-002 | Sev-0 y Sev-1 sin waivers abiertos | QA Lead | 0 defects abiertos Sev-0/1 | release gate | defect tracker export |
| TST-003 | Fuzz seguridad activo en CI | Security Lead | >=1 fuzz job por release | security gate | fuzz logs |
| TST-004 | Parity gui/cli/py para diagnosticos | DX Lead | >=95% campos equivalentes | parity gate | snapshot compare |
| TST-005 | Package smoke-test windows obligatorio | Release Eng | 100% pass | packaging gate | smoke report |

## Evidencia minima por release

1. reporte de gates.
2. reporte KPI con delta.
3. riesgos abiertos y mitigacion.
4. hashes de artefacto.
