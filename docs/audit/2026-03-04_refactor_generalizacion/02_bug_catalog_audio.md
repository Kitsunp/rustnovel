# Catalogo Audio + Contrato Audio V2

Owner: Runtime Lead

## Problema estructural

1. Contrato audio previo tenia perdida de semantica entre core y runtime.
2. Canales y acciones no estaban tipados de forma interpretable extremo a extremo.

## Contrato Audio V2 (normativo)

### Tipos

1. `AudioChannel`: `Bgm | Sfx | Voice | Custom(String)`.
2. `AudioKind`: `Play | Stop | FadeOut | Queue | SetVolume`.
3. `AudioCommandV2`: `{channel, kind, asset, volume, fade_ms, loop_mode}`.

### Reglas

1. No degradar silenciosamente parametros (`loop`, `fade`, `volume`).
2. Todo comando aplicado retorna `AudioApplyResult` con `trace_id`.
3. `voice` mantiene canal separado de `sfx`.

## Brechas auditadas (resumen)

1. AUDIO-001: contrato core-runtime incompleto.
2. AUDIO-002: loop BGM forzado.
3. AUDIO-003: fade/volume no consistente.
4. AUDIO-004: canal voice degradado.
5. AUDIO-005: riesgo de bloqueo por flujo custom action.
6. AUDIO-006: tipado opaco en dominio.

## Regla de migracion

1. Se permite breaking directo de `AudioCommand` legacy.
2. Se debe proveer tabla de migracion en `13_public_api_v2_contract.md`.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| AUD-001 | 0 perdida silenciosa de `loop/fade/volume` | Runtime Lead | >=95% paridad audio | `audio_runtime_parity_tests` | parity report |
| AUD-002 | `voice` canal independiente obligatorio | Runtime Lead | 100% casos voice pasan | runtime integration suite | audio contract tests |
| AUD-003 | Tipado de dominio sin `u8` opaco en API V2 | Core Lead | 0 contratos audio opacos | API lint + review | API schema diff |
| AUD-004 | Resultado aplicado trazable | Observability Lead | 100% comandos con `trace_id` | runtime trace gate | trace snapshots |
| AUD-005 | Error explicable sin hardcode aislado | DX Lead | 100% errores en catalogo central | diagnostics catalog tests | catalog coverage report |

## Casos de prueba obligatorios

1. BGM loop=true/false.
2. FadeOut con duracion minima/maxima.
3. Voice + Sfx simultaneo con stops selectivos.
4. SetVolume por canal.
5. Queue con politica de orden estable.

## No objetivos

1. No replicar efectos propietarios de DSL externos.
2. No introducir heuristicas ocultas sin diagnostico.
