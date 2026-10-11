# Reporte de Auditoría y Depuración: Suites de Pruebas GUI, CLI, Runtime y Python

- **Fecha:** 2026-10-10
- **Área:** `audit_and_debug_gui_cli_python_tests`
- **Repositorio:** `D:\Rust\audit\rustnovel`
- **Suites Auditadas:**
  - `crates/gui/tests`
  - `tools/cli/tests`
  - `crates/runtime/tests`
  - `tests/python`

---

## 1. Resumen Ejecutivo

Se ejecutó una auditoría exhaustiva y sistemática de las suites de pruebas de integración y contratos de la GUI, herramientas CLI, runtime y bindings de Python.

Durante la ejecución se detectaron:
1. **Dos fallos de prueba activos (regresión real en GUI):** El viewport del compositor visual (`crates/gui/src/editor/visual_composer/viewport.rs`) resta la altura reservada dos veces, causando el pánico y fallo de dos tests de contrato (`composer_viewport_uses_reserved_stage_height_without_double_status_deduction` en `editor_contract_tests.rs` y `visual_composer_dialogue_overlay_keeps_stage_visible_in_compact_window` en `gui_render_contract_tests.rs`).
2. **Tests simulados o vacíos (smoke tests ficticios):** Tests que afirman validar comportamientos dinámicos o de GUI pero en realidad solo instancian structs con `Default::default()` o calculan fórmulas matemáticas locales sin ejercitar el pipeline gráfico o de interacción.
3. **Rutas fijas y no portables:** Rutas Unix `/tmp` hardcodeadas en tests de Python, rutas con unidades de Windows (`C:/project-a`) y barras invertidas en tests de Rust, y creación de directorios temporales en rutas fijas no aisladas relativas a `cwd()`.
4. **Fallas en la ejecución y diseño de tests en Python:** Incompatibilidad con `unittest discover` debido a una dependencia de nivel superior `import pytest` no declarada en el entorno base, y prohibición estricta de `@pytest.mark.skip` mediante ganchos en `conftest.py`.
5. **Carencias en pruebas de CLI y empaquetado:** `package_command_tests.rs` no valida códigos de salida específicos (1, 2, 3) ni prueba la emisión del sobre JSON de error ante fallos (`--json` solo se ejercita ante éxitos).
6. **Condiciones de carrera y timeouts frágiles:** Carrera de hilos en `loader_contract_tests.rs` donde `is_loading()` puede retornar `false` antes de que el hilo de prueba ejecute el `assert!`, junto con timeouts arbitrarios por `sleep`.

---

## 2. Bug Activo y Fallos Reales en la Suite de Tests

Al ejecutar la suite de pruebas de `visual_novel_gui`:
- `cargo test -p visual_novel_gui --test editor_contract_tests -- --test-threads=1`
- `cargo test -p visual_novel_gui --test gui_render_contract_tests -- --test-threads=1`

Se encontraron dos fallos consistentes derivados de la misma causa raíz.

### Fallo A: `composer_viewport_uses_reserved_stage_height_without_double_status_deduction`
- **Ubicación del Test:** `crates/gui/tests/editor_contract/visual_composer_tests.rs:245:5`
- **Mensaje de Pánico:**
  ```text
  thread 'visual_composer_tests::cases::composer_viewport_uses_reserved_stage_height_without_double_status_deduction' panicked at crates/gui/tests/editor_contract/visual_composer_tests.rs:245:5:
  stage viewport should consume the reserved stage band instead of subtracting footer/status rows twice: [560.0, 132.0]
  ```

### Fallo B: `visual_composer_dialogue_overlay_keeps_stage_visible_in_compact_window`
- **Ubicación del Test:** `crates/gui/tests/gui_render_contract_tests.rs:769:5`
- **Mensaje de Pánico:**
  ```text
  thread 'visual_composer_dialogue_overlay_keeps_stage_visible_in_compact_window' panicked at crates/gui/tests/gui_render_contract_tests.rs:769:5:
  dialogue overlay and overlay edit should not collapse the visual composer stage; max painted height was 100.5
  ```

### Causa Raíz
- **Archivo Fuente:** `crates/gui/src/editor/visual_composer/viewport.rs:3-13`
- **Explicación:**
  En `crates/gui/src/editor/visual_composer.rs:360-365`:
  ```rust
  let visible_remaining = (panel_clip.bottom() - ui.cursor().min.y).max(0.0);
  let viewport_available_height =
      composer_stage_available_height(visible_remaining, self.selected_authoring_node);
  let viewport_size = viewport::composer_viewport_size(
      egui::vec2(ui.available_width(), viewport_available_height),
      self.stage_size(),
  );
  ```
  La función `composer_stage_available_height` **ya ha descontado** la fila de estado (`status_row`) y el editor de superposiciones (`overlay_editor`) de `visible_remaining`.
  Sin embargo, en `crates/gui/src/editor/visual_composer/viewport.rs`:
  ```rust
  const RESERVED_HEIGHT: f32 = 28.0;
  const VIEWPORT_VERTICAL_BUDGET: f32 = 1.0;

  pub fn composer_viewport_size(available: egui::Vec2, stage_size: (f32, f32)) -> egui::Vec2 {
      crate::editor::visual_composer_preview::stage_viewport_size(
          available,
          stage_size,
          RESERVED_HEIGHT,
          VIEWPORT_VERTICAL_BUDGET,
      )
  }
  ```
  `composer_viewport_size` pasa `RESERVED_HEIGHT = 28.0` a `stage_viewport_size`. Esto provoca que los 28px se resten una segunda vez (`usable_height = available.y - 28.0`), colapsando la altura útil de 160.0 a 132.0 en el test unitario, y a 100.5 en el test de renderizado con overlay compacto, violando el contrato de altura mínima del escenario.

### Arreglo Mínimo Propuesto
En `crates/gui/src/editor/visual_composer/viewport.rs`:
```rust
const RESERVED_HEIGHT: f32 = 0.0;
```
Al fijar `RESERVED_HEIGHT = 0.0`, el viewport consume exactamente la franja presupuestada por `composer_stage_available_height` sin duplicar la deducción de la fila de estado.

---

## 3. Área 1: Tests Simulados o Vacíos (Smoke Tests Ficticios)

Se identificaron tests que aparentan validar contratos complejos de interfaz o flujos de usuario, pero solo comprueban valores de inicialización por defecto o realizan cálculos aislados sin interactuar con el sistema real:

1. **`crates/gui/tests/editor_contract/player_ui_tests.rs:39-45`**
   - **Test:** `player_menu_preview_starts_uninitialized_until_play_mode_enters`
   - **Problema:** El título indica que el menú permanece sin inicializar *hasta que se entra en modo juego*, pero el test solo hace:
     ```rust
     let state = PlayerSessionState::default();
     assert!(!state.menu_initialized);
     assert!(!state.show_menu);
     assert_eq!(state.menu_tab, PlayerMenuTabKind::Saves);
     ```
     Nunca se invoca la transición al modo juego (`play_mode`) ni se valida que la entrada al modo juego realmente inicialice el menú o cambie estas banderas. Es un test trivial sobre `Default::default()`.

2. **`crates/gui/tests/editor_contract/workbench/core_tests.rs:9-39`**
   - **Test:** `test_workbench_initialization`
   - **Problema:** Pretende probar la inicialización y el avance del Workbench del editor. Crea `EditorWorkbench::new(config)`, modifica directamente la variable mutable `workbench.is_playing = true`, llama a `workbench.update(1)` y comprueba `workbench.current_time > 0.0`. No renderiza ningún fotograma de `egui`, no valida la disposición de paneles ni las ventanas acoplables.

3. **`crates/gui/tests/editor_contract/node_rendering_tests.rs:4-32`**
   - **Test:** `test_bezier_connection_does_not_panic`
   - **Problema:** En lugar de probar la función del motor de renderizado `draw_bezier_connection` o el pintor de `egui`, el test ejecuta un bucle local propio que evalúa una fórmula polinómica de Bézier cúbica y verifica `points[0] == from` y `points[20] == to`. No valida si el código de producción renderiza correctamente ni si maneja viewports fuera de pantalla.

4. **`crates/runtime/tests/input_contract_tests.rs:5-34`**
   - **Test:** `pointer_action_maps_choice_buttons_from_design_to_window_coordinates`
   - **Problema:** Construye un struct en memoria `SceneFrame` con botones y coordenadas artificiales y llama a la función de mapeo numérico `pointer_action_for_scene_frame`. No interactúa con el despachador de eventos de `winit`, ni valida la integración con el bucle de eventos del runtime.

---

## 4. Área 2: Rutas Fijas y No Portables

Se detectaron varias prácticas de rutas no portables que comprometen la ejecución multiplataforma (Linux / Windows / macOS / CI):

1. **Rutas Unix absolutas fijas en tests de Python (`/tmp`):**
   - **Archivos:**
     - `tests/python/test_vnengine_bindings.py:101, 110`
     - `tests/python/test_vnengine.py:127, 148`
   - **Fragmento:**
     ```python
     module.__file__ = "/tmp/visual_novel_engine.py"
     ...
     self.assertIn("/tmp/visual_novel_engine.py", message)
     ```
   - **Impacto:** En Windows `/tmp` no existe; si el mensaje de error o la capa nativa normaliza las rutas a rutas del sistema de archivos mediante `Path.resolve()` o separadores de barra invertida (`\`), la aserción de coincidencia de subcadena fallará.

2. **Uso de `Path.cwd()` y carpetas fijas en tests de Python:**
   - **Archivos:**
     - `tests/python/test_vnengine_bindings.py:18-27`
     - `tests/python/test_vnengine_composer_report_bindings.py:11-20`
     - `tests/python/test_vnengine_graph_bindings.py:12-21`
   - **Fragmento:**
     ```python
     @contextmanager
     def workspace_tempdir(name: str):
         root = Path.cwd() / "target" / "python-test-tmp" / name
         if root.exists():
             shutil.rmtree(root)
         root.mkdir(parents=True)
         try:
             yield root
         finally:
             shutil.rmtree(root, ignore_errors=True)
     ```
   - **Impacto:**
     - Si los tests se ejecutan desde un directorio distinto a la raíz del repositorio (ej. `python -m unittest discover tests/python`), la carpeta se crea en `tests/python/target/...`.
     - Si los tests se ejecutan en paralelo con múltiples procesos (`pytest -n auto`), diferentes casos que usen el mismo nombre colisionarán y borrarán carpetas en uso.
     - **Recomendación:** Usar `tempfile.TemporaryDirectory()`.

3. **Rutas absolutas con letra de unidad de Windows en tests de Rust:**
   - **Archivo:** `crates/gui/tests/editor_contract/scene_stage_tests.rs:190-192`
   - **Fragmento:**
     ```rust
     let asset_path = "assets\\backgrounds\\room.png";
     let root_a = std::path::Path::new("C:/project-a");
     let root_b = std::path::Path::new("C:/project-b");
     ```
   - **Impacto:** En plataformas Unix / Linux, `C:/project-a` es interpretado como una ruta relativa con un directorio llamado `C:`, rompiendo la semántica esperada de raíz absoluta en CI. Además, `asset_path` incluye barras invertidas no canónicas (`\\`).

4. **Barras invertidas en aserciones de scripts generados:**
   - **Archivo:** `tools/cli/tests/package_command_tests.rs:428`
   - **Fragmento:**
     ```rust
     assert!(launcher.contains("scripts\\compiled.vnscript.json"));
     ```
   - **Impacto:** Si se generaliza la generación de scripts de lanzamiento a entornos multiplataforma o emulados (como Wine / MinGW / WSL), las comparaciones con barras invertidas fallan.

---

## 5. Área 3: Paridad de Tests Python vs Rust y Entorno de Ejecución

1. **Estado de `@pytest.mark.skip`:**
   - No se encontraron tests decorados con `@pytest.mark.skip` ni `@pytest.mark.skipif`.
   - La configuración en `tests/python/conftest.py` contiene ganchos específicos (`pytest_collection_modifyitems` y `pytest_runtest_makereport`) diseñados explícitamente para prohibir cualquier test saltado:
     ```python
     def pytest_collection_modifyitems(config, items):
         ...
         if skipped:
             raise pytest.UsageError(f"Skipped Python tests are not allowed:\n{joined}")
     ```
2. **Deficiencia de Ejecución y Dependencias:**
   - En el entorno Python proporcionado (`pythoncore-3.14-64`), el paquete `pytest` no está instalado.
   - En `tests/python/test_vnengine_bindings.py:11`, existe una sentencia incondicional a nivel de módulo:
     ```python
     import pytest
     ```
   - Al ejecutar `python -m unittest discover -s tests/python`, el cargador de tests falla inmediatamente con `ModuleNotFoundError: No module named 'pytest'`, impidiendo la ejecución de 14 pruebas de contrato y cobertura de excepciones.
   - Los archivos de prueba en `tests/python` mezclan tests basados en `unittest.TestCase` con funciones que requieren fixtures de pytest (`test_pytest_conftest_is_loaded(vnengine_pytest_conftest_loaded)`).
3. **Paridad de Contratos y APIs:**
   - Las APIs de autoría modernas (operaciones de reversión/rehacer del grafo, importación/exportación de reportes de diagnóstico v2 y validaciones semánticas) están parcialmente representadas en Python mediante serialización JSON manual (`test_vnengine_composer_report_bindings.py`), pero carecen de tipos fuertemente tipados en Python como los que existen en Rust (por ejemplo, `DiagnosticTarget::ChoiceOption` y `StorySemanticFingerprint`).

---

## 6. Área 4: Pruebas de Empaquetado y CLI (`package_command_tests.rs` y `editor_contract_tests.rs`)

1. **Auditoría de `tools/cli/tests/package_command_tests.rs`:**
   - **Validación de códigos de salida ante fallos:**
     - En todos los tests negativos (`package_command_require_executable_rejects_missing_runtime`, `package_command_require_executable_rejects_fake_exe_payload`, `package_command_hmac_mode_requires_key`), únicamente se valida:
       ```rust
       assert!(!output.status.success());
       ```
     - Ninguna prueba verifica el código de salida exacto (`output.status.code() == Some(1)` o `Some(2)` según la clasificación de `classify_error` en `vnengine.rs`).
   - **Validación del reporte JSON ante fallos:**
     - **Ausencia total:** Ninguno de los tests de fallo ejecuta el comando con la bandera `--json`.
     - Todos los tests que usan `--json` (`package_command_global_json_emits_envelope_only`, `package_command_json_execute_matches_written_report_manifest_and_compat_flow`, `package_command_plan_dry_run_does_not_write_bundle`) son exclusivamente pruebas de ÉXITO (`ok: true`, `code: "ok"`).
     - La suite no verifica que `vnengine --json package` emita el sobre de fallo estructurado `{"ok": false, "code": "engine_error", "data": null, "error": "..."}`, ni verifica que en modo JSON el canal `stderr` permanezca completamente en blanco.
2. **Auditoría de `crates/gui/tests/editor_contract_tests.rs`:**
   - El asistente de empaquetado del Workbench se prueba en `crates/gui/tests/editor_contract/workbench/core_tests.rs:128` (`export_wizard_report_summary_matches_real_export_flow_outputs`).
   - Esta prueba únicamente valida el flujo exitoso invocando directamente `visual_novel_engine::export_bundle(...).expect("export flow")`.
   - **Cero validación de errores en el editor:** No existe ninguna prueba que verifique cómo el asistente de exportación o el diálogo de la GUI responde ante fallos de empaquetado (como ejecutables de runtime corruptos, fallos de HMAC o scripts no compilables), ni se valida el contenido del reporte de diagnóstico cuando la exportación falla.

---

## 7. Área 5: Tiempos de Espera Frágiles y Carreras (Race Conditions)

1. **Condición de carrera en `crates/runtime/tests/loader_contract_tests.rs`:**
   - **Test:** `async_loader_reports_success_and_inflight_state`
   - **Código:**
     ```rust
     let loader = AsyncLoader::new();
     let id = AssetId::from_path("test_asset");
     assert!(!loader.is_loading());
     loader
         .enqueue(id, PathBuf::from("Cargo.toml"))
         .expect("enqueue cargo manifest");
     assert!(loader.is_loading()); // <-- CONDICIÓN DE CARRERA
     let result = wait_for_result(&loader);
     ```
   - **Explicación:** `AsyncLoader::new()` inicia un hilo de fondo que procesa solicitudes inmediatamente. Dado que `"Cargo.toml"` es un archivo local pequeño, si el planificador del sistema operativo conmuta el hilo antes de la línea `assert!(loader.is_loading())`, el hilo de trabajo lee el archivo y ejecuta `inflight_thread.fetch_sub(1, Ordering::Release)` antes de la aserción. Cuando el hilo del test retoma la ejecución, `loader.is_loading()` ya es `false`, provocando un fallo intermitente espurio en entornos de integración continua o máquinas bajo carga.
2. **Timeout rígido en `wait_for_result`:**
   - **Código:**
     ```rust
     fn wait_for_result(loader: &AsyncLoader) -> vnengine_runtime::LoadResult {
         for _ in 0..100 {
             if let Some(res) = loader.try_recv().expect("...") {
                 return res;
             }
             std::thread::sleep(Duration::from_millis(10));
         }
         panic!("loader should complete");
     }
     ```
   - **Problema:** Límite estricto de 1.000 ms (100 iteraciones de 10 ms). En sistemas con alta contención de I/O de disco o runners compartidos en la nube, lecturas de archivos pueden superar fácilmente 1 segundo, generando pánicos falsos.
3. **Condición de carrera en `tests/python` por carpetas compartidas:**
   - Las funciones auxiliares `workspace_tempdir` en `test_vnengine_bindings.py`, `test_vnengine_composer_report_bindings.py` y `test_vnengine_graph_bindings.py` usan rutas estáticas compartidas (`target/python-test-tmp/<name>`). La ejecución concurrente o distribuida de tests provoca interferencias mutuas y fallos de permisos al intentar borrar y recrear la misma carpeta simultáneamente.

---

## 8. Matriz de Hallazgos y Acciones Recomendadas

| ID | Área | Componente | Severidad | Descripción del Problema | Acción Recomendada |
|---|---|---|---|---|---|
| **BUG-01** | Regresión GUI | `crates/gui/src/editor/visual_composer/viewport.rs:3` | **Alta** | Doble deducción de 28px en viewport colapsa el escenario y causa pánico en 2 tests. | Cambiar `RESERVED_HEIGHT = 0.0`. |
| **TEST-01** | Smoke Test | `crates/gui/tests/editor_contract/player_ui_tests.rs:39` | **Media** | Test `player_menu_preview_starts_uninitialized_until_play_mode_enters` solo evalúa `Default::default()`. | Agregar transición real a modo juego y verificar mutación de estado. |
| **TEST-02** | Smoke Test | `crates/gui/tests/editor_contract/node_rendering_tests.rs:4` | **Baja** | `test_bezier_connection_does_not_panic` evalúa polinomio local sin llamar al renderizador. | Reemplazar por test que invoque `draw_bezier_connection` sobre un `egui::Painter`. |
| **PORT-01** | Portabilidad | `tests/python/test_vnengine_bindings.py:101` | **Media** | `/tmp/visual_novel_engine.py` hardcodeado en aserciones de mensaje de error. | Usar `Path.resolve()` o rutas multiplataforma generadas con `tempfile`. |
| **PORT-02** | Portabilidad | `crates/gui/tests/editor_contract/scene_stage_tests.rs:191` | **Media** | Rutas `C:/project-a` y barras invertidas `\\` en tests de cache key. | Reemplazar por rutas generadas mediante `tempfile::tempdir()`. |
| **PY-01** | Tooling Python | `tests/python/test_vnengine_bindings.py:11` | **Alta** | `import pytest` a nivel raíz impide ejecutar la suite con `python -m unittest discover`. | Mover importación dentro de los tests específicos o declarar pytest en el entorno de testing. |
| **CLI-01** | Cobertura CLI | `tools/cli/tests/package_command_tests.rs:361` | **Media** | No se validan códigos de salida (1, 2, 3) ni respuestas `--json` en escenarios de error de `package`. | Agregar aserciones de `output.status.code()` y probar `--json` con errores de empaquetado. |
| **RACE-01** | Concurrencia | `crates/runtime/tests/loader_contract_tests.rs:44` | **Media** | Carrera entre el hilo de trabajo de `AsyncLoader` y `assert!(loader.is_loading())`. | Sincronizar el estado encolado antes de procesar o usar canales con barrera. |
| **RACE-02** | Concurrencia | `tests/python/test_vnengine_*.py` | **Baja** | `workspace_tempdir` usa rutas fijas bajo `target/python-test-tmp/`. | Migrar a `tempfile.TemporaryDirectory()`. |
