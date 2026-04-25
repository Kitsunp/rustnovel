# UI Performance y AutoFix Batch

Owner: GUI Lead

## Problema

1. Auto-fix masivo puede causar lag por recompilaciones repetidas.
2. Render de paneles grandes requiere control de presupuesto p95.

## Estrategia V2

1. Batch transaccional con preview y commit final.
2. Una compilacion final por lote (evitar N recompilaciones).
3. Progreso/cancelacion visibles.
4. Virtualizacion para listas grandes de issues.

## SLO objetivo

1. Input lag p95 < 120ms durante batch.
2. Apertura lint panel con 10k issues < 100ms.
3. Mejora de auto-fix >=3x vs baseline.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| UIP-001 | Batch auto-fix transaccional obligatorio | GUI Lead | 100% lotes con preview+commit | UI integration tests | batch logs |
| UIP-002 | Recompilacion final unica por lote | GUI Lead | <=1 compile final por lote | perf test suite | profiling traces |
| UIP-003 | SLO input lag p95 <120ms | Perf Lead | p95 medido en stress suite | perf gate | perf report |
| UIP-004 | Lint panel 10k issues <100ms | Perf Lead | tiempo inicial <100ms | UI perf benchmark | benchmark artifact |
| UIP-005 | Cancelacion segura sin corrupcion | GUI Lead | 100% cancel tests pasan | UI regression suite | rollback traces |

## Casos de prueba

1. lote 100/500/1000 issues.
2. cancelacion al 25/50/75%.
3. comparacion baseline vs V2.
4. stress en grafo grande con diff abierto.

## DoD

1. KPIs de performance en verde.
2. No perdida de trazabilidad en auto-fix.
