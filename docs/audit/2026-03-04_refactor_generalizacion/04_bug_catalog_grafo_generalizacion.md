# Grafo, Generalizacion y ActionRegistry V2

Owner: Graph Lead

## Problema

1. Nodos genericos y acciones custom pueden introducir ambiguedad de ejecucion.
2. Reglas de puertos y transiciones necesitan contrato formal.

## GraphContractV2 (normativo)

1. Invariantes de entrada:
2. exactamente un `Start` exportable.
3. rutas de `Choice` completas por opcion.
4. puertos validados por tipo de nodo.
5. idempotencia roundtrip `script <-> graph`.

## ActionRegistry V2

1. `ActionId`, `ActionSchema`, `ActionExecutionPolicy`.
2. Politicas permitidas: `AutoAdvance`, `RequiresResume`, `AsyncAwait`.
3. Toda accion custom declara precondiciones y postcondiciones.

## Politica de nodos genericos

1. Nodo generico solo permitido como preview-only, o
2. se migra a nodo tipado exportable.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| GRF-001 | Roundtrip graph/script idempotente | Graph Lead | 100% fixtures estables | graph roundtrip tests | snapshot diffs |
| GRF-002 | Choice sin opcion unlinked en export | Graph Lead | 0 errores unlinked en release | graph validation gate | validation report |
| GRF-003 | Action policy sin bloqueos de flujo | Runtime Lead | 0 stalls por action | runtime integration tests | ext/action flow traces |
| GRF-004 | Nodo generico sin ambiguedad export | Product Eng | 100% generic nodes clasificados | export contract tests | contract audit |
| GRF-005 | Reglas de puertos por tipo de nodo | Graph Lead | 100% conexiones invalidas detectadas | validator tests | lint snapshots |

## Casos de prueba obligatorios

1. ciclos alcanzables con salida controlada.
2. choices multinivel con rutas divergentes.
3. acciones custom sync/async con resume policy.
4. grafo grande con 10k edges para estabilidad de validacion.
