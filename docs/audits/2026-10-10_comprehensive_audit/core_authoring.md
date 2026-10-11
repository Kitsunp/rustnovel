# Reporte de Auditoría: Core Authoring & Ren'Py Import

- **Crate**: `crates/core` (`visual_novel_engine`)
- **Áreas Auditadas**:
  - `src/authoring/**` (`compiler`, `command_bus`, `document_command_bus`, `validation`, `script_sync`, `composer`, `quick_fix`, `diagnostics`, `graph`, `lint`)
  - `src/renpy_import/**` (`parser`, `flow`, `syntax`, `assets`, `postprocess`, `types`, `decorators`)
- **Fecha**: 11 de Octubre de 2026
- **Auditor**: Senior Rust Engine Auditor

---

## 1. Resumen Ejecutivo

Durante la auditoría exhaustiva de la capa de autoría (`authoring`) y del importador de scripts Ren'Py (`renpy_import`), se diseñó e implementó un conjunto de pruebas adversarias de integración en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs), evaluando:
1. Ciclos y bucles en grafos y sus implicaciones en compilación y simulación.
2. Eliminación de nodos con puertos y conexiones activas en el command bus.
3. Consistencia de la máquina de estados de undo/redo ante fallos parciales o irreversibles.
4. Quick-fixes que generan scripts inválidos ante la exportación estricta y violaciones de idempotencia.
5. Código hostil en el importador de Ren'Py (indentaciones anómalas, nombres de labels con emojis/no-ASCII, y rutas de assets maliciosas con path traversal).
6. Colisiones en identificadores de diagnósticos y no-determinismo en fingerprints de grafo.

Se identificaron **10 bugs confirmados mediante tests ejecutables**, destacando fallos lógicos graves como el *fall-through* no deseado entre opciones de menú de Ren'Py, la persistencia de rutas maliciosas en scripts importados, la corrupción del stack de deshecho (`undo_deltas`) en el command bus, y la generación por parte de quick-fixes de opciones que son explícitamente rechazadas por el validador estricto.

---

## 2. Bugs CONFIRMADOS

### BUG-AUTH-01: Fall-through no deseado en opciones de `menu:` del importador Ren'Py
- **Severidad**: Alta
- **Archivo:Línea**: [crates/core/src/renpy_import/flow.rs:132-147](file:///D:/Rust/audit/rustnovel/crates/core/src/renpy_import/flow.rs#L132-L147)
- **Descripción**:
  Al analizar bloques `menu:` en scripts de Ren'Py, el importador crea un evento `Choice` y posteriormente itera sobre las opciones emitiendo secuencialmente los eventos de cada bloque en `state.events`. Sin embargo, a diferencia de `parse_if_chain` (que genera una etiqueta `ifend` e inserta un `Jump` al final de cada rama), `parse_menu` no genera ninguna etiqueta de finalización del menú ni inserta un `Jump` al terminar el bloque de una opción.
  
  En tiempo de ejecución, cuando el jugador selecciona la primera opción, una vez finalizado su diálogo el flujo de instrucciones continúa linealmente y cae (*fall-through*) directamente dentro del cuerpo de la segunda opción, ejecutando diálogos y mutaciones de opciones que el usuario nunca seleccionó.
- **Test que lo reproduce**:
  `test_renpy_import_menu_falls_through_between_options` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L390-L450)
  ```
  Salida resumida:
  running 1 test
  test test_renpy_import_menu_falls_through_between_options ... ok
  (Verificó que al elegir Opcion Alfa, se ejecuta secuencialmente 'Elegiste Beta')
  ```
- **Fix sugerido**:
  En `crates/core/src/renpy_import/flow.rs`, generar una etiqueta sintética `menu_end = self.next_synthetic_label("menuend");` e insertar al final de cada bloque de opción un `self.events.push(EventRaw::Jump { target: menu_end.clone() });`, y finalmente insertar la etiqueta `self.labels.insert(menu_end, self.events.len());` después de compilar todas las opciones.

---

### BUG-AUTH-02: Rutas de assets inseguras (path traversal y absolutas) se conservan intactas en eventos importados
- **Severidad**: Alta / Seguridad
- **Archivo:Línea**: [crates/core/src/renpy_import/assets.rs:188-196, 264](file:///D:/Rust/audit/rustnovel/crates/core/src/renpy_import/assets.rs#L188-L196)
- **Descripción**:
  Cuando `rewrite_path_and_copy` procesa un asset referenciado en eventos (`Scene`, `Patch`, `AudioAction`), si `normalize_asset_rel_path` detecta componentes inseguros como `Component::ParentDir` (`../../`) o `Component::RootDir` / `Prefix` (`C:\Windows\...`), emite un warning mediante `push_asset_issue` y retorna anticipadamente.
  
  Al retornar sin modificar `*path_value`, la ruta original **insegura y maliciosa permanece intacta** dentro del `EventRaw` que se serializa en `main.json`. Esto permite que un script importado preserve rutas con evasión de directorio o acceso a archivos del sistema operativo anfitrión.
- **Test que lo reproduce**:
  `test_renpy_import_retains_unsafe_asset_paths_in_events` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L452-L510)
  ```
  Salida resumida:
  test test_renpy_import_retains_unsafe_asset_paths_in_events ... ok
  (Verificó que "../../windows/system32/evil.png" y "C:/Windows/System32/calc.exe" persisten en el json importado)
  ```
- **Fix sugerido**:
  En `rewrite_path_and_copy`, en caso de error de normalización de ruta, neutralizar `*path_value = String::new();` o asignar una ruta inocua de reemplazo (como `assets/missing_asset.png`), o alternativamente abortar la importación cuando `strict_mode` o la política de seguridad esté activada.

---

### BUG-AUTH-03: Corrupción del stack de deshacer (`undo_deltas`) cuando `revert_last` falla
- **Severidad**: Alta
- **Archivo:Línea**: [crates/core/src/authoring/command_bus.rs:197-203](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus.rs#L197-L203)
- **Descripción**:
  En `AuthoringCommandBus::revert_last`, se extrae el último delta mediante:
  ```rust
  let delta = self.undo_deltas.pop().ok_or_else(...)?;
  self.apply_inverse_delta(&delta)?;
  ```
  Si `self.apply_inverse_delta(&delta)` falla y retorna `Err` (lo cual ocurre por diseño con variantes como `AuthoringDelta::BranchConnected` y `AuthoringDelta::QuickFixApplied`, las cuales retornan explícitamente error indicando que no pueden revertirse como un delta único de grafo), el delta extraído **no se reinserta** en `undo_deltas`.
  
  Como consecuencia, `undo_deltas.len()` disminuye de forma espuria, el delta que no pudo deshacerse se pierde permanentemente, y una subsecuente invocación a `revert_last` intentará revertir un comando anterior desfasado, dejando el grafo y el historial en un estado inconsistente y corrupto. (Nótese que en `AuthoringDocumentSession::revert_last`, línea 224, sí se implementó `self.undo_deltas.push(delta)` en caso de error, evidenciando el olvido en `AuthoringCommandBus`).
- **Test que lo reproduce**:
  `test_failed_revert_last_corrupts_undo_history` y `test_failed_revert_of_quick_fix_corrupts_undo_history` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L220-L300)
  ```
  Salida resumida:
  test test_failed_revert_last_corrupts_undo_history ... ok
  test test_failed_revert_of_quick_fix_corrupts_undo_history ... ok
  ```
- **Fix sugerido**:
  En `AuthoringCommandBus::revert_last`:
  ```rust
  if let Err(error) = self.apply_inverse_delta(&delta) {
      self.undo_deltas.push(delta);
      return Err(error);
  }
  ```

---

### BUG-AUTH-04: El quick-fix recomendado genera texto de placeholder que rompe la exportación estricta
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/quick_fix.rs:237, 295](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/quick_fix.rs#L237) vs [crates/core/src/authoring/script_sync/export_validation.rs:182](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/script_sync/export_validation.rs#L182)
- **Descripción**:
  Cuando un nodo `Choice` carece de opciones, el linter emite `LintCode::ChoiceNoOptions` y sugiere el quick-fix `choice_add_default_option` o `choice_add_default_option_to_end`. Al aplicarse, la función añade la opción `"Option 1"`. Asimismo, `expand_choice_options_to_ports` agrega opciones con el formato `format!("Option {}", next)`.
  
  Sin embargo, en `validate_strict_graph_export`, se valida:
  ```rust
  if option.trim() == format!("Option {}", port + 1) {
      return Err(VnError::invalid_script(format!(
          "choice node {node_id} option {port} still uses placeholder text"
      )));
  }
  ```
  Como resultado, aplicar el quick-fix sugerido por la interfaz deja el grafo en un estado que **falla fatalmente la exportación estricta** (`to_script_strict`).
- **Test que lo reproduce**:
  `test_quick_fix_choice_add_default_option_breaks_strict_export` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L340-L388)
  ```
  Salida resumida:
  test test_quick_fix_choice_add_default_option_breaks_strict_export ... ok
  (Verificó que to_script_strict falla con "choice node 1 option 0 still uses placeholder text")
  ```
- **Fix sugerido**:
  Hacer que el quick-fix use textos semánticos no sujetos a la regla de placeholder (por ejemplo `"Continuar"` o `"Nueva opción"`), o bien permitir que la exportación estricta tolere la opción si fue marcada por el sistema de autoría como resultado de una auto-reparación.

---

### BUG-AUTH-05: Falsa alarma de error crítico en Dry Run ante ciclos narrativos válidos
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/compiler/repro.rs:107-126](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/compiler/repro.rs#L107-L126) y [crates/core/src/authoring/compiler/dry_run.rs:25-37](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/compiler/dry_run.rs#L25-L37)
- **Descripción**:
  En novelas visuales es común tener bucles narrativos intencionales (por ejemplo, hubs de exploración donde el usuario vuelve al mismo diálogo).
  Al ejecutar `compile_authoring_graph`:
  1. `simulate_raw_sequence` detecta el bucle en el segundo recorrido y se detiene correctamente en 2 pasos con `RawSimulationStopReason::LoopDetected`.
  2. `run_dry_run` (basado en runtime) **no tiene detección de loops** y ejecuta iteraciones hasta el límite artificial de 2048 pasos (`DRY_RUN_MAX_STEPS`), deteniéndose con `DryRunStopReason::StepLimit`.
  3. La función `check_preview_runtime_parity` compara la cantidad de pasos de runtime (2048) con la de raw simulation (2). Al notar la discrepancia, genera un `LintIssue::error` con código `DryRunParityMismatch` ("Parity length mismatch [route=first]: preview=2 runtime=2048").
  
  Debido a este error de severidad `Error`, la traza de fase `CompilationPhase::DryRun` se marca como `ok: false`, bloqueando la validación de compilación del proyecto aunque el grafo sea semánticamente correcto.
- **Test que lo reproduce**:
  `test_story_graph_cycle_causes_dry_run_parity_error_and_phase_failure` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L28-L88)
  ```
  Salida resumida:
  test test_story_graph_cycle_causes_dry_run_parity_error_and_phase_failure ... ok
  ```
- **Fix sugerido**:
  Implementar detección de estado repetido en `run_dry_run` idéntica a la de `simulate_raw_sequence`, y en `check_preview_runtime_parity` no emitir un error de longitud si ambas simulaciones terminaron por detección de bucle o límite de pasos por ciclo.

---

### BUG-AUTH-06: Pérdida definitiva de bookmarks al eliminar y deshacer la eliminación de un nodo
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/graph.rs:117](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/graph.rs#L117) y [crates/core/src/authoring/command_bus/inverse.rs:11-28](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus/inverse.rs#L11-L28)
- **Descripción**:
  En `NodeGraph::remove_node(id)`, los marcadores (`bookmarks`) asociados al nodo son purgados con `self.bookmarks.retain(|_, target| *target != id)`.
  Sin embargo, el delta `AuthoringDelta::NodeRemoved` generado por el command bus únicamente almacena `node_id`, `node`, `position` y `connections`.
  Cuando el usuario ejecuta deshacer (`RevertLast`), `apply_inverse_delta` restaura el nodo y sus conexiones, pero **los bookmarks nunca son restaurados**.
- **Test que lo reproduce**:
  `test_remove_node_with_active_connections_leaves_dangling_ports_and_loses_bookmarks_on_undo` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L90-L168)
  ```
  Salida resumida:
  test test_remove_node_with_active_connections_leaves_dangling_ports_and_loses_bookmarks_on_undo ... ok
  ```
- **Fix sugerido**:
  Incluir `bookmarks: Vec<(String, u32)>` dentro de `AuthoringDelta::NodeRemoved` y reinsertarlos en `apply_inverse_delta`.

---

### BUG-AUTH-07: Estado huérfano en `composer_background_fit_overrides` al eliminar nodos
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/document_command_bus/apply.rs:35-51](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/document_command_bus/apply.rs#L35-L51)
- **Descripción**:
  `AuthoringDocument` almacena `composer_background_fit_overrides: BTreeMap<String, BackgroundFit>` indexado por el ID del nodo en formato cadena (`node_id.to_string()`).
  Al ejecutar `AuthoringDocumentCommand::Graph(AuthoringCommand::RemoveNode { node_id })`, el comando solo muta el `NodeGraph`. La tabla `composer_background_fit_overrides` no se limpia, manteniendo entradas huérfanas hacia nodos eliminados. Si posteriormente se reutiliza o crea un nodo con el mismo ID, este hereda un ajuste de fondo no intencionado.
- **Test que lo reproduce**:
  `test_document_session_remove_node_orphans_background_fit_overrides` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L170-L208)
  ```
  Salida resumida:
  test test_document_session_remove_node_orphans_background_fit_overrides ... ok
  ```
- **Fix sugerido**:
  En `AuthoringDocumentSession::apply_without_logging`, interceptar `RemoveNode` y eliminar la clave en `composer_background_fit_overrides`, emitiendo el delta correspondiente para permitir su restauración en undo.

---

### BUG-AUTH-08: Colisiones de `diagnostic_id` debido a normalización con pérdida
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/lint.rs:394-405](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/lint.rs#L394-L405)
- **Descripción**:
  La función `normalize_diagnostic_part` convierte cualquier carácter que no sea alfanumérico ASCII a `_`, luego compacta `__` a `_` y recorta los guiones bajos al inicio y final.
  Como consecuencia:
  - `"bg/hero-view.png"`, `"bg/hero_view.png"`, `"bg.hero.view.png"` y `"bg/hero_🚀_view.png"` normalizan exactamente a `"bg_hero_view_png"`.
  
  En `VerificationRun`, los diagnósticos antes y después se comparan como conjuntos `BTreeSet<String>` de `diagnostic_id`. Si coexisten dos errores distintos que colisionan en ID, uno sobreescribe al otro en el conjunto; al corregirse uno solo de ellos, el sistema no puede discernir cuál fue resuelto.
- **Test que lo reproduce**:
  `test_diagnostic_id_collision_via_lossy_normalization` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L540-L596)
  ```
  Salida resumida:
  test test_diagnostic_id_collision_via_lossy_normalization ... ok
  ```
- **Fix sugerido**:
  En lugar de colapsar agresivamente todo carácter especial a `_`, preservar la unicidad agregando un hash corto (p. ej. los primeros 8 caracteres de SHA-256 de la ruta completa) o usando percent-encoding estándar.

---

### BUG-AUTH-09: Pánico por integer overflow en `NodeGraph::add_node` al alcanzar `u32::MAX`
- **Severidad**: Baja / Robustez
- **Archivo:Línea**: [crates/core/src/authoring/graph.rs:93](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/graph.rs#L93)
- **Descripción**:
  `NodeGraph::add_node` realiza `let id = self.next_id; self.next_id += 1;`.
  Si un nodo fue previamente insertado con ID `u32::MAX` mediante `add_node_with_id(u32::MAX, ...)`, `self.next_id` se satura en `u32::MAX`.
  Cualquier llamada posterior a `add_node()` genera un pánico en modo debug por desbordamiento de entero (`attempt to add with overflow`). En modo release sin verificaciones, se desborda silenciosamente a `0`, colisionando con el nodo inicial.
- **Test que lo reproduce**:
  `test_node_id_overflow_causes_panic_or_collision` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L650-L685)
  ```
  Salida resumida:
  test test_node_id_overflow_causes_panic_or_collision ... ok
  ```
- **Fix sugerido**:
  Implementar asignación segura de IDs:
  ```rust
  let id = self.next_id;
  self.next_id = self.next_id.checked_add(1).unwrap_or_else(|| self.find_first_free_id());
  ```

---

### BUG-AUTH-10: No-determinismo en `authoring_graph_sha256` según el orden de inserción de nodos
- **Severidad**: Media
- **Archivo:Línea**: [crates/core/src/authoring/report_fingerprint.rs:132-153](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/report_fingerprint.rs#L132-L153)
- **Descripción**:
  `authoring_story_graph_sha256` ordena canónicamente los nodos y conexiones por su clave numérica antes de computar el hash SHA-256.
  Por el contrario, `authoring_graph_sha256` serializa la estructura `NodeGraph` directamente (`nodes` como `Vec<(u32, StoryNode, AuthoringPosition)>`). Si dos grafos contienen exactamente los mismos nodos, posiciones y conexiones, pero los nodos fueron agregados en diferente secuencia cronológica, `authoring_graph_sha256` genera hashes dispares, provocando falsos positivos en la detección de cambios de layout/grafo en reportes y sesiones de workbench.
- **Test que lo reproduce**:
  `test_authoring_graph_sha256_nondeterministic_with_insertion_order` en [crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs#L598-L648)
  ```
  Salida resumida:
  test test_authoring_graph_sha256_nondeterministic_with_insertion_order ... ok
  ```
- **Fix sugerido**:
  Ordenar los nodos y conexiones por ID al momento de generar la referencia de serialización en `authoring_graph_sha256`.

---

## 3. Sospechas (Sin confirmar por API privada o comportamiento ambiguo)

### SOSPECHA-01: Pérdida del diálogo cuando el hablante está entrecomillado en Ren'Py
- **Archivo:Línea**: [crates/core/src/renpy_import/syntax.rs:193-199](file:///D:/Rust/audit/rustnovel/crates/core/src/renpy_import/syntax.rs#L193-L199)
- **Evidencia**:
  En Ren'Py es válido escribir `"Eileen" "Hola mundo"` (hablante y texto ambos entre comillas). En `parse_dialogue_line`, si la línea comienza con `"` o `'`, la función asume inmediatamente que se trata de una línea del narrador y extrae únicamente la primera cadena (`"Eileen"`), descartando `"Hola mundo"` y asignando como texto del diálogo el nombre del personaje.

### SOSPECHA-02: Generación de labels anónimos vacíos en Ren'Py
- **Archivo:Línea**: [crates/core/src/renpy_import/parser.rs:334-343](file:///D:/Rust/audit/rustnovel/crates/core/src/renpy_import/parser.rs#L334-L343)
- **Evidencia**:
  Una declaración errónea como `label :` no es rechazada, sino que extrae `""` y almacena `self.current_global_label = Some("")`. Si a continuación se declaran sub-etiquetas como `.local`, se genera la etiqueta errónea `".local"`.

---

## 4. Simplificaciones Propuestas

1. **Eliminar estado muerto de Redo o implementarlo completamente**:
   - `AuthoringCommandBus` y `AuthoringDocumentSession` mantienen campos `redo_deltas: Vec<...>` que crecen pasivamente al invocar `revert_last`.
   - Sin embargo, ni `AuthoringCommand` ni `AuthoringDocumentCommand` poseen variante `Redo`, ni los structs exponen ningún método `redo()`.
   - *Propuesta*: O bien implementar el comando `Redo` ejecutando la acción inversa de la delta revertida, o bien eliminar los vectores `redo_deltas` y sus contadores expuestos.

2. **Unificar la duplicación entre `AuthoringCommandBus` y `AuthoringDocumentSession`**:
   - `AuthoringDocumentSession` duplica gran parte del tracking de `AuthoringCommandBus` (fingerprints, logging de operaciones, verificación de diagnósticos).
   - En cada comando de grafo, `AuthoringDocumentSession` instancia un `AuthoringCommandBus` efímero, mueve el grafo por `std::mem::take`, ejecuta el comando y extrae el grafo de vuelta.
   - *Propuesta*: Fusionar ambas abstracciones en un único `AuthoringSession` que gestione tanto el documento completo como las mutaciones del grafo.

---

## 5. Mejoras Arquitectónicas

1. **Gestión Unificada de E/S y Sanitización de Assets**:
   Centralizar la validación de rutas seguras en un único módulo (`crate::security`), garantizando que tanto el importador de Ren'Py, el compilador y el validador de grafo usen las mismas reglas estrictas de rechazo de rutas relativas con escape o rutas absolutas de Windows.
2. **Determinismo Canónico en Serialización JSON**:
   Garantizar que todas las estructuras de autoría utilicen mapas ordenados (`BTreeMap`) y que los vectores con significado de conjunto (nodos, conexiones, bookmarks) se ordenen antes de calcular cualquier huella digital (SHA-256).

---

## 6. Tests Añadidos y Cómo Ejecutarlos

Se agregó la suite completa de pruebas adversarias en:
[crates/core/tests/audit_core_authoring_tests.rs](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_authoring_tests.rs)

### Ejecución:
En una terminal de PowerShell:
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test audit_core_authoring_tests
```

### Resultado de la ejecución:
```
running 13 tests
test test_diagnostic_id_collision_via_lossy_normalization ... ok
test test_node_id_overflow_causes_panic_or_collision ... ok
test test_authoring_graph_sha256_nondeterministic_with_insertion_order ... ok
test test_quick_fix_choice_add_default_option_breaks_strict_export ... ok
test test_failed_revert_of_quick_fix_corrupts_undo_history ... ok
test test_remove_node_with_active_connections_leaves_dangling_ports_and_loses_bookmarks_on_undo ... ok
test test_redo_functionality_is_completely_missing ... ok
test test_failed_revert_last_corrupts_undo_history ... ok
test test_document_session_remove_node_orphans_background_fit_overrides ... ok
test test_renpy_import_retains_unsafe_asset_paths_in_events ... ok
test test_story_graph_cycle_causes_dry_run_parity_error_and_phase_failure ... ok
test test_renpy_import_irregular_indent_and_non_ascii_labels ... ok
test test_renpy_import_menu_falls_through_between_options ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
