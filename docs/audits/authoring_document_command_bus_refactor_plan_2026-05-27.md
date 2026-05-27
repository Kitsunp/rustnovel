# AuthoringDocumentCommandBus Refactor Plan - 2026-05-27

## Resumen Ejecutivo

Este plan define la siguiente fase de refactor para unificar la trazabilidad de autoria del repositorio. El objetivo no es agregar una capa por abstraccion teorica, sino eliminar rutas paralelas donde GUI, Python o CLI mutan estado documental y construyen logs, fingerprints o verificaciones por su cuenta.

El estado actual ya separa razonablemente el grafo narrativo mediante `AuthoringCommandBus`, pero `AuthoringDocument` todavia contiene datos editoriales que se modifican fuera de un contrato central:

- `composer_layer_overrides`
- `composer_background_fit_overrides`
- `operation_log`
- `verification_runs`
- fingerprints documentales
- metadata editorial usada por GUI/Python

La meta de esta fase es convertir `AuthoringDocument` en el contrato canonico de autoria completa:

```text
command -> delta -> document update -> operation log -> fingerprint -> verification -> outcome
```

Regla principal:

```text
GUI/Python/CLI envian comandos.
Core aplica comandos.
Core produce deltas, operation logs, fingerprints y verification runs.
GUI/Python/CLI no inventan trazabilidad semantica ni documental.
```

## Problema Que Resuelve

Actualmente existen dos niveles de estado:

### Estado Narrativo: NodeGraph

Representa historia y estructura ejecutable:

- nodos
- conexiones
- choices
- fragments
- scene data narrativa
- mutaciones de grafo
- export runtime

Este nivel debe seguir bajo `AuthoringCommandBus`.

### Estado Documental: AuthoringDocument

Representa estado de autoria completa y experiencia editorial:

- `graph`
- `composer_layer_overrides`
- `composer_background_fit_overrides`
- `operation_log`
- `verification_runs`
- fingerprints
- metadata editorial

Este nivel necesita un bus propio: `AuthoringDocumentCommandBus`.

## Por Que No Meter visible/locked En NodeGraph

`visible` y `locked` no son parte de la narrativa ni del runtime. Son decisiones de authoring/editor:

- `visible`: como el editor presenta una capa durante la composicion.
- `locked`: si el editor debe permitir mover una capa.

Meterlos en `NodeGraph` mezclaria estado editorial con estado narrativo. Eso haria que cambios de UI parecieran cambios de historia, contaminaria fingerprints semanticos y podria afectar export/runtime sin necesidad.

La frontera correcta es:

```text
NodeGraph:
  historia y estructura narrativa.

AuthoringDocument:
  documento completo, incluyendo estado editorial.

Runtime:
  ejecucion de scripts; no depende de UI/editor salvo export explicito.
```

## Evidencia Del Estado Actual

Puntos actuales relevantes:

- `crates/core/src/authoring/document.rs` define `AuthoringDocument` con `graph`, `composer_layer_overrides`, `composer_background_fit_overrides`, `operation_log` y `verification_runs`.
- `crates/core/src/authoring/command_bus.rs` y submodulos controlan mutaciones de `NodeGraph`.
- `crates/core/src/authoring/composer/objects.rs` contiene `set_layer_visible`, `set_layer_locked` y `apply_layer_overrides`.
- `crates/core/src/authoring/report_fingerprint.rs` ya incluye overrides documentales en fingerprint de documento.
- `crates/gui/src/editor/workbench/ui_actions.rs` todavia maneja `handle_layer_visibility_changed`, `handle_layer_lock_changed` y `handle_background_fit_changed` con operacion local.
- `crates/py/src/bindings/editor_node_graph/composer.rs` todavia maneja `py_set_layer_visible` y `py_set_layer_locked` con log construido desde el binding.
- `crates/gui/src/editor/workbench/operation_ops.rs` y `crates/py/src/bindings/editor_node_graph/internal.rs` son sinks centrales de operation log.

El refactor debe conservar sinks centrales solo como mecanismo de compatibilidad o borde de adaptacion, no como lugar donde features concretas inventan trazabilidad.

## Objetivo Arquitectonico

Crear una API de core para mutaciones documentales:

```rust
AuthoringDocumentCommandBus
```

El bus debe:

1. Ser duenio de un `AuthoringDocument`.
2. Aceptar comandos tipados.
3. Delegar comandos de grafo a `AuthoringCommandBus`.
4. Aplicar comandos documentales sin tocar `NodeGraph` cuando no corresponde.
5. Producir deltas reversibles.
6. Producir operation log v2.
7. Producir verification run.
8. Calcular fingerprints before/after desde core.
9. Rechazar no-ops sin crear logs falsos.
10. Permitir replay headless.

## Uso Coherente En Componentes Consumidores

El bus documental no debe quedar como una API nueva que solo usan los tests de core. El objetivo real es que los consumidores que hoy editan estado de autoria usen el mismo contrato y dejen de construir trazabilidad por su cuenta.

Componentes que deben consumirlo:

- GUI/editor: cambios de `composer_layer_overrides`, `composer_background_fit_overrides`, visible/locked, background fit y mutaciones documentales equivalentes.
- Python bindings: metodos de autoria que cambian overrides documentales o exponen operation log/verification runs.
- CLI authoring: comandos headless que apliquen mutaciones documentales, expliquen cambios o serialicen outcomes.
- Export/validation/reporting: lectura de fingerprints, stale status, operation ids y diagnostics generados por el mismo flujo core.
- Tests compartidos: fixtures que comparen core, GUI, Python y CLI contra el mismo resultado observable.

Componentes que no deben consumirlo directamente:

- Runtime de ejecucion: debe consumir scripts runtime, compiled scripts o planes de export, no comandos editoriales.
- Render/audio runtime: solo deben ver estado ya materializado, no decidir operation logs ni fingerprints de autoria.
- Estado visual transitorio de UI: seleccion, hover, viewport, scroll, foco y drag preview pueden seguir siendo locales mientras no muten `AuthoringDocument`.

Regla de integracion:

```text
Si una accion modifica AuthoringDocument o su trazabilidad,
debe pasar por AuthoringDocumentCommandBus.

Si una accion solo modifica estado transitorio de UI,
no debe contaminar AuthoringDocument ni operation_log.
```

Esto evita que existan "versiones por ocasion" del mismo comportamiento. Una mutacion documental debe tener el mismo `OperationKind`, `field_path`, fingerprint, verification run y delta sin importar si viene desde GUI, Python, CLI o un test headless.

## Contrato Publico Propuesto

### Modulos

```text
crates/core/src/authoring/document_command_bus.rs
crates/core/src/authoring/document_command_bus/types.rs
crates/core/src/authoring/document_command_bus/apply.rs
crates/core/src/authoring/document_command_bus/inverse.rs
crates/core/src/authoring/document_command_bus/replay.rs
```

### Comandos

```rust
pub enum AuthoringDocumentCommand {
    Graph(AuthoringCommand),

    SetLayerVisible {
        object_id: String,
        visible: bool,
    },

    SetLayerLocked {
        object_id: String,
        locked: bool,
    },

    SetBackgroundFitOverride {
        node_id: u32,
        fit: BackgroundFit,
    },

    ClearBackgroundFitOverride {
        node_id: u32,
    },
}
```

No empezar con un catalogo enorme. Primero se deben migrar los comandos que hoy generan duplicacion real: layer visibility, layer lock y background fit.

### Deltas

```rust
pub enum AuthoringDocumentDelta {
    Graph(AuthoringDelta),

    LayerVisibleChanged {
        object_id: String,
        before: Option<LayerOverride>,
        after: Option<LayerOverride>,
    },

    LayerLockedChanged {
        object_id: String,
        before: Option<LayerOverride>,
        after: Option<LayerOverride>,
    },

    BackgroundFitChanged {
        node_id: u32,
        before: Option<BackgroundFit>,
        after: Option<BackgroundFit>,
    },

    BackgroundFitCleared {
        node_id: u32,
        before: Option<BackgroundFit>,
    },

    Reverted {
        reverted: Box<AuthoringDocumentDelta>,
    },
}
```

Los deltas deben guardar suficiente informacion para undo/replay sin snapshots completos por accion.

### Outcome

```rust
pub struct AuthoringDocumentCommandOutcome {
    pub delta: AuthoringDocumentDelta,
    pub operation: OperationLogEntry,
    pub verification: VerificationRun,
    pub before_fingerprint: AuthoringReportFingerprint,
    pub after_fingerprint: AuthoringReportFingerprint,
}
```

### Bus

```rust
pub struct AuthoringDocumentCommandBus {
    document: AuthoringDocument,
    undo_deltas: Vec<AuthoringDocumentDelta>,
    redo_deltas: Vec<AuthoringDocumentDelta>,
    recorded_commands: Vec<AuthoringDocumentCommand>,
}
```

Metodos esperados:

```rust
impl AuthoringDocumentCommandBus {
    pub fn new(document: AuthoringDocument) -> Self;
    pub fn replay(commands: &[AuthoringDocumentCommand]) -> Result<Self, String>;
    pub fn document(&self) -> &AuthoringDocument;
    pub fn into_document(self) -> AuthoringDocument;
    pub fn recorded_commands(&self) -> &[AuthoringDocumentCommand];
    pub fn undo_delta_count(&self) -> usize;
    pub fn redo_delta_count(&self) -> usize;
    pub fn apply(&mut self, command: AuthoringDocumentCommand)
        -> Result<AuthoringDocumentCommandOutcome, String>;
}
```

## Reglas De Implementacion Rust

1. Usar tipos concretos, no strings genericos para comandos.
2. Evitar lifetimes complejos si no agregan valor; `String` en comandos publicos es aceptable para API/Python.
3. `Graph(command)` debe componer `AuthoringCommandBus`; no debe duplicar logica de grafo.
4. Los comandos documentales no deben mutar `document.graph`.
5. Los comandos de grafo no deben mutar overrides documentales salvo contrato explicito.
6. Los no-ops deben devolver error o estado `NoOp` tipado, pero nunca agregar log falso como `Applied`.
7. El bus debe ser deterministicamente serializable/replayable si se decide persistir comandos.
8. Los helpers de `composer` pueden seguir existiendo, pero la decision de trazabilidad debe vivir en el bus.
9. No introducir `unwrap`/`expect` en rutas de usuario; solo tests o invariantes justificadas.
10. No usar `std::env::current_dir()` para resolver proyectos, assets o fixtures.

## Semantica De Fingerprints

El plan debe mantener separacion entre fingerprint semantico y fingerprint documental.

### SetLayerVisible

Debe:

- cambiar fingerprint documental si el valor cambia.
- no cambiar fingerprint semantico del grafo.
- no cambiar script runtime exportado.
- producir `OperationKind::LayerVisibilityChanged`.
- producir field path estable: `composer.layers[{object_id}].visible` o equivalente canonico.

### SetLayerLocked

Debe:

- cambiar fingerprint documental si el valor cambia.
- no cambiar fingerprint semantico del grafo.
- no cambiar script runtime exportado.
- bloquear acciones editoriales que respeten locked.
- producir `OperationKind::LayerLockChanged`.

### SetBackgroundFitOverride

Debe:

- cambiar fingerprint documental.
- no cambiar `NodeGraph`.
- afectar preview/composer layout cuando se consume el documento.
- no afectar runtime salvo que export incluya metadata editorial explicitamente.
- producir `OperationKind::FieldEdited` o una variante v2 dedicada si se agrega.

### Graph(EditDialogue)

Debe:

- cambiar `NodeGraph`.
- cambiar fingerprint semantico.
- cambiar script runtime exportado.
- delegar en `AuthoringCommandBus`.
- devolver `AuthoringDocumentDelta::Graph`.

## Plan De Migracion

### Fase 1: Core Contract

Crear el bus documental y sus tipos.

Archivos probables:

- `crates/core/src/authoring/document_command_bus.rs`
- `crates/core/src/authoring/document_command_bus/types.rs`
- `crates/core/src/authoring/document_command_bus/apply.rs`
- `crates/core/src/authoring/document_command_bus/inverse.rs`
- `crates/core/src/authoring/document_command_bus/replay.rs`
- `crates/core/src/authoring/mod.rs`

Resultado esperado:

- El crate core expone `AuthoringDocumentCommandBus`.
- No hay cambios GUI/Python aun.
- Tests core pasan.

### Fase 2: Tests Core Duros

Agregar tests:

- `document_command_bus_layer_visible_trace`
- `document_command_bus_layer_locked_trace`
- `document_command_bus_background_fit_trace`
- `document_command_bus_clear_background_fit_trace`
- `document_command_bus_graph_command_delegates_to_authoring_bus`
- `document_command_bus_replay_headless`
- `document_command_bus_rejects_noop_without_fake_log`
- `document_command_bus_fingerprint_semantic_vs_document_split`
- `document_command_bus_undo_redo_delta_contract`

Los tests deben verificar estado real del documento, no solo que una funcion devuelve `Ok`.

### Fase 3: Python

Migrar:

- `py_set_layer_visible`
- `py_set_layer_locked`
- APIs documentales relacionadas con background fit si estan expuestas o se exponen.

Regla:

- Python no construye `OperationLogEntry` para estas mutaciones.
- Python llama al bus documental y copia el documento resultante a su wrapper.
- Python expone los mismos logs/verifications producidos por core.

Tests:

- `python_document_command_layer_visible_trace`
- `python_document_command_layer_locked_trace`
- `python_document_command_no_global_site_package_origin`
- `python_document_command_core_parity`

### Fase 4: GUI

Migrar:

- `handle_layer_visibility_changed`
- `handle_layer_lock_changed`
- `handle_background_fit_changed`
- cualquier ruta equivalente que edite overrides documentales.

Regla:

- GUI puede manejar estado visual transitorio, seleccion, viewport y hover.
- GUI no decide operation kind, operation id, fingerprints ni verification runs para mutaciones documentales.

Tests:

- `gui_layer_visible_uses_document_command_bus`
- `gui_layer_locked_uses_document_command_bus`
- `gui_background_fit_uses_document_command_bus`
- `gui_document_command_preserves_preview_behavior`

### Fase 5: Eliminar Duplicacion

Despues de core/Python/GUI:

- Reducir usos feature-specific de `OperationLogEntry::new_typed` en GUI/Python.
- Reducir usos feature-specific de `operation_log.push`.
- Mantener sinks centrales solo como adaptadores temporales o para operaciones fuera del bus.

### Fase 6: Hardening Anti-Simulacion

Agregar tests que fallen si alguien solo simula trazabilidad.

Cada test debe comprobar:

- cambio real de `AuthoringDocument`.
- delta real del bus.
- operation log real producido por core.
- verification run real producido por core.
- fingerprints before/after correctos.
- no cambio semantico cuando el comando es documental.
- cambio semantico cuando el comando es de grafo.

## Criterios De Aceptacion Duros

### Codigo

- Ningun archivo Rust/Python de codigo o test supera 500 lineas.
- No hay tests dentro de `src`.
- No hay nuevos `#[cfg(test)] mod tests` dentro de `src`.
- No hay nuevos `#[path = "...tests..."]` dentro de `src`.
- No hay `OperationLogEntry::new_typed` en GUI/Python salvo sinks centrales aprobados.
- No hay `operation_log.push` en features concretas.
- No hay `std::env::current_dir()` en resolucion de assets, documentos o comandos.
- No hay sleeps/timeouts para hacer pasar tests.
- No hay skips nuevos.
- No hay mocks que oculten mutaciones reales del core.
- No hay `unwrap`/`expect` nuevos en runtime de usuario salvo invariantes documentadas.

### Trazabilidad

- Todo comando aplicado produce `operation_id` unico.
- Todo comando aplicado produce `before_fingerprint`.
- Todo comando aplicado produce `after_fingerprint`.
- Todo comando aplicado produce `verification_run`.
- Toda operacion usa `OperationKind` v2.
- Toda operacion tiene `field_path` o `DiagnosticTarget` cuando aplica.
- Todo no-op se rechaza o se registra como `NoOp` tipado sin mutar documento.
- No hay logs `Applied` para comandos que no cambiaron estado.
- Replay de comandos produce documento equivalente.
- Undo/redo usa deltas, no snapshots completos por accion.

### Anti-Hardcode

- Tests usan `tempdir` o `project_root` inyectado.
- Tests no dependen de `C:\`, `/tmp`, usuario, locale, timezone o `cwd`.
- Paths absolutos solo aparecen en fixtures de seguridad controlados.
- Comparaciones de paths normalizan separadores cuando el contrato es multiplataforma.
- Fingerprints excluyen campos volatiles.
- Snapshots normalizan `target_os`, `target_arch`, timestamps y paths si aparecen.

### Anti-Simulacion

- GUI/Python deben ejecutar comandos reales de core.
- Tests de GUI/Python deben verificar que el log resultante viene del outcome/core, no de construccion local.
- Tests deben validar cambios en `AuthoringDocument`, no solo strings de output.
- Tests deben verificar que el script runtime no cambia para comandos documentales.
- Tests deben verificar que el script runtime si cambia para `Graph(...)` cuando corresponde.

### Paridad

- Core, GUI y Python producen el mismo `OperationKind` para la misma mutacion.
- Core, GUI y Python producen field paths equivalentes.
- Core, GUI y Python preservan stale status.
- Core, GUI y Python clasifican fingerprints igual: semantico vs documental.
- CLI, si expone document commands, serializa el mismo outcome que Python/GUI.

### Adopcion Real En Componentes

- Ninguna ruta GUI/Python/CLI que modifique `AuthoringDocument` puede saltarse el bus documental.
- `set_layer_visible`, `set_layer_locked` y background fit deben tener un unico flujo core observable desde GUI/Python.
- Los tests deben fallar si GUI/Python actualizan mapas locales pero no reciben `AuthoringDocumentCommandOutcome`.
- Los tests deben fallar si CLI serializa un outcome construido a mano en vez del producido por core.
- Los sinks legacy permitidos deben estar documentados por nombre, con comentario de migracion y test que demuestre por que siguen existiendo.
- `rg "OperationLogEntry::new_typed" crates/gui/src crates/py/src tools/cli/src` debe mostrar solo adaptadores aprobados; cualquier uso nuevo feature-specific bloquea aceptacion.
- `rg "operation_log\\.push" crates/gui/src crates/py/src tools/cli/src` debe mostrar solo sinks aprobados; no handlers concretos.
- `rg "composer_layer_overrides|composer_background_fit_overrides" crates/gui/src crates/py/src tools/cli/src` debe revisarse en cada PR para confirmar que las escrituras pasan por el bus.
- La documentacion publica debe indicar que `AuthoringDocumentCommandBus` es la unica API soportada para mutaciones documentales persistentes.

## Comandos De Verificacion

Rust:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets --no-fail-fast
```

Audit:

```powershell
cargo audit -D warnings --ignore RUSTSEC-2024-0436
```

Python:

```powershell
py -m ruff format --check .
py -m ruff check .
py -m mypy
target\py-audit-venv\Scripts\maturin develop --manifest-path crates\py\Cargo.toml --features extension-module
target\py-audit-venv\Scripts\python -m pytest -q
```

Static gates:

```powershell
rg "OperationLogEntry::new_typed" crates/gui/src crates/py/src
rg "operation_log\.push" crates/gui/src crates/py/src
rg "current_dir\(" crates
rg "#\[cfg\(test\)\]|mod tests|#\[path\s*=\s*\".*tests" crates/**/src tools/cli/src
```

Line policy:

```powershell
$files = rg --files --glob '*.rs' --glob '*.py' --glob '!target/**' --glob '!**/.venv/**'
foreach ($file in $files) {
  $count = (Get-Content -LiteralPath $file | Measure-Object -Line).Lines
  if ($count -gt 500) { "$count`t$file" }
}
```

## Riesgos Y Decisiones

### Riesgo: Duplicar AuthoringCommandBus

Mitigacion:

- `AuthoringDocumentCommandBus::Graph(command)` debe delegar en `AuthoringCommandBus`.
- No reimplementar mutaciones de grafo en el bus documental.

### Riesgo: Contaminar NodeGraph Con Estado Editorial

Mitigacion:

- `visible/locked` y background fit permanecen en `AuthoringDocument`.
- Tests contrastivos deben probar que el script runtime no cambia.

### Riesgo: Logs Sinteticos Paralelos

Mitigacion:

- GUI/Python no crean logs para document commands.
- Sinks centrales solo se conservan temporalmente.
- Static gates deben fallar si aparecen logs feature-specific nuevos.

### Riesgo: Tests Verdes Pero Simulados

Mitigacion:

- Tests deben inspeccionar documento final, delta, log, verification y fingerprints.
- No basta con validar stdout o que una funcion retorne `true`.

### Riesgo: Exceso De Abstraccion

Mitigacion:

- Empezar con 5 comandos reales.
- No migrar todo el mundo en una sola fase.
- Cada comando nuevo requiere test core + paridad si cruza GUI/Python.

## Definition Of Done

El refactor se considera terminado solo si:

1. `AuthoringDocumentCommandBus` existe y esta expuesto desde `visual_novel_engine::authoring`.
2. `Graph(AuthoringCommand)` delega en `AuthoringCommandBus`.
3. `SetLayerVisible`, `SetLayerLocked`, `SetBackgroundFitOverride` y `ClearBackgroundFitOverride` funcionan con deltas.
4. Python usa el bus documental para layer overrides.
5. GUI usa el bus documental para layer overrides y background fit.
6. Los logs y verification runs para esas mutaciones nacen en core.
7. Tests core, Python y GUI prueban paridad.
8. No hay logs falsos para no-ops.
9. No hay dependencias de entorno o paths hardcodeados.
10. Pasan todos los comandos de verificacion.

## Prioridad Recomendada

1. Crear `AuthoringDocumentCommandBus`.
2. Agregar tests core de delta/fingerprint/log/replay.
3. Migrar Python.
4. Migrar GUI.
5. Eliminar logs paralelos.
6. Documentar snapshot de dependencias duplicadas como tema separado.

La trazabilidad inconsistente es el problema arquitectonico. Las dependencias duplicadas son limpieza operativa; no deben bloquear este refactor salvo que impidan validacion o empaquetado.
