# Auditoría de Seguridad, Tipos y FFI: Python Bindings (`python_bindings`)

**Fecha:** 11 de Octubre de 2026  
**Auditor:** Senior Rust Security & FFI Auditor  
**Workspace:** `rustnovel` (`crates/py`, `python/`, `tests/python`)  
**Archivos Auditados:**
- `crates/py/Cargo.toml`
- `crates/py/src/lib.rs`
- `crates/py/src/bindings/mod.rs`
- `crates/py/src/bindings/audio.rs`
- `crates/py/src/bindings/builder.rs`
- `crates/py/src/bindings/conversion.rs`
- `crates/py/src/bindings/editor.rs`
- `crates/py/src/bindings/editor_api_v2.rs`
- `crates/py/src/bindings/editor_api_v2/fragments.rs`
- `crates/py/src/bindings/editor_diagnostics.rs`
- `crates/py/src/bindings/editor_node_graph.rs`
- `crates/py/src/bindings/editor_node_graph/composer.rs`
- `crates/py/src/bindings/editor_node_graph/fragments.rs`
- `crates/py/src/bindings/editor_node_graph/internal.rs`
- `crates/py/src/bindings/editor_node_graph/misc.rs`
- `crates/py/src/bindings/editor_story_node.rs`
- `crates/py/src/bindings/editor_support.rs`
- `crates/py/src/bindings/engine.rs`
- `crates/py/src/bindings/graph.rs`
- `crates/py/src/bindings/timeline.rs`
- `crates/py/src/bindings/types.rs`
- `python/visual_novel_engine.pyi`
- `python/vnengine/**` (`__init__.py`, `engine.py`, `types.py`, `script_types.py`, `native.py`, `builder.py`, `app.py`, `localization.py`)
- `tests/python/**` (`test_vnengine.py`, `test_vnengine_bindings.py`, `test_vnengine_engine.py`, `native_healthcheck.py`, `conftest.py`, etc.)

---

## 1. Resumen Ejecutivo

Se ha llevado a cabo una auditoría integral sobre las capas de enlace entre Rust y Python (bindings PyO3 0.24, stubs `.pyi`, wrappers de alto nivel `vnengine` y suites de prueba en Python). La auditoría abarcó la coherencia de contratos de API, seguridad y solidez FFI (panics, manejo de memoria y GIL/borrowing), tipos de retorno, nombres de parámetros por palabra clave (kwargs), y resiliencia ante entradas adversas.

El paquete fue compilado exitosamente en Windows con la toolchain MinGW (`x86_64-pc-windows-gnu`) y Python 3.14.7 utilizando PyO3 con `abi3-py38` y compatibilidad forward ABI3. Las 95 pruebas unitarias preexistentes en `tests/python` pasaron sin errores una vez cargado el artefacto binario nativo.

Sin embargo, el análisis estático y dinámico reveló una **desconexión profunda entre los stubs `.pyi` y la implementación Rust**, así como **inconsistencias críticas de deserialización entre los eventos generados por Rust (`conversion.rs`) y los decodificadores oficiales en Python (`vnengine.types`)**.

### Principales Hallazgos Confirmados (15 Bugs Verificados Dinámicamente):
1. **Desfase Posicional Destructivo en `StoryNode.scene_full`**: El stub `.pyi` omitió el parámetro inicial `profile`, provocando que cualquier invocación posicional en Python según el `.pyi` desplace los argumentos e intente convertir la lista de personajes en el string de música (`TypeError`).
2. **Incompatibilidad Fatal de Deserialización en `SetVar`**: `EventCompiled::SetVar` en `conversion.rs` no incluye la clave `'key'`, lo que hace que `vnengine.types.event_from_dict()` y `SetVar.from_dict()` crasheen inmediatamente con `KeyError: 'key'`.
3. **Pérdida de Condición en `JumpIf`**: `EventCompiled::JumpIf` en `conversion.rs` descarta totalmente la condición `cond` y el `target` simbólico, provocando que `JumpIf.from_dict()` falle con `ValueError: Unknown condition kind: None`.
4. **Desconexión Total de Firma en `ComposerPreviewSession.snapshot`**: Los parámetros en `.pyi` (`selected_node_id`, `runtime_mode`) difieren completamente de los implementados en Rust (`stage_width`, `stage_height`, `locale`), lanzando `TypeError` inmediato al usarlos.
5. **Bloqueo por Re-entrancia de `PyRefMut` en `Engine.register_handler`**: El método `step(&mut self)` retiene el préstamo exclusivo del motor al invocar el callback de Python, impidiendo que el handler consulte el estado (`engine.visual_state()`, etc.) o encole audio (`RuntimeError: Already mutably borrowed`).
6. **Discrepancia en `AudioController`**: Todos los métodos de audio en Rust declaran el parámetro `resource`, pero el archivo `.pyi` declara `asset`. El uso de kwargs (`asset=...`) falla con `TypeError`.
7. **Campos Inexistentes en `QuickFixCandidate`**: Los atributos `label` y `description` documentados en `.pyi` no existen en Rust (que expone `title_es`, `title_en`, etc.), arrojando `AttributeError`.
8. **Múltiples Discordancias de Tipos de Retorno**: `Track.evaluate` devuelve `Optional[int]` (no `Dict`), `Timeline.evaluate` devuelve `List[Tuple[int, str, int]]` (no `Dict`), `NodeGraph.connect_new_choice_option` devuelve `int` (no `bool`), `NodeGraph.fragment_ports` devuelve una tupla de dos listas (no `List[FragmentPort]`), y `NodeGraph.autofix_issue` devuelve `Optional[str]` (no `bool`).
9. **Asimetría en Caché de Assets**: `cache_asset` recorta espacios en blanco con `.trim()`, pero `get_cached_asset` no lo hace, causando fallos de caché silenciosos.
10. **Inconsistencia en `scene_patch_full`**: Mientras `add` acepta tuplas completas de 6 elementos con posición espacial `(x, y, scale)`, `update` solo acepta 3 elementos, impidiendo actualizar la posición de personajes en parches.

---

## 2. Bugs CONFIRMADOS

### BUG-PY-01: Desfase Posicional Destructivo en `StoryNode.scene_full` por Omisión de `profile` en `.pyi`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/py/src/bindings/editor_story_node.rs:80`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_story_node.rs#L80) vs [`python/visual_novel_engine.pyi:113-118`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L113-L118)
- **Descripción:**
  En Rust, la firma de `scene_full` expone `profile` como primer argumento:
  ```rust
  #[staticmethod]
  #[pyo3(signature = (profile=None, background=None, music=None, characters=Vec::new()))]
  fn scene_full(
      profile: Option<String>,
      background: Option<String>,
      music: Option<String>,
      characters: Vec<PyCharacterPlacementFullInput>,
  ) -> Self
  ```
  Sin embargo, en `python/visual_novel_engine.pyi`:
  ```python
  @classmethod
  def scene_full(
      cls,
      background: Optional[str] = ...,
      music: Optional[str] = ...,
      characters: Sequence[Any] = ...,
  ) -> "StoryNode": ...
  ```
  El stub `.pyi` omitió completamente el parámetro `profile`. Si un desarrollador en Python invoca la función pasando argumentos posicionales según la especificación de tipos:
  `StoryNode.scene_full("bg.png", "music.ogg", [])`
  Rust asigna:
  - `profile = Some("bg.png")`
  - `background = Some("music.ogg")`
  - `music = []` (lo que falla la conversión de tipos en PyO3 porque espera `Option<String>`).
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_story_node_scene_full_positional_mismatch` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  ```python
  vne.StoryNode.scene_full("bg.png", "music.ogg", [])
  ```
  Salida:
  ```
  TypeError: argument 'music': 'list' object cannot be converted to 'PyString'
  ```
- **Fix sugerido:**
  Actualizar `python/visual_novel_engine.pyi` para incluir `profile: Optional[str] = ...` como primer parámetro opcional en `scene_full`.

---

### BUG-PY-02: Conversión de `EventCompiled::SetVar` Omite `'key'`, Quebrando `vnengine.types.event_from_dict`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/py/src/bindings/conversion.rs:56-60`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/conversion.rs#L56-L60) y [`python/vnengine/types.py:417-419`](file:///D:/Rust/audit/rustnovel/python/vnengine/types.py#L417-L419)
- **Descripción:**
  En `conversion.rs`, la serialización del evento `SetFlag` incluye compatibilidad bidireccional de nombres:
  ```rust
  EventCompiled::SetFlag { flag_id, value } => {
      dict.set_item("type", "set_flag")?;
      dict.set_item("key", *flag_id)?;
      dict.set_item("flag_id", *flag_id)?;
      dict.set_item("value", *value)?;
  }
  ```
  Pero en `SetVar`, se omitió el campo `"key"`:
  ```rust
  EventCompiled::SetVar { var_id, value } => {
      dict.set_item("type", "set_var")?;
      dict.set_item("var_id", *var_id)?;
      dict.set_item("value", *value)?;
  }
  ```
  La clase estándar en Python `vnengine.types.SetVar.from_dict` exige obligatoriamente `data["key"]`:
  ```python
  @classmethod
  def from_dict(cls, data: Mapping[str, Any]) -> "SetVar":
      value = data["value"]
      return cls(key=str(data["key"]), value=_require_int(value, "SetVar 'value'"))
  ```
  Como consecuencia, cualquier script que ejecute una asignación de variable y cuyo evento sea procesado por las capas oficiales de Python (`EngineApp`, `run_script_headless`, o `event_from_dict`) crashea con `KeyError: 'key'`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_set_var_conversion_drops_key` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  ```python
  e = vne.Engine(script_with_set_var)
  t.event_from_dict(e.current_event())
  ```
  Salida:
  ```
  KeyError: 'key'
  Event dict from Rust: {'type': 'set_var', 'var_id': 0, 'value': 100}
  ```
- **Fix sugerido:**
  En `crates/py/src/bindings/conversion.rs:56-60`, agregar `dict.set_item("key", *var_id)?;` para mantener paridad con `SetFlag`. Alternativamente, en `vnengine/types.py:419`, soportar fallback: `key=str(data.get("key", data.get("var_id", "")))`.

---

### BUG-PY-03: Incompatibilidad de Contrato en `ComposerPreviewSession.snapshot`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/py/src/bindings/editor_api_v2.rs:244-255`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_api_v2.rs#L244-L255) vs [`python/visual_novel_engine.pyi:501-503`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L501-L503)
- **Descripción:**
  En Rust:
  ```rust
  #[pyo3(signature = (stage_width=None, stage_height=None, locale=None))]
  fn snapshot(
      &self,
      stage_width: Option<u32>,
      stage_height: Option<u32>,
      locale: Option<&str>,
  ) -> PyComposerSnapshot
  ```
  En `python/visual_novel_engine.pyi`:
  ```python
  class ComposerPreviewSession:
      def snapshot(
          self, selected_node_id: Optional[int] = ..., runtime_mode: bool = ...
      ) -> ComposerSnapshot: ...
  ```
  Los parámetros declarados en el stub de tipos no guardan ninguna relación con la función implementada en PyO3. Llamar a `snapshot` pasando `selected_node_id` o `runtime_mode` arroja una excepción en tiempo de ejecución.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_composer_preview_session_snapshot_kwargs_mismatch` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  ```python
  session.snapshot(selected_node_id=d)
  ```
  Salida:
  ```
  TypeError: ComposerPreviewSession.snapshot() got an unexpected keyword argument 'selected_node_id'
  ```
- **Fix sugerido:**
  Actualizar `visual_novel_engine.pyi` para reflejar la firma real: `def snapshot(self, stage_width: Optional[int] = ..., stage_height: Optional[int] = ..., locale: Optional[str] = ...) -> ComposerSnapshot: ...`.

---

### BUG-PY-04: Bloqueo de PyRefMut y Falla de Re-entrancia en `Engine.register_handler`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/py/src/bindings/engine.rs:175-187`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/engine.rs#L175-L187)
- **Descripción:**
  La función `PyEngine::step(&mut self, py)` toma un préstamo mutable exclusivo del objeto `PyEngine` gestionado por PyO3. Cuando encuentra un evento `ExtCall`:
  ```rust
  if let Some(handler) = &self.handler {
      let handler = handler.clone_ref(py);
      if let Err(e) = handler.call1(py, (command.as_str(), args.clone())) {
  ```
  Invoca sincrónicamente el handler registrado en Python mientras el préstamo `&mut self` del motor permanece activo en el stack de Rust.
  Si el handler de Python intenta consultar información del motor (`engine.visual_state()`, `engine.current_event()`) o despachar audio (`engine.audio()`), PyO3 detecta el conflicto de préstamos y aborta inmediatamente con `RuntimeError: Already mutably borrowed`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_engine_reentrancy_fails_with_borrow_error` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```
  RuntimeError: Already mutably borrowed
  ```
- **Fix sugerido:**
  En `step()`, para evitar mantener el préstamo mutable durante la llamada al código de usuario en Python, clonar los datos necesarios del `ExtCall`, finalizar o soltar el préstamo mutable antes de invocar `handler.call1()`, o estructurar el motor con celdas de interior mutability (`RefCell`/`RwLock`) que permitan lecturas compartidas de solo lectura durante la ejecución del hook.

---

### BUG-PY-05: Discrepancia de Nombres de Parámetro Kwargs en `AudioController` (`asset` vs `resource`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/audio.rs:21, 52, 63`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/audio.rs#L21) vs [`python/visual_novel_engine.pyi:47-56`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L47-L56)
- **Descripción:**
  En `.pyi`:
  ```python
  def play_bgm(self, asset: str, loop: bool = ..., fade_in: float = ..., volume: Optional[float] = ...) -> None: ...
  def play_sfx(self, asset: str, volume: Optional[float] = ...) -> None: ...
  def play_voice(self, asset: str, volume: Optional[float] = ...) -> None: ...
  ```
  En Rust:
  ```rust
  #[pyo3(signature = (resource, r#loop=true, fade_in=0.0, volume=None))]
  fn play_bgm(&self, py: Python<'_>, resource: &str, ...) -> PyResult<()>
  #[pyo3(signature = (resource, volume=None))]
  fn play_sfx(&self, py: Python<'_>, resource: &str, ...) -> PyResult<()>
  #[pyo3(signature = (resource, volume=None))]
  fn play_voice(&self, py: Python<'_>, resource: &str, ...) -> PyResult<()>
  ```
  Cualquier código Python que invoque `audio.play_bgm(asset="music.mp3")` guiado por los stubs falla con `TypeError`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_audio_controller_keyword_arg_mismatch` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```
  TypeError: AudioController.play_bgm() got an unexpected keyword argument 'asset'
  ```
- **Fix sugerido:**
  Renombrar el parámetro en `audio.rs` de `resource` a `asset`, o en el stub `.pyi` de `asset` a `resource`. Se recomienda estandarizar a `asset` en todo el workspace.

---

### BUG-PY-06: Deserialización de `EventCompiled::JumpIf` Omite Condición y Destino
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/conversion.rs:61-64`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/conversion.rs#L61-L64) vs [`python/vnengine/types.py:473-475`](file:///D:/Rust/audit/rustnovel/python/vnengine/types.py#L473-L475)
- **Descripción:**
  En `conversion.rs`:
  ```rust
  EventCompiled::JumpIf { target_ip, .. } => {
      dict.set_item("type", "jump_if")?;
      dict.set_item("target_ip", *target_ip)?;
  }
  ```
  No se serializa la condición (`cond`), ni el nombre de destino (`target`). Al intentar decodificar este diccionario con `vnengine.types.JumpIf.from_dict()`, este ejecuta:
  ```python
  cond = cond_from_dict(data.get("cond", {}))
  ```
  Al ser `cond` un diccionario vacío `{}`, `cond_from_dict` arroja `ValueError: Unknown condition kind: None`. Además, fallaría seguidamente en `data["target"]` con `KeyError`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_jump_if_conversion_drops_cond_and_target` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```
  ValueError: Unknown condition kind: None
  Event dict from Rust: {'type': 'jump_if', 'target_ip': 1}
  ```
- **Fix sugerido:**
  En `conversion.rs`, serializar la condición compilada (ej. `{"kind": "flag", "key": flag_id, "is_set": ...}`) y proporcionar `"target"` junto con `"target_ip"`.

---

### BUG-PY-07: Atributos Fantasma `label` y `description` en `QuickFixCandidate`
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_diagnostics.rs:10-28`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_diagnostics.rs#L10-L28) vs [`python/visual_novel_engine.pyi:367-373`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L367-L373)
- **Descripción:**
  En `visual_novel_engine.pyi`:
  ```python
  class QuickFixCandidate:
      fix_id: str
      label: str
      description: str
      risk: str
      structural: bool
  ```
  En Rust (`editor_diagnostics.rs`), los getters disponibles son: `fix_id`, `risk`, `structural`, `title_es`, `title_en`, `preconditions_es`, `preconditions_en`, `postconditions_es`, `postconditions_en`. Los atributos `label` y `description` no existen en la clase nativa.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_quick_fix_candidate_missing_pyi_attributes` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```
  AttributeError: 'QuickFixCandidate' object has no attribute 'label'
  ```
- **Fix sugerido:**
  Alinear `.pyi` con los campos reales internacionalizados (`title_es`, `title_en`, etc.) o agregar propiedades `label` (alias a `title_en`) y `description` en Rust para retrocompatibilidad.

---

### BUG-PY-08: Discordancia de Tipos en `FragmentPort` (`port_id: str` y `node_id: Optional[int]`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_api_v2/fragments.rs:14, 24`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_api_v2/fragments.rs#L14) vs [`python/visual_novel_engine.pyi:427-431`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L427-L431)
- **Descripción:**
  En `visual_novel_engine.pyi`:
  ```python
  class FragmentPort(_JsonRoundtrip):
      node_id: int
      port_id: int
      label: str
  ```
  En Rust:
  - `port_id` es un `String` (`self.inner.port_id.clone()`).
  - `node_id` es un `Option<u32>` (`self.inner.node_id`), pudiendo ser `None` si el puerto aún no está asignado a un nodo.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_fragment_port_types_mismatch` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida confirmada: `port_id` es `<class 'str'>` y `node_id` es `None`.
- **Fix sugerido:**
  Actualizar `python/visual_novel_engine.pyi`:
  ```python
  class FragmentPort(_JsonRoundtrip):
      node_id: Optional[int]
      port_id: str
      label: str
  ```

---

### BUG-PY-09: Tipos de Retorno Incompatibles en `Track.evaluate` y `Timeline.evaluate`
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/timeline.rs:146, 245, 254`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/timeline.rs#L146) vs [`python/visual_novel_engine.pyi:353, 362, 365`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L353)
- **Descripción:**
  En `python/visual_novel_engine.pyi`:
  ```python
  class Track:
      def evaluate(self, time: int) -> Dict[str, Any]: ...
  class Timeline:
      def evaluate_at(self, time: int) -> Dict[str, Any]: ...
      def evaluate(self) -> Dict[str, Any]: ...
  ```
  En Rust:
  - `Track.evaluate(time: u32) -> Option<i32>` (retorna `int` o `None`).
  - `Timeline.evaluate(&self) -> Vec<(u32, String, i32)>` (retorna `List[Tuple[int, str, int]]`).
  - `Timeline.evaluate_at(&self, time: u32) -> Vec<(u32, String, i32)>`.
  Ninguno de los métodos devuelve un diccionario. Código que intente acceder como diccionario (`result["key"]`) arrojará `TypeError`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_timeline_and_track_evaluate_return_types` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida confirmada: `Track.evaluate` retorna `int` / `None`; `Timeline.evaluate` retorna `[(1, 'PositionX', 10)]`.
- **Fix sugerido:**
  Corregir las anotaciones en `visual_novel_engine.pyi`:
  - `Track.evaluate(...) -> Optional[int]`
  - `Timeline.evaluate(...) -> List[Tuple[int, str, int]]`
  - `Timeline.evaluate_at(...) -> List[Tuple[int, str, int]]`

---

### BUG-PY-10: Tipo de Retorno Erróneo en `NodeGraph.connect_new_choice_option` (`int`, no `bool`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_node_graph.rs:88-99`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_node_graph.rs#L88-L99) vs [`python/visual_novel_engine.pyi:184-186`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L184-L186)
- **Descripción:**
  En Rust:
  ```rust
  fn connect_new_choice_option(&mut self, choice_id: u32, to_id: u32, text: &str) -> PyResult<usize> {
      ...
      match outcome.delta {
          AuthoringDelta::ChoiceOptionConnected { option_index, .. } => Ok(option_index),
          ...
      }
  }
  ```
  Retorna el índice entero de la opción creada (`usize`).
  En `.pyi`:
  ```python
  def connect_new_choice_option(self, choice_id: int, to_id: int, text: str = ...) -> bool: ...
  ```
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_connect_new_choice_option_returns_int_not_bool` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida confirmada: `<class 'int'> 1`.
- **Fix sugerido:**
  Actualizar el retorno en `.pyi` a `-> int`.

---

### BUG-PY-11: Tipo de Retorno Erróneo en `NodeGraph.fragment_ports` (`Tuple[List, List]`, no `List[FragmentPort]`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_node_graph.rs:288`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_node_graph.rs#L288) vs [`python/visual_novel_engine.pyi:228`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L228)
- **Descripción:**
  En Rust:
  ```rust
  fn fragment_ports(&self, fragment_id: &str) -> Option<(Vec<PyFragmentPort>, Vec<PyFragmentPort>)>
  ```
  Devuelve una tupla de dos listas: `(inputs, outputs)`, o `None` si el fragmento no existe.
  En `.pyi`:
  ```python
  def fragment_ports(self, fragment_id: str) -> List["FragmentPort"]: ...
  ```
  Código que itere asumiendo una lista plana de puertos (`for port in graph.fragment_ports(id): port.node_id`) fallará con `AttributeError: 'list' object has no attribute 'node_id'`, ya que el primer elemento iterado es la lista de inputs completa.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_fragment_ports_returns_tuple_of_lists` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida: `<class 'tuple'> 2 [<class 'list'>, <class 'list'>]`.
- **Fix sugerido:**
  Actualizar el stub `.pyi` a: `def fragment_ports(self, fragment_id: str) -> Optional[Tuple[List["FragmentPort"], List["FragmentPort"]]]: ...`.

---

### BUG-PY-12: Tipo de Retorno Erróneo en `NodeGraph.autofix_issue` (`Option[str]`, no `bool`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_node_graph.rs:391`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_node_graph.rs#L391) y [`crates/py/src/bindings/editor_node_graph/misc.rs:44`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_node_graph/misc.rs#L44) vs [`python/visual_novel_engine.pyi:262`](file:///D:/Rust/audit/rustnovel/python/visual_novel_engine.pyi#L262)
- **Descripción:**
  En Rust:
  ```rust
  fn autofix_issue(&mut self, issue_index: usize, include_review: bool) -> PyResult<Option<String>>
  ```
  Retorna `Ok(Some(fix_id))` (el identificador de la corrección aplicada como string) o lanza `PyValueError` si no hay candidato. Nunca retorna `bool`.
  En `.pyi`:
  ```python
  def autofix_issue(self, issue_index: int, include_review: bool = ...) -> bool: ...
  ```
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_autofix_issue_returns_str_on_success` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida confirmada: retorna `<class 'str'>` con el id de fix.
- **Fix sugerido:**
  Actualizar `.pyi` a `-> Optional[str]`.

---

### BUG-PY-13: Inconsistencias de Nombres de Kwargs en Métodos de Grafo y Timeline
- **Severidad:** Media
- **Archivo:Línea:** Múltiples archivos de bindings vs `.pyi`
- **Descripción:**
  1. `NodeGraph.connect`: pyi `(from_node, to_node)` vs Rust `(from_id, to_id)`.
  2. `NodeGraph.connect_port`: pyi `(from_node, from_port, to_node)` vs Rust `(from_id, from_port, to_id)`.
  3. `NodeGraph.connect_or_branch`: pyi `(from_node, from_port, to_node)` vs Rust `(from_id, from_port, to_id)`.
  4. `Engine.register_handler`: pyi `handler` vs Rust `callback`.
  5. `Track.add_keyframe`: pyi `keyframe` vs Rust `kf`.
  6. `Timeline.advance`: pyi `ticks` vs Rust `delta_ticks`.
  Al pasar argumentos por palabra clave según la especificación del `.pyi`, PyO3 arroja `TypeError: unexpected keyword argument`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_method_kwarg_name_mismatches` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
- **Fix sugerido:**
  Armonizar los nombres de argumentos en las macros `#[pyo3(signature = ...)]` en Rust para que coincidan con la convención descriptiva de `.pyi`.

---

### BUG-PY-14: Asimetría en Trimming de Rutas en `PyAssetCache`
- **Severidad:** Media
- **Archivo:Línea:** [`crates/py/src/bindings/engine.rs:380, 399, 408`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/engine.rs#L380)
- **Descripción:**
  En `cache_asset`:
  ```rust
  let asset_path = asset_path.trim();
  ```
  Sin embargo, en `get_cached_asset`:
  ```rust
  fn get_cached_asset<'py>(&mut self, py: Python<'py>, asset_path: &str) -> PyResult<Option<PyObject>> {
      if let Some(data) = self.asset_cache.get(asset_path) { ... }
  }
  ```
  Y en `invalidate_cached_asset`:
  ```rust
  fn invalidate_cached_asset(&mut self, asset_path: &str) -> bool {
      self.asset_cache.remove(asset_path)
  }
  ```
  Ninguno de los métodos de consulta o eliminación recorta la cadena. Al consultar la caché con la misma cadena que se pasó a `cache_asset` (si contenía espacios al inicio o final), `get_cached_asset` falla silenciosamente devolviendo `None` y aumentando el contador de fallos de caché (`cache_misses`).
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_asset_cache_trimming_inconsistency` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```python
  get_cached_asset("  asset.png  ") == None
  get_cached_asset("asset.png") == b"data"
  ```
- **Fix sugerido:**
  Aplicar `let asset_path = asset_path.trim();` de manera consistente en `get_cached_asset` e `invalidate_cached_asset`.

---

### BUG-PY-15: Restricción Innecesaria de Tuplas en `StoryNode.scene_patch_full`
- **Severidad:** Baja/Media
- **Archivo:Línea:** [`crates/py/src/bindings/editor_story_node.rs:200-230`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_story_node.rs#L200-L230)
- **Descripción:**
  En `scene_patch_full`:
  ```rust
  fn scene_patch_full(
      background: Option<String>,
      music: Option<String>,
      add: Vec<PyCharacterPlacementFullInput>, // 6-tupla: (name, expr, pos, x, y, scale)
      update: Vec<PyCharacterPlacementInput>,   // 3-tupla: (name, expr, pos)  <-- LIMITACIÓN
      remove: Vec<String>,
  ) -> Self
  ```
  El struct subyacente `CharacterPatchRaw` en `crates/core` sí admite campos `x`, `y`, `scale`. Sin embargo, los bindings de Python en `scene_patch_full` fuerzan que `update` sea una 3-tupla y descartan explícitamente las coordenadas (`x: None, y: None, scale: None`). Si Python intenta pasar una 6-tupla a `update` (como en `add`), PyO3 arroja `ValueError: expected tuple of length 3, but got tuple of length 6`.
- **Test que lo reproduce:**
  `AuditAdversarialBindingsTests.test_scene_patch_full_update_rejects_6_tuples` en [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py).
  Salida:
  ```
  ValueError: expected tuple of length 3, but got tuple of length 6
  ```
- **Fix sugerido:**
  Permitir que `update` en `scene_patch_full` acepte `PyCharacterPlacementFullInput` (o una variante con tupla completa) para permitir modificar la posición de personajes en parches de escena.

---

## 3. Sospechas

1. **Uso de `.expect_err()` a través de la barrera FFI en `engine.rs:197`**:
   - [`crates/py/src/bindings/engine.rs:197`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/engine.rs#L197):
   ```rust
   let err = self.inner.complete_external_call(...).expect_err("failed ext_call outcome must not advance");
   ```
   Aunque la implementación actual del runtime en `crates/core` devuelve un `Err` cuando el estado es fallido, depender de `.expect_err()` dentro de un método expuesto a Python puede causar un pánico que cruce la frontera FFI si alguna validación previa o cambio futuro en `core` devolviera `Ok`. Se debe usar `match` o `if let Err(...)`.

2. **Stack Overflow Potencial en `json_value_to_py` por Recursión sin Límite**:
   - [`crates/py/src/bindings/types.rs:256-289`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/types.rs#L256-L289):
   `json_value_to_py` convierte recursivamente `serde_json::Value` a estructuras nativas de Python (`PyDict`, `PyList`). Si se recibe un árbol JSON con miles de niveles de anidamiento, provocará un stack overflow en el hilo de ejecución sin posibilidad de captura por parte de Python.

3. **Validación Incompleta en `validate_runtime_config`**:
   - [`crates/py/src/lib.rs:60-75`](file:///D:/Rust/audit/rustnovel/crates/py/src/lib.rs#L60-L75):
   La función `validate_runtime_config` acepta `config: Option<PyVnConfig>`, pero de todas sus propiedades (título, dimensiones, modo de seguridad, rutas de manifiesto, caché de assets), únicamente valida `player_menu_json`. Valores inválidos en `security_mode` o presupuestos corruptos pasan inadvertidos.

---

## 4. Simplificaciones

1. **Eliminación de Clases Fantasma en `.pyi`**:
   `_JsonBacked` y `_JsonRoundtrip` se encuentran en `visual_novel_engine.pyi` como jerarquías base, pero no existen en el módulo nativo exportado por Rust (`hasattr(vne, '_JsonBacked') == False`). Se recomienda remover estas clases o marcarlas explícitamente como `typing.Protocol`.

2. **Deduplicación de `parse_cmp_op`**:
   La función `parse_cmp_op` está duplicada de forma idéntica en:
   - [`crates/py/src/bindings/builder.rs:34-46`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/builder.rs#L34-L46)
   - [`crates/py/src/bindings/editor_support.rs:7-19`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_support.rs#L7-L19)
   Debe consolidarse en `conversion.rs` o `types.rs`.

3. **Racionalización de Wrappers Ciegos `json_wrapper!`**:
   En [`crates/py/src/bindings/editor_api_v2.rs:16-60`](file:///D:/Rust/audit/rustnovel/crates/py/src/bindings/editor_api_v2.rs#L16-L60), la macro `json_wrapper!` genera 9 clases completas (`DiagnosticTarget`, `FieldPath`, `SemanticValue`, `EvidenceTrace`, `TraceAtom`, `TraceEdge`, `OperationStatus`, `OperationLogEntry`, `VerificationRun`) que no exponen absolutamente ningún getter ni método más allá de `from_json` y `to_json`. Representan sobreingeniería de código contenedor sin utilidad directa en Python; o bien se les agregan getters útiles, o bien se exponen directamente como diccionarios/strings JSON.

4. **Corrección de Semántica de Retorno Booleana en `NodeGraph`**:
   Métodos como `create_fragment`, `remove_fragment`, `enter_fragment`, `edit_dialogue`, `edit_choice_prompt` devuelven `PyResult<bool>` pero en la práctica siempre devuelven `Ok(true)` o un `Err` (`map(|_| true)`). En `.pyi` aparecen como `-> bool`, sugiriendo falsamente que pueden retornar `False` ante un error recuperable. Deberían devolver `-> None` y documentar que lanzan excepciones ante errores.

---

## 5. Mejoras

1. **Adopción de Generación Automática de Stubs (`pyo3-stub-gen`)**:
   La gran cantidad de discrepancias entre `.pyi` y Rust se debe a la edición manual desincronizada del archivo `.pyi`. Incorporar `pyo3-stub-gen` o un paso en CI que valide `stubtest visual_novel_engine` evitaría la aparición de estos errores.
2. **Soporte de Punto Flotante en `Keyframe`**:
   Actualmente `Keyframe.value` es estrictamente `i32`. Propiedades de animación como `scale` (factores de escala 1.25) u `opacity` (0.0 - 1.0) no pueden representarse de forma continua sin recurrir a escalado artificial entero. `Keyframe` debería aceptar `f64` o un enum numérico convertible.
3. **Manejo de Errores con Reportes Miette Limpios**:
   `vn_error_to_py` convierte el error con `miette::Report::new(err).to_string()`. Se recomienda verificar que las cadenas no contengan códigos de escape ANSI cuando se ejecuten en entornos de Python donde la salida se guarde en logs planos.
4. **Pruebas de Integración Rust en `crates/py`**:
   `Cargo.toml` de `crates/py` define `test = false` en `[lib]`. Se recomienda habilitar pruebas o agregar pruebas de integración que aseguren que los bindings puedan validarse desde `cargo test`.

---

## 6. Tests Añadidos y Cómo Ejecutarlos

Se han añadido pruebas completas que reproducen y confirman dinámicamente todos los bugs detallados:

### Ubicación de los Tests:
- Script de prueba adversarial completa:
  [`D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py`](file:///D:/Rust/audit/scratch/python_bindings/audit_adversarial_tests.py)
- Script de comparación exhaustiva de API / firmas:
  [`D:\Rust\audit\scratch\python_bindings\compare_signatures.py`](file:///D:/Rust/audit/scratch/python_bindings/compare_signatures.py)
- Script de comparación de miembros y clases exportadas:
  [`D:\Rust\audit\scratch\python_bindings\compare_api.py`](file:///D:/Rust/audit/scratch/python_bindings/compare_api.py)

### Cómo Ejecutar las Pruebas:

1. **Configurar el entorno en PowerShell**:
```powershell
$env:PATH="D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64\Scripts;$env:PATH"
$env:PYTHONPATH="D:\Rust\audit\rustnovel\target\debug;D:\Rust\audit\rustnovel\python;D:\Rust\audit\rustnovel\tests\python"
```

2. **Ejecutar la suite adversarial (15 pruebas confirmadas)**:
```powershell
python D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py
# O con pytest:
pytest D:\Rust\audit\scratch\python_bindings\audit_adversarial_tests.py -v
```

3. **Ejecutar la suite de pruebas existente del repositorio**:
```powershell
pytest D:\Rust\audit\rustnovel\tests\python -v
```
