# Tipado, Interpretabilidad y Contratos de Diagnostico

Owner: Core Lead + DX Lead

## Problema

1. Tipos de dominio historicamente mezclaron semantica fuerte con payload debil.
2. Mensajes de error dispersos reducen explicabilidad y trazabilidad.

## Objetivo V2

1. Tipado de dominio fuerte y explicito.
2. Diagnosticos uniformes entre core/gui/cli/python.
3. Prohibicion de hardcodeo de errores fuera del catalogo central.

## Contrato DiagnosticEnvelopeV2

1. `diagnostic_id`
2. `trace_id`
3. `phase`
4. `code`
5. `severity`
6. `node_id`
7. `event_ip`
8. `root_cause`
9. `why_failed`
10. `how_to_fix`
11. `docs_ref`
12. `context`

## Reglas de tipado

1. Tipos de dominio no exponen `String` opaco cuando existe enum/struct canonico.
2. Enums de dominio deben serializar de forma estable (`snake_case`).
3. Cambios breaking deben documentar migracion explicita.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| TYP-001 | 0 payload opaco en contratos centrales V2 | Core Lead | 0 campos opacos sin schema | API lint + review | schema diff |
| TYP-002 | DiagnosticEnvelopeV2 consistente en 3 canales | DX Lead | paridad >=95% gui/cli/py | parity test suite | snapshots envelope |
| TYP-003 | Errores operativos sin hardcode aislado | DX Lead | 100% errores en catalogo central | catalog coverage tests | catalog map |
| TYP-004 | Cada error incluye `trace_id` y `code` | Observability Lead | >=95% eventos de error | traceability gate | telemetry report |
| TYP-005 | Migracion breaking documentada | Architecture Council | 100% cambios con tabla de migracion | API gate | migration docs |

## Casos de prueba

1. serializacion/deserializacion de tipos V2.
2. snapshots bilingues de diagnosticos.
3. paridad de campos en gui/cli/python.
4. colision de `diagnostic_id` = 0.

## Resultado esperado

1. Diagnosticos reproducibles y accionables.
2. Menor ambiguedad para usuarios y tooling.
