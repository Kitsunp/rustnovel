# Blueprint de Refactor y Metaprogramacion V2

Owner: Architecture Council

## Objetivo

1. Reducir boilerplate y ambiguedad en contratos.
2. Garantizar catalogos consistentes en core/gui/cli/python.

## Estrategia de metaprogramacion

1. Macros declarativas para catalogos de diagnostico.
2. Traits para contratos de acciones custom.
3. Builders tipados para comandos audio y package.
4. Evitar proc-macros complejas en fase inicial.

## Modulo compartido `contracts`

1. `DiagnosticEnvelopeV2`
2. `ActionSchema`
3. `AudioCommandV2`
4. `ProjectPathContext`
5. `ExportBundleSpec`

## Regla de codificacion

1. No hardcodear mensajes de error fuera del catalogo.
2. No duplicar logica de dominio en adaptadores.
3. Adaptadores solo traducen I/O externo al canon interno.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| META-001 | Catalogo de diagnosticos generado centralmente | DX Lead | 100% codigos en fuente unica | catalog tests | generated catalog map |
| META-002 | ActionRegistry con schema tipado | Core Lead | 100% acciones registradas con schema | action registry tests | registry snapshots |
| META-003 | Reuse de contratos sin duplicacion | Architecture Council | duplicacion dominio inter-capas <5% | static analysis gate | duplication report |
| META-004 | Adaptadores sin logica canonica extra | Architecture Council | 100% revisiones cumplen regla | architecture review gate | design review notes |
| META-005 | Macros mantenibles y auditables | Core Lead | cobertura >=90% en modulos macro | unit tests + docs | macro test report |

## Fases

1. Fase A: contratos base y catalogo central.
2. Fase B: migracion adaptadores gui/cli/py.
3. Fase C: retiro de rutas legacy.
