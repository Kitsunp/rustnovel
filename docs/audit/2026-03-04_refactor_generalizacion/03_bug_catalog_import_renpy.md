# Catalogo Import Ren'Py Adaptativo al Motor

Owner: Import Lead

## Principio rector

1. El motor define semantica canonica.
2. Ren'Py se adapta al motor por perfiles de import.
3. Compatibilidad 1:1 con Ren'Py no es objetivo.

## ImportRenpyOptionsV2 (normativo)

1. `profile`: `story_first | full | custom`.
2. `include_tl`, `include_ui`.
3. `include_patterns`, `exclude_patterns`.
4. `strict_mode`: falla en incompatibilidad critica.
5. `fallback_policy`: `strict | degrade_with_trace`.

## Esquema de issue V2

1. `code`, `severity`, `phase`, `area`.
2. `file`, `line`, `column`, `snippet`.
3. `fallback_applied`, `trace_id`, `path_display`.

## Perfiles

1. `story_first` (default): prioriza narrativa canonica.
2. `full`: parseo amplio para auditoria.
3. `custom`: control por patrones para pipelines mixtos.

## Politica de fallback

1. `strict`: aborta en statements criticos no mapeables.
2. `degrade_with_trace`: degrada con issue obligatorio y `trace_id`.

## Brechas auditadas (resumen)

1. Ruido excesivo por `tl/` y UI.
2. Degradacion alta sin contexto suficiente.
3. Seguridad de path no uniforme.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| IMP-001 | `story_first` como default oficial | Import Lead | 100% invocaciones default = story_first | CLI/GUI defaults tests | integration snapshots |
| IMP-002 | Reporte V2 con campos de trazabilidad | Observability Lead | >=95% issues con contexto completo | schema validation tests | import_report_v2 |
| IMP-003 | Fallback explicito, nunca silencioso | Import Lead | 0 degradaciones sin issue | import regression suite | degraded trace map |
| IMP-004 | Ruido de import reducido >=80% story_first | QA Lead | delta issues baseline >=80% | KPI gate F01 | KPI report |
| IMP-005 | Seguridad de paths centralizada | Security Lead | 100% tests traversal pass | security gate | security test logs |

## No objetivos

1. No ejecutar codigo Ren'Py embebido.
2. No modelar DSL de UI completo como canon del motor.

## Evidencia minima

1. `import_report_v2.json` por perfil.
2. comparativo baseline/delta.
3. listado top codes por area.
