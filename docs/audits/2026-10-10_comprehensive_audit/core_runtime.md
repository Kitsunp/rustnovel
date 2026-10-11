# Informe de Auditoría: Core Runtime (`visual_novel_engine`)

**Área auditada**: `core_runtime` (`crates/core`)  
**Fecha**: 11 de Octubre de 2026  
**Auditor**: Senior Rust Security & Architecture Auditor  
**Workspace**: `D:\Rust\audit\rustnovel`  

---

## 1. Resumen Ejecutivo

Se ha llevado a cabo una auditoría profunda de la lógica de tiempo de ejecución (runtime) del crate `crates/core` (`visual_novel_engine`). El análisis cubrió exhaustivamente:
- `src/engine/**` (ejecución paso a paso, prefetch de assets, audio encolado, saltos y opciones)
- `src/script/**` (compilación, representación `ScriptRaw` vs `ScriptCompiled`, encriptado binario/Postcard)
- `src/event/**` y `src/event_behavior/**` (comportamiento de eventos, dispatch, interpolación y mutación de estado)
- `src/storage/**` (serialización binaria `SaveData`, autenticación HMAC-SHA256, recuperación ante fallos de slots)
- `src/scene_frame/**` (resolución de layout de pantalla, presentación headless, temas y safe areas)
- `src/timeline/**` (tracks de animación, keyframes, interpolación Q16.16, funciones de easing)
- `src/graph/**` (generación de grafo de historia, detección de ciclos y nodos inalcanzables)
- `src/repro/**` (ejecución de casos de reproducción y verificación de oráculos)
- `src/player_menu/**` (validación y normalización de configuraciones de menú de jugador)
- `src/state.rs`, `src/entity.rs`, `lib.rs` (almacenamiento de estado, entidades y determinismo)

Se implementaron 26 pruebas adversarias de integración en `crates/core/tests/audit_core_runtime_tests.rs`. Se identificaron **5 Bugs CONFIRMADOS** (2 de Severidad Alta, 3 de Severidad Media), **4 Sospechas / Riesgos Latentes**, varias simplificaciones y mejoras arquitectónicas.

---

## 2. Bugs CONFIRMADOS

### BUG-CR-01: Desincronización de Índice y Colisión de IDs en `SceneState` tras Deserialización
- **Severidad**: Alta
- **Archivo:Línea**: [`crates/core/src/entity.rs:216-220`](file:///D:/Rust/audit/rustnovel/crates/core/src/entity.rs#L216-L220)
- **Descripción**:  
  En `SceneState`, los campos `index: HashMap<EntityId, usize>` y `next_id: u32` están decorados con `#[serde(skip)]`.  
  Cuando un `SceneState` se deserializa (desde JSON o Postcard):
  1. `index` queda completamente vacío (`HashMap::new()`). Cualquier llamada posterior a `scene.get(id)`, `scene.get_mut(id)` o `scene.despawn(id)` devuelve `None`/`false`, a pesar de que las entidades existen en el vector interno.
  2. `next_id` se inicializa en 0 (`default_next_id()`). Si se invoca `scene.spawn(...)` después de deserializar, se asigna `EntityId(0)`, **colisionando** con la entidad con ID 0 ya existente.  
  3. Si luego se llama manualmente a `rebuild_index()`, la entidad duplicada sobreescribe la entrada en el mapa hash, dejando la entidad previa huérfana e inaccesible por ID.
- **Test que lo reproduce**:  
  `test_scene_state_deserialization_index_and_next_id_desync_bug` en [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)  
  *Salida*:
  ```
  test test_scene_state_deserialization_index_and_next_id_desync_bug ... ok
  ```
  El test comprueba que `restored.get(id0)` es `None` y que el siguiente `spawn()` devuelve nuevamente `id0` (`EntityId(0)`).
- **Fix sugerido**:  
  Implementar una deserialización personalizada o un hook post-deserialización (`#[serde(from = "...")]` o implementación manual de `Deserialize`) que invoque `rebuild_index()` automáticamente al reconstruir `SceneState`:
  ```rust
  impl<'de> Deserialize<'de> for SceneState {
      fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
      where
          D: serde::Deserializer<'de>,
      {
          #[derive(Deserialize)]
          struct SceneStateData {
              entities: Vec<Entity>,
          }
          let data = SceneStateData::deserialize(deserializer)?;
          let mut state = SceneState {
              entities: data.entities,
              index: HashMap::new(),
              next_id: 0,
          };
          state.rebuild_index();
          Ok(state)
      }
  }
  ```

---

### BUG-CR-02: Pánico por Desbordamiento Aritmético en `SceneState::rebuild_index` con `EntityId(u32::MAX)`
- **Severidad**: Alta
- **Archivo:Línea**: [`crates/core/src/entity.rs:367`](file:///D:/Rust/audit/rustnovel/crates/core/src/entity.rs#L367)
- **Descripción**:  
  El método `rebuild_index` calcula el siguiente ID disponible calculando el máximo y sumándole 1:
  ```rust
  self.next_id = self
      .entities
      .iter()
      .map(|e| e.id.raw())
      .max()
      .map(|m| m + 1)
      .unwrap_or(0);
  ```
  Si el estado deserializado contiene una entidad con `id = 4294967295` (`u32::MAX`), la expresión `m + 1` desborda `u32` en modo depuración (debug), provocando un **panic** incondicional (`attempt to add with overflow`).
- **Test que lo reproduce**:  
  `test_scene_state_rebuild_index_overflow_panic_on_u32_max` en [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)  
  *Salida*:
  ```
  test test_scene_state_rebuild_index_overflow_panic_on_u32_max ... ok
  ```
  El test captura el `panic` arrojado por `restored.rebuild_index()` cuando contiene una entidad con ID `u32::MAX`.
- **Fix sugerido**:  
  Usar suma saturada o `checked_add`:
  ```rust
  self.next_id = self
      .entities
      .iter()
      .map(|e| e.id.raw())
      .max()
      .and_then(|m| m.checked_add(1))
      .unwrap_or(u32::MAX);
  ```

---

### BUG-CR-03: Pánico por Desbordamiento Aritmético en `Engine::peek_linear_asset_paths` con `depth == usize::MAX`
- **Severidad**: Media
- **Archivo:Línea**: [`crates/core/src/engine/prefetch.rs:42`](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/prefetch.rs#L42)
- **Descripción**:  
  Al calcular el rango de eventos a inspeccionar para prefetching lineal:
  ```rust
  let start = self.state().position as usize;
  let end = (start + depth).min(self.script().events.len());
  ```
  Si el motor ha avanzado (`start >= 1`) y el cliente solicita prefetch con profundidad máxima (`depth = usize::MAX`), la suma `start + depth` desborda `usize`, generando un **panic** por desbordamiento entero en modo debug (`attempt to add with overflow`).
- **Test que lo reproduce**:  
  `test_prefetch_depth_overflow_panic_bug` en [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)  
  *Salida*:
  ```
  test test_prefetch_depth_overflow_panic_bug ... ok
  ```
  El test avanza 1 paso (`start = 1`) y ejecuta `engine.peek_next_asset_paths(usize::MAX)`, capturando el pánico producido por la adición desbordada.
- **Fix sugerido**:  
  Utilizar `saturating_add`:
  ```rust
  let end = start.saturating_add(depth).min(self.script().events.len());
  ```

---

### BUG-CR-04: Truncamiento Silencioso y Corrupción Aritmética en `Fixed::from_int`
- **Severidad**: Media
- **Archivo:Línea**: [`crates/core/src/timeline/types.rs:14`](file:///D:/Rust/audit/rustnovel/crates/core/src/timeline/types.rs#L14)
- **Descripción**:  
  La estructura `Fixed` modela números de punto fijo con formato Q16.16 sobre un `i32` interno (16 bits para parte entera con signo y 16 bits para parte fraccionaria). Esto limita el rango representable entero estricto a $[-32768, 32767]$.  
  Al convertir enteros mediante:
  ```rust
  pub const fn from_int(n: i32) -> Self {
      Self(n << Self::FRAC_BITS)
  }
  ```
  Cualquier valor de propiedad o coordenada mayor a $32767$ (como `50_000`, común en coordenadas virtuales de escenas grandes o mundos de novela visual) desborda hacia el bit de signo o trunca bits significativos. Por ejemplo, `50_000` se convierte silenciosamente en `-15_536` al evaluar la interpolación.
- **Test que lo reproduce**:  
  `test_timeline_fixed_point_q16_16_overflow_bug` en [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)  
  *Salida*:
  ```
  test test_timeline_fixed_point_q16_16_overflow_bug ... ok
  ```
  El test verifica que `Fixed::from_int(50_000).to_int()` devuelve `-15_536` en lugar de `50_000`.
- **Fix sugerido**:  
  - Opción A (recomendada): Migrar la representación interna a Q32.32 sobre `i64` (`Fixed64`), manteniendo determinismo bit-exacto entre arquitecturas con rango entero de $\pm 2 \times 10^9$.  
  - Opción B: Documentar formalmente el rango $[-32768, 32767]$ y aplicar saturación en `from_int` (`n.clamp(-32768, 32767) << 16`).

---

### BUG-CR-05: Parámetro `StageProfile.safe_area_mode` Ignorado Incondicionalmente en `resolve_layout`
- **Severidad**: Media
- **Archivo:Línea**: [`crates/core/src/scene_frame/layout.rs:51-54`](file:///D:/Rust/audit/rustnovel/crates/core/src/scene_frame/layout.rs#L51-L54) y [`crates/core/src/scene_frame.rs:84`](file:///D:/Rust/audit/rustnovel/crates/core/src/scene_frame.rs#L84)
- **Descripción**:  
  `StageProfile` declara el campo `pub safe_area_mode: SafeAreaMode`, que permite configurar `SafeAreaMode::Ignore` o `SafeAreaMode::Respect`.  
  Sin embargo, en `resolve_layout`:
  ```rust
  let available_width =
      (display.logical_size[0] - display.safe_area.left - display.safe_area.right).max(1.0);
  let available_height =
      (display.logical_size[1] - display.safe_area.top - display.safe_area.bottom).max(1.0);
  ```
  El campo `stage.safe_area_mode` **nunca se consulta**. El layout siempre descuenta los insets del área segura, imposibilitando que una pantalla o stage configurado con `SafeAreaMode::Ignore` ocupe la superficie total de la pantalla.
- **Test que lo reproduce**:  
  `test_resolve_layout_ignores_safe_area_mode` en [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)  
  *Salida*:
  ```
  test test_resolve_layout_ignores_safe_area_mode ... ok
  ```
  El test demuestra que `resolve_layout` produce exactamente el mismo `stage_rect` tanto con `SafeAreaMode::Respect` como con `SafeAreaMode::Ignore`.
- **Fix sugerido**:  
  Consultar `stage.safe_area_mode` al calcular el área disponible:
  ```rust
  let (available_width, available_height, offset_x, offset_y) = match stage.safe_area_mode {
      SafeAreaMode::Respect => (
          (display.logical_size[0] - display.safe_area.left - display.safe_area.right).max(1.0),
          (display.logical_size[1] - display.safe_area.top - display.safe_area.bottom).max(1.0),
          display.safe_area.left,
          display.safe_area.top,
      ),
      SafeAreaMode::Ignore => (
          display.logical_size[0].max(1.0),
          display.logical_size[1].max(1.0),
          0.0,
          0.0,
      ),
  };
  ```

---

## 3. Sospechas y Riesgos Latentes

### SOSPECHA-01: Mutación con Efectos Secundarios en Match Guard en `resolve_visual_linear`
- **Archivo:Línea**: [`crates/core/src/route_tree.rs:166-172`](file:///D:/Rust/audit/rustnovel/crates/core/src/route_tree.rs#L166-L172)
- **Evidencia**:
  ```rust
  EventCompiled::SetCharacterPosition(pos)
      if visual.set_character_position(pos).is_err() =>
  {
      break;
  }
  EventCompiled::SetCharacterPosition(_) => {}
  ```
  Llamar a `visual.set_character_position(pos)` (que muta `visual` via `&mut self`) dentro de la condición de guarda de un `match` es un anti-patrón en Rust. Si la llamada tiene éxito, el guard evalúa a `false`, el brazo no se toma, y Rust pasa al siguiente brazo vacío. Aunque actualmente funciona en el compilador actual, cualquier refactorización o reordenamiento romperá la mutación esperada.
- **Fix sugerido**:
  Reemplazar el guard por una llamada explícita dentro del cuerpo del match:
  ```rust
  EventCompiled::SetCharacterPosition(pos) => {
      if visual.set_character_position(pos).is_err() {
          break;
      }
  }
  ```

---

### SOSPECHA-02: Pérdida de Sincronización de Comandos de Audio en `Engine::set_state`
- **Archivo:Línea**: [`crates/core/src/engine/runtime.rs:450-480`](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L450-L480)
- **Evidencia**:  
  Al inicializar el motor con `Engine::from_validated_compiled`, se generan los comandos de audio iniciales llamando a `initial_audio_commands(&state)` ([línea 121](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L121)).  
  Sin embargo, en `Engine::set_state(state)`:
  ```rust
  self.state = state;
  self.pending_transition = None;
  ...
  ```
  `self.queued_audio` no se actualiza ni se limpia. Si el estado restaurado contiene una pista musical diferente o si el motor tenía comandos de audio pendientes antes de cargar, el backend de renderizado no recibirá la orden de reproducir la pista del nuevo estado o reproducirá comandos obsoletos.

---

### SOSPECHA-03: Aborto Completo de `SaveSlotStore::list_slots` ante un Único Archivo de Metadatos Corrupto
- **Archivo:Línea**: [`crates/core/src/storage.rs:360-361`](file:///D:/Rust/audit/rustnovel/crates/core/src/storage.rs#L360-L361)
- **Evidencia**:  
  En `SaveSlotStore::list_slots`:
  ```rust
  let bytes = read_existing_regular_file(&path, "slot metadata")?;
  let metadata: SaveSlotMetadata = serde_json::from_slice(&bytes)
      .map_err(|err| SaveError::Serialization(err.to_string()))?;
  ```
  Si cualquier archivo JSON en el directorio `meta/` sufre corrupción de bytes (por un apagón o escritura incompleta), `serde_json::from_slice` propaga el error con `?`, haciendo que la función entera falle y el usuario no pueda ver ninguna de sus partidas guardadas restantes.
- **Fix sugerido**:  
  Omitir entradas de metadatos corruptas y continuar iterando, o registrarlas como advertencia.

---

### SOSPECHA-04: Potencial Asignación Masiva (OOM) en `EngineState::set_var` y `set_flag` con IDs Grandes
- **Archivo:Línea**: [`crates/core/src/state.rs:45`](file:///D:/Rust/audit/rustnovel/crates/core/src/state.rs#L45), [`L67`](file:///D:/Rust/audit/rustnovel/crates/core/src/state.rs#L67)
- **Evidencia**:  
  ```rust
  pub fn set_var(&mut self, id: u32, value: i32) {
      let idx = id as usize;
      if idx >= self.vars.len() {
          self.vars.resize(idx + 1, 0);
      }
      self.vars[idx] = value;
  }
  ```
  A diferencia de la validación estricta en tiempo de compilación (`CompileCtx`), si a nivel de API pública o mediante deserialización de un estado alterado se pasa un `id` elevado (ej. `1_000_000`), `resize` asignará memoria desproporcionada sin validar límites presupuestarios. Debería verificarse contra un límite predefinido (ej. `MAX_VARS`).

---

## 4. Simplificaciones Propuestas

1. **Eliminar o Integrar el Campo Huérfano `ticks_per_second` en `Timeline`**:  
   - *Ubicación*: [`crates/core/src/timeline.rs:176`](file:///D:/Rust/audit/rustnovel/crates/core/src/timeline.rs#L176)  
   - *Diagnóstico*: Se almacena en la estructura y se exige como argumento en `Timeline::new(ticks_per_second: u32)`, pero nunca se utiliza en ningún cálculo (`advance`, `evaluate`, `duration`).  
   - *Propuesta*: Agregar métodos de conveniencia como `advance_seconds(&mut self, dt: f32)` o eliminar el campo si la API se mantiene exclusivamente en ticks discretos.

2. **Reemplazo del Mapeo Hash Manual en `SceneState`**:  
   - *Ubicación*: [`crates/core/src/entity.rs:214-220`](file:///D:/Rust/audit/rustnovel/crates/core/src/entity.rs#L214-L220)  
   - *Diagnóstico*: `SceneState` mantiene un `Vec<Entity>` y un `HashMap<EntityId, usize>` sincronizado manualmente con `swap_remove`. Dado que el presupuesto es de sólo 1024 entidades (`MAX_ENTITIES`), el overhead de sincronización manual, el riesgo de desincronización tras serde y los métodos manuales `rebuild_index` añaden complejidad sin beneficio real de rendimiento frente a un vector compacto o un `slotmap`.

3. **Consolidación de Métodos de Búsqueda de Entidades**:  
   - En `SceneState`, `iter_sorted()` asigna un nuevo `Vec<&Entity>` en cada invocación:
     ```rust
     let mut sorted: Vec<_> = self.entities.iter().collect();
     sorted.sort_by(...);
     ```
     Para evitar asignaciones de memoria dinámicas en cada frame de renderizado, se puede mantener el vector interno ordenado por `(z_order, id)` o reutilizar un buffer de índices.

---

## 5. Mejoras Recomendadas

1. **Robustez en la Autenticación de Guardado**:  
   Actualmente `SaveData::from_any_binary` sólo acepta cargas autenticadas (`from_authenticated_binary`), rechazando formatos estándar `from_binary`. Esto es bueno para la seguridad del juego empaquetado, pero debería renombrarse a `from_authenticated_slot` para evitar ambigüedad en la API pública.

2. **Migración a Q32.32 en `Fixed`**:  
   La biblioteca se beneficia enormemente del determinismo bit-exacto en animación y timeline, pero el formato Q16.16 sobre `i32` es insuficiente para coordenadas mayores a 32,767. La transición a `i64` eliminará los problemas de truncamiento y desbordamiento sin perder reproducibilidad.

3. **Manejo Resiliente de Slots de Guardado**:  
   En `SaveSlotStore::load_slot`, la recuperación automática mediante archivos `.bak` funciona de manera excelente (verificado por prueba unitaria). Extender esta misma resiliencia a `list_slots` aislará fallos de metadatos corruptos individuales.

---

## 6. Pruebas Añadidas y Cómo Ejecutarlas

Se ha creado la suite de pruebas de integración adversarial:
- **Ruta**: [`crates/core/tests/audit_core_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/core/tests/audit_core_runtime_tests.rs)

### Cobertura de las Pruebas:
1. `test_choose_on_non_choice_event_fails`: Error `InvalidChoice` al elegir sobre evento que no es `Choice`.
2. `test_choose_out_of_bounds_index_fails`: Error `InvalidChoice` al elegir índice inexistente o `usize::MAX`.
3. `test_step_at_end_of_script_returns_end_of_script`: Finalización correcta y retorno de `EndOfScript`.
4. `test_compiled_validation_rejects_out_of_range_choice_target`: Validación de seguridad rechazando targets inválidos.
5. `test_variable_extreme_values_comparison`: Comparaciones condicionales exactas con `i32::MAX` y `i32::MIN`.
6. `test_uninitialized_variable_comparison`: Manejo determinista (valor por defecto 0) para variables no inicializadas.
7. `test_jump_to_nonexistent_label_fails_compilation`: Detección en compilación y validación de labels inexistentes.
8. `test_engine_jump_to_label_nonexistent_fails`: Error `InvalidScript` en `jump_to_label` inexistente en runtime.
9. `test_infinite_jump_loop_execution_and_cycle_detection`: Ejecución de saltos circulares y detección formal de ciclos en `StoryGraph`.
10. `test_binary_save_truncated_data`: Manejo de errores `TooSmall` y `LengthMismatch` ante saves truncados.
11. `test_binary_save_altered_checksum`: Detección y rechazo inmediato por `ChecksumMismatch`.
12. `test_binary_save_altered_payload`: Detección de payload alterado sin alterar checksum.
13. `test_save_script_id_mismatch`: Rechazo de saves con `ScriptMismatch` ante hash de script diferente.
14. `test_authenticated_save_tampering_and_recovery`: Recuperación automática desde backup `.bak` ante corrupción de HMAC.
15. `test_timeline_duplicate_keyframe_time_rejected`: Rechazo de keyframes duplicados en el mismo tick.
16. `test_timeline_keyframe_and_track_capacity_limits`: Enforzamiento de límites `MAX_KEYFRAMES_PER_TRACK` y `MAX_TRACKS`.
17. `test_timeline_excessive_seek_and_advance`: Resistencia a `seek(u32::MAX)` y avance con adición saturada.
18. `test_timeline_fixed_point_q16_16_overflow_bug`: Demostración del bug de truncamiento en valores > 32767.
19. `test_timeline_easing_extreme_values`: Evaluación de funciones de easing con valores extremos y negativos.
20. `test_scene_state_iter_sorted_determinism`: Determinismo estricto de ordenamiento por `(z_order, id)`.
21. `test_scene_state_deserialization_index_and_next_id_desync_bug`: Demostración del bug de pérdida de índice y colisión de IDs en `SceneState`.
22. `test_scene_state_rebuild_index_overflow_panic_on_u32_max`: Demostración del pánico por desbordamiento con `EntityId(u32::MAX)`.
23. `test_dialogue_history_limit_capped_at_200`: Validación del límite de 200 diálogos con expulsión FIFO.
24. `test_choice_history_limit_capped_at_512`: Validación del límite de 512 decisiones en historial de opciones.
25. `test_prefetch_depth_overflow_panic_bug`: Demostración del pánico por desbordamiento en prefetch con `usize::MAX`.
26. `test_resolve_layout_ignores_safe_area_mode`: Demostración de que `safe_area_mode` es ignorado en el cálculo de layout.

### Comando de Ejecución:
En PowerShell (con las variables de entorno configuradas):
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH"; cargo test -p visual_novel_engine --test audit_core_runtime_tests
```

**Resultado de la ejecución**:
```
running 26 tests
test test_timeline_duplicate_keyframe_time_rejected ... ok
test test_dialogue_history_limit_capped_at_200 ... ok
test test_resolve_layout_ignores_safe_area_mode ... ok
test test_compiled_validation_rejects_out_of_range_choice_target ... ok
test test_jump_to_nonexistent_label_fails_compilation ... ok
test test_timeline_easing_extreme_values ... ok
test test_timeline_fixed_point_q16_16_overflow_bug ... ok
test test_timeline_excessive_seek_and_advance ... ok
test test_timeline_keyframe_and_track_capacity_limits ... ok
test test_uninitialized_variable_comparison ... ok
test test_scene_state_iter_sorted_determinism ... ok
test test_choose_on_non_choice_event_fails ... ok
test test_choose_out_of_bounds_index_fails ... ok
test test_variable_extreme_values_comparison ... ok
test test_engine_jump_to_label_nonexistent_fails ... ok
test test_step_at_end_of_script_returns_end_of_script ... ok
test test_save_script_id_mismatch ... ok
test test_scene_state_deserialization_index_and_next_id_desync_bug ... ok
test test_binary_save_truncated_data ... ok
test test_binary_save_altered_checksum ... ok
test test_infinite_jump_loop_execution_and_cycle_detection ... ok
test test_binary_save_altered_payload ... ok
test test_scene_state_rebuild_index_overflow_panic_on_u32_max ... ok
test test_prefetch_depth_overflow_panic_bug ... ok
test test_choice_history_limit_capped_at_512 ... ok
test test_authenticated_save_tampering_and_recovery ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
