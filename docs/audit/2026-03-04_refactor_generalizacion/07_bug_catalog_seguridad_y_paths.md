# Seguridad y Politica de Paths Unificada

Owner: Security Lead

## Problema

1. Validaciones de paths historicamente distribuidas en varias capas.
2. Riesgo de divergencia entre import/core/gui/runtime.

## Politica central PathPolicyV2

1. Prohibidos: traversal (`..`), absolutos no autorizados, URL remotas.
2. Resolucion relativa al `ProjectPathContext`.
3. Normalizacion multiplataforma de separadores.
4. Rechazo de symlink fuera de root permitido.

## ProjectPathContext

1. `project_root`
2. `assets_root`
3. `resolver_mode`: `strict | lenient`
4. `allowed_external_roots` (solo cuando aplique)

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| SEC-001 | 100% resolucion de asset via ProjectPathContext | Security Lead | paths por CWD = 0 | path context tests | resolver logs |
| SEC-002 | Traversal/URL/absolutos bloqueados por defecto | Security Lead | 100% casos bloqueados | security unit tests | negative test report |
| SEC-003 | Paridad de politica en core/gui/cli/import | Architecture Council | 100% capas usan policy comun | contract tests | module audit |
| SEC-004 | Symlink escape bloqueado | Security Lead | 100% symlink escape fail | security integration tests | fs sandbox report |
| SEC-005 | Mensaje de rechazo trazable | DX Lead | >=95% rechazos con trace_id+how_to_fix | diagnostics gate | issue snapshots |

## Casos de prueba

1. rutas relativas validas.
2. traversal directo/indirecto.
3. rutas con drive letter windows.
4. symlink a parent externo.
5. urls http/https.
