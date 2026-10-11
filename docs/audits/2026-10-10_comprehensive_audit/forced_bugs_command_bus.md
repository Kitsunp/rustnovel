# Reporte de Auditoría: Forzado de Bugs en Command Bus y Undo/Redo

**Área auditada:** `crates/core/src/authoring/command_bus` y `crates/core/src/authoring/document_command_bus`  
**Archivos fuente principales:**
- `crates/core/src/authoring/command_bus.rs`
- `crates/core/src/authoring/command_bus/apply.rs`
- `crates/core/src/authoring/command_bus/choice.rs`
- `crates/core/src/authoring/command_bus/inverse.rs`
- `crates/core/src/authoring/command_bus/node_edits.rs`
- `crates/core/src/authoring/command_bus/types.rs`
- `crates/core/src/authoring/document_command_bus.rs`
- `crates/core/src/authoring/document_command_bus/apply.rs`
- `crates/core/src/authoring/document_command_bus/inverse.rs`
- `crates/core/src/authoring/document_command_bus/read_model.rs`
- `crates/core/src/authoring/document_command_bus/types.rs`
- `crates/core/src/authoring/graph.rs`

**Suite de pruebas de verificación:** `crates/core/tests/force_command_bus_bugs.rs`  
**Comando de ejecución:**
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test force_command_bus_bugs -- --test-threads=1
```
**Resultado de la suite:** 17 pruebas ejecutadas, 17 pasadas (reproducción y forzado 100% verificado).

---

## Respuestas Específicas a las Preguntas de Auditoría

### 1. Undo sin comandos previos
* **¿Qué pasa si se llama a `undo` (`RevertLast`) en un bus recién inicializado?**
  * Tanto `AuthoringCommandBus` como `AuthoringDocumentCommandBus` / `AuthoringDocumentSession` manejan la llamada limpiamente retornando `Result::Err`:
    * `AuthoringCommandBus::apply(AuthoringCommand::RevertLast)` retorna `Err("no command delta to revert".to_string())`.
    * `AuthoringDocumentCommandBus::apply(AuthoringDocumentCommand::RevertLast)` retorna `Err("no document command delta to revert".to_string())`.
  * No ocurre pánico por pop en vector vacío (`Vec::pop` retorna `None`, protegido por `.ok_or_else(...)`).
* **Llamadas repetidas tras agotar el historial:**
  * Si se aplica 1 comando y se llama a `RevertLast` dos veces, la primera revierte correctamente y la segunda retorna limpiamente `Err("no command delta to revert")` sin panics.
* **Bus inicializado con `with_history` pero sin deltas en memoria:**
  * Si se restaura un bus con `AuthoringCommandBus::with_history(graph, operation_log, verification_runs)`, el vector interno `undo_deltas` se inicializa vacío (`Vec::new()`). Llamar a `RevertLast` retorna limpiamente `Err("no command delta to revert")`, protegiendo la integridad en memoria aunque el log persista operaciones pasadas.
* **Delegación cruzada indebida:**
  * Invocar `AuthoringDocumentCommandBus::apply(AuthoringDocumentCommand::Graph(AuthoringCommand::RevertLast))` es interceptado explícitamente y rechazado con `Err("use AuthoringDocumentCommand::RevertLast for document undo")`.

---

### 2. Redo inválido y Brecha Crítica de API
* **Comportamiento del stack de Redo:**
  * Al revertir un comando, el delta deshecho se traslada a `redo_deltas`, incrementando `redo_delta_count()`.
  * Al aplicar cualquier comando subsiguiente, `redo_deltas.clear()` invalida el historial futuro correctamente (comportamiento estándar de branching history).
* **BUG / BRECHA ARQUITECTÓNICA CRÍTICA: Ausencia total de API de Redo en `crates/core`:**
  * A pesar de que `AuthoringCommandBus` y `AuthoringDocumentSession`:
    1. Mantienen y asignan memoria para un campo privado `redo_deltas: Vec<...>`,
    2. Exponen públicamente `pub fn redo_delta_count(&self) -> usize`,
    3. Acumulan deltas en `self.redo_deltas.push(delta)` durante `revert_last()`,
    4. Limpian deltas en `self.redo_deltas.clear()`,
    5. Y definen `OperationKind::Redo` en `operation_log.rs`,
  * **NO EXISTE NINGÚN MÉTODO `redo()` NI NINGUNA VARIANTE `AuthoringCommand::Redo` NI `AuthoringDocumentCommand::Redo` EN TODO EL CRATE `crates/core`**.
  * La funcionalidad de rehacer comandos está completamente trunca/incompleta. Como consecuencia directa, la capa gráfica (`crates/gui/src/editor/undo.rs`) tuvo que descartar el bus de comandos para undo/redo e implementar un `UndoStack` independiente que clona instancias completas de `NodeGraph` en cada paso.

---

### 3. Invariantes rotos tras fallo
* **BUG CRÍTICO 1: Corrupción del stack de undo por deltas irreversibles (`ConnectOrBranch`, `ApplyQuickFix`):**
  * En `AuthoringCommandBus::revert_last` (`crates/core/src/authoring/command_bus.rs:199`), el delta se extrae con `let delta = self.undo_deltas.pop()?;` **antes** de invocar `self.apply_inverse_delta(&delta)?;`.
  * Si el delta es irreversible (como `AuthoringDelta::BranchConnected` o `AuthoringDelta::QuickFixApplied`), `apply_inverse_delta` retorna `Err(...)`.
  * **El delta extraído NUNCA se reinserta en `self.undo_deltas`.**
  * **Consecuencia grave:** La mutación en el grafo permanece intacta (el nodo de bifurcación y sus 3 conexiones persisten), pero el delta de undo se perdió. Si el usuario vuelve a presionar "Undo", el sistema salta este comando y revierte el comando anterior (ej. eliminando un nodo al que la bifurcación apunta), creando conexiones rotas / dangling edges y corrompiendo la topología.
* **BUG CRÍTICO 2: Congelamiento permanente del Undo Stack en `AuthoringDocumentSession`:**
  * En `AuthoringDocumentSession::revert_last`, ante el error de `apply_inverse_delta`, el delta sí se reinserta con `self.undo_deltas.push(delta);`.
  * Sin embargo, dado que `BranchConnected` o `QuickFixApplied` **nunca** podrán revertirse como deltas unitarios, ese delta bloquea la cima de la pila para siempre: cualquier llamada futura a `undo` fallará repetidamente, impidiendo que el usuario revierta cualquier acción previa.
* **BUG CRÍTICO 3: Inversión de `RemoveNode` destruye membresía de fragmentos y bookmarks:**
  * Al eliminar un nodo con `AuthoringCommand::RemoveNode`, `graph.remove_node(id)` purga al nodo de los fragmentos en los que participaba y borra todos los bookmarks asociados.
  * Sin embargo, `AuthoringDelta::NodeRemoved` solo registra `node_id`, `node`, `position` y `connections`. No preserva la pertenencia a fragmentos ni los bookmarks.
  * Al hacer `RevertLast`, el nodo se restaura en la raíz del grafo, pero **el fragmento queda vacío/incompleto** y **los bookmarks se pierden para siempre**.
* **BUG 4: `EditNode` conserva conexiones incompatibles e ilegales:**
  * Si un nodo de tipo `Choice` con 2 opciones conectadas a otros nodos es editado mediante `AuthoringCommand::EditNode` convirtiéndolo en `StoryNode::End`, la sustitución no valida ni purga `graph.connections`.
  * El nodo `StoryNode::End` termina con 2 conexiones salientes en el grafo, violando las reglas semánticas del motor (`StoryNode::can_connect_from() == false`).

---

### 4. Referencias a nodos inexistentes
* **Comandos sobre nodos inexistentes o previamente eliminados:**
  * `Connect`, `Disconnect`, `EditDialogue`, `EditChoicePrompt` y `RemoveNode` validan la existencia de los nodos o puertos y retornan `Result::Err` limpio sin causar pánico ni mutar el grafo.
* **BUG CONFIRMADO: Sobrescritura fantasma de `BackgroundFitOverride`:**
  * En `AuthoringDocumentCommandBus::apply(AuthoringDocumentCommand::SetBackgroundFitOverride { node_id, fit })`:
  * A diferencia de `SetLayerVisible` (que valida `ensure_composer_layer_exists`), `set_background_fit` **no verifica si `node_id` existe en el grafo ni si es un nodo de tipo Scene**.
  * Ejecutar el comando con `node_id: 99999` tiene éxito (`Ok(...)`) e inserta una entrada fantasma en `document.composer_background_fit_overrides`.

---

### 5. Ciclos de comandos y límite de historial
* **BUG DE CONSUMO DE RECURSOS: Historial no acotado (Unbounded Growth):**
  * `AuthoringCommandBus` y `AuthoringDocumentSession` carecen de límites de profundidad (`MAX_UNDO_STATES`).
  * Cada comando inserta entradas en:
    1. `undo_deltas: Vec<AuthoringDelta>`
    2. `recorded_commands: Vec<AuthoringCommand>`
    3. `operation_log: Vec<OperationLogEntry>`
    4. `verification_runs: Vec<VerificationRun>`
  * 10,000 comandos generan 40,000 estructuras con cadenas SHA-256 completas, rutas de campos e identificadores diagnósticos sin política de poda o ventana deslizante.
* **Auto-conexiones (Self-Loops) y Ciclos topológicos:**
  * `NodeGraph::connect_port` no valida si `from == to`. Un nodo puede conectarse a sí mismo sin error.
  * La detección de ciclos opera downstream en `read_model.reachable_cycle_node_ids()`, lo que previene bucles infinitos en el runtime pero permite que el grafo de autoría entre en ciclos arbitrarios.

---

## Fichas de Bugs Forzados

### Bug 1: Corrupción y Desincronización del Stack de Undo por Deltas Irreversibles

* **Ubicación:** [crates/core/src/authoring/command_bus.rs#L196-L235](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus.rs#L196-L235) y [crates/core/src/authoring/command_bus/inverse.rs#L76-L78](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus/inverse.rs#L76-L78)
* **Comportamiento inesperado:**
  Al invocar `AuthoringCommand::RevertLast` sobre un comando que genera `AuthoringDelta::BranchConnected` (mediante `ConnectOrBranch`) o `AuthoringDelta::QuickFixApplied`, la función `apply_inverse_delta` retorna un error:
  `"branch connection from node X port Y cannot be reverted as a single graph delta"`
  Sin embargo, `revert_last` ya ejecutó `let delta = self.undo_deltas.pop()?;`.
  Al propagar el error con `?`, `delta` se descarta y nunca se devuelve a `undo_deltas`.
  El contador de deltas decrece, la mutación en el grafo sigue presente, y la siguiente llamada a `RevertLast` revierte el comando anterior al branch, desincronizando la historia y dejando nodos huérfanos o conexiones corruptas.
* **Causa raíz:**
  Falta de manejo transaccional en `revert_last`. No se preserva el delta si la inversión falla, y se expone `ConnectOrBranch` y `ApplyQuickFix` en la pila de undo a pesar de que el subsistema de deltas no implementa su inversa.
* **Solución propuesta:**
  Restaurar el delta en caso de error, idéntico a `document_command_bus.rs:223-226`:
  ```rust
  let delta = self.undo_deltas.pop().ok_or_else(|| "no command delta to revert".to_string())?;
  if let Err(error) = self.apply_inverse_delta(&delta) {
      self.undo_deltas.push(delta);
      return Err(error);
  }
  ```
  Adicionalmente, implementar la lógica inversa para `BranchConnected` y `QuickFixApplied` para no dejar comandos irreversibles en el bus.

---

### Bug 2: Bloqueo Permanente (Poisoning) del Historial de Undo en `AuthoringDocumentSession`

* **Ubicación:** [crates/core/src/authoring/document_command_bus.rs#L223-L226](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/document_command_bus.rs#L223-L226)
* **Comportamiento inesperado:**
  Cuando un usuario aplica `ConnectOrBranch` o `ApplyQuickFix` en `AuthoringDocumentSession`, dicho delta entra en `undo_deltas`. Si el usuario intenta hacer deshacer (`RevertLast`), la operación falla con error. Al reinsertarse el delta en la pila, cada intento subsiguiente de deshacer vuelve a toparse con el mismo delta irreversible. La pila de deshacer queda permanentemente trabada e inservible.
* **Causa raíz:**
  El bus de documentos acepta comandos cuyas deltas no son invertibles (`apply_inverse_delta` devuelve `Err` incondicional), convirtiéndolas en bloqueos insalvables para LIFO undo.
* **Solución propuesta:**
  1. Si un comando es irreversible, no debe registrarse como delta invertible o debe registrarse como punto de control no-revertible con política clara (ej. advertencia de que la acción limpia el historial de deshacer).
  2. Implementar los deltas compuestos inversos para que toda operación del bus sea genuinamente simétrica y reversible.

---

### Bug 3: Redo Inexistente en la API de `AuthoringCommandBus` y `AuthoringDocumentSession`

* **Ubicación:** [crates/core/src/authoring/command_bus.rs#L34](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus.rs#L34), [crates/core/src/authoring/document_command_bus.rs#L40](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/document_command_bus.rs#L40)
* **Comportamiento inesperado:**
  Los buses mantienen vectores `redo_deltas`, cuentan deltas de rehacer con `redo_delta_count()`, y limpian el historial al aplicar mutaciones, pero ningún módulo en `visual_novel_engine::authoring` proporciona una forma de re-aplicar o rehacer los deltas (`redo`).
* **Causa raíz:**
  Implementación parcial e inconclusa del patrón Command / Undo-Redo.
* **Solución propuesta:**
  Exponer `AuthoringCommand::Redo` y `AuthoringDocumentCommand::Redo`, e implementar `redo_last(&mut self)` re-aplicando el delta guardado en `redo_deltas`.

---

### Bug 4: Pérdida Irreversible de Fragmentos y Bookmarks al Deshacer `RemoveNode`

* **Ubicación:** [crates/core/src/authoring/command_bus/inverse.rs#L11-L28](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus/inverse.rs#L11-L28) y [crates/core/src/authoring/graph.rs#L117-L122](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/graph.rs#L117-L122)
* **Comportamiento inesperado:**
  Al eliminar un nodo que pertenecía a un `GraphFragment` y/o tenía un `bookmark`:
  - `graph.remove_node` purga el ID del nodo de los fragmentos y elimina los bookmarks.
  - Al ejecutar `RevertLast`, `AuthoringDelta::NodeRemoved` solo restaura el nodo en `self.graph.nodes` y sus conexiones.
  - **El nodo nunca es devuelto al fragmento** (el fragmento queda con lista de nodos vacía o incompleta) y **los bookmarks nunca son restaurados**.
* **Causa raíz:**
  `AuthoringDelta::NodeRemoved` no captura en su payload la lista de fragmentos ni los bookmarks a los que el nodo estaba vinculado antes de la eliminación.
* **Solución propuesta:**
  Extender `AuthoringDelta::NodeRemoved`:
  ```rust
  NodeRemoved {
      node_id: u32,
      node: StoryNode,
      position: AuthoringPosition,
      connections: Vec<GraphConnection>,
      fragment_ids: Vec<String>,
      bookmarks: Vec<String>,
  }
  ```
  y en `apply_inverse_delta`, reinsertar el nodo en sus respectivos fragmentos y restaurar los bookmarks.

---

### Bug 5: Inserción de Sobrescrituras Fantasma para Nodos Inexistentes (`SetBackgroundFitOverride`)

* **Ubicación:** [crates/core/src/authoring/document_command_bus/apply.rs#L138-L167](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/document_command_bus/apply.rs#L138-L167)
* **Comportamiento inesperado:**
  `AuthoringDocumentCommand::SetBackgroundFitOverride { node_id: 99999, fit: BackgroundFit::Contain }` se ejecuta exitosamente (`Ok`) aunque el nodo `99999` no exista en el documento. `document.composer_background_fit_overrides` queda contaminado con IDs fantasmas.
* **Causa raíz:**
  `set_background_fit` omite comprobar si el `node_id` existe en `self.document.graph` y si el nodo es de tipo `StoryNode::Scene`.
* **Solución propuesta:**
  Agregar validación previa:
  ```rust
  fn set_background_fit(&mut self, node_id: u32, fit: BackgroundFit) -> DocumentApplyResult {
      let Some(StoryNode::Scene { .. }) = self.document.graph.get_node(node_id) else {
          return Err(format!("node {node_id} is not an existing scene node"));
      };
      // ...
  }
  ```

---

### Bug 6: Fuga de Conexiones Inválidas en `EditNode`

* **Ubicación:** [crates/core/src/authoring/command_bus/apply.rs#L162-L182](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus/apply.rs#L162-L182)
* **Comportamiento inesperado:**
  Al usar `EditNode` para cambiar un nodo `Choice` con puertos de salida conectados a un nodo `End` (que no admite salidas), el comando muta el nodo sin sanear las conexiones del grafo, dejando conexiones ilegales que violan el contrato de tipos.
* **Causa raíz:**
  `EditNode` reemplaza directamente el `StoryNode` en el grafo sin validar ni desconectar puertos que ya no existen en el nuevo tipo de nodo.
* **Solución propuesta:**
  Validar puertos salientes y entrantes permitidos según `node_spec_for_authoring_node(&replacement)` y purgar o rechazar la mutación si existen conexiones que violen el nuevo contrato.

---

### Bug 7: Crecimiento Sin Límite de Memoria en Historial

* **Ubicación:** [crates/core/src/authoring/command_bus.rs#L184-L188](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/command_bus.rs#L184-L188), [crates/core/src/authoring/document_command_bus.rs#L162-L166](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/document_command_bus.rs#L162-L166)
* **Comportamiento inesperado:**
  `undo_deltas`, `recorded_commands`, `operation_log` y `verification_runs` crecen indefinidamente. En sesiones de edición largas con miles de comandos o scripts automatizados, el consumo de memoria se incrementa de forma no acotada.
* **Causa raíz:**
  Ausencia de una cota de historial o mecanismo de truncado como `MAX_UNDO_STATES`.
* **Solución propuesta:**
  Establecer un límite configurable de deltas de undo (ej. 100) y podar deltas antiguos mediante `VecDeque` o `truncate/pop_front` cuando se exceda la cota.

---

## Resumen de Resultados de Verificación

| ID | Test | Área | Estado | Hallazgo |
|---|---|---|---|---|
| 1 | `test_undo_on_fresh_command_bus_returns_clean_err` | Undo Inicial | PASA | `Err` limpio sin pánico |
| 2 | `test_undo_on_fresh_document_command_bus_returns_clean_err` | Undo Inicial | PASA | `Err` limpio sin pánico |
| 3 | `test_undo_on_bus_with_history_but_empty_deltas_returns_clean_err` | Undo Historial | PASA | `Err` limpio sin pánico |
| 4 | `test_undo_via_document_graph_command_wrapper_rejected` | Undo Wrapper | PASA | Rechazado con mensaje descriptivo |
| 5 | `test_repeated_undo_past_history_exhaustion` | Undo Agotado | PASA | Segundo undo retorna `Err` limpio |
| 6 | `test_redo_accounting_and_missing_api_gap` | Redo API | PASA | Brecha crítica: `redo()` no existe en API |
| 7 | `test_bug_command_bus_undo_stack_corrupted_on_irreversible_command` | Undo Corrupción | PASA | Delta descartado tras error, historial corrupto |
| 8 | `test_bug_document_session_undo_stack_permanently_frozen_on_branch` | Undo Bloqueo | PASA | Historial trabado permanentemente |
| 9 | `test_bug_undo_remove_node_destroys_fragment_membership_and_bookmarks` | Undo Fragmentos | PASA | Membresía de fragmento y bookmarks destruidos |
| 10 | `test_bug_quick_fix_undo_stack_corruption` | QuickFix Undo | PASA | Delta de quick-fix descartado y perdido |
| 11 | `test_bug_bookmark_destroyed_by_remove_node_and_not_restored_on_undo` | Bookmarks Undo | PASA | Bookmarks eliminados no se recuperan |
| 12 | `test_bug_edit_node_leaves_stale_illegal_connections` | Invariantes Grafo | PASA | Conexiones ilegales persisten tras `EditNode` |
| 13 | `test_nonexistent_node_references_handled_cleanly` | Nodos Inexistentes | PASA | Errores limpios en operaciones estándar |
| 14 | `test_bug_phantom_background_fit_on_nonexistent_node` | Nodos Fantasma | PASA | `SetBackgroundFitOverride` acepta nodos inexistentes |
| 15 | `test_bug_unbounded_history_growth_no_depth_limit` | Consumo Memoria | PASA | Historial crece sin cota |
| 16 | `test_graph_self_loop_connection_allowed` | Topología Grafo | PASA | Auto-conexiones permitidas por `connect_port` |
| 17 | `test_cycle_handling_in_document_session` | Ciclos Flujo | PASA | Read model identifica ciclo sin bucles infinitos |
