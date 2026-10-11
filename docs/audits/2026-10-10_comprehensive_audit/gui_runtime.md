# Informe de Auditoría: GUI y Runtime (`visual_novel_gui` y `vnengine_runtime`)

**Área auditada**: `gui_runtime` (`crates/gui` y `crates/runtime`)  
**Fecha**: 11 de Octubre de 2026  
**Auditor**: Senior Rust Security & Architecture Auditor  
**Workspace**: `D:\Rust\audit\rustnovel`  

---

## 1. Resumen Ejecutivo

Se ha realizado una auditoría exhaustiva de los crates `crates/gui` (`visual_novel_gui`, ~26.248 líneas de código) y `crates/runtime` (`vnengine_runtime`, ~2.352 líneas de código).

El alcance de la auditoría comprendió:
1. **Cálculos matemáticos del viewport y transformaciones geométricas**: dimensiones nulas (`stage_size = (0.0, 0.0)`), dimensiones negativas, overflow de zoom a infinito/NaN, cotas de clipping en Visual Composer y transformaciones locales a pantalla.
2. **Comportamiento del editor de nodos y algoritmos de layout**: comportamiento con grafos vacíos, grafos unitarios, grafos cíclicos y análisis de complejidad O(N² × 48) con cientos y miles de nodos.
3. **SoftwareRenderer y Render Backends**: buffers de tamaño cero, desbordamiento entero en buffers de alta resolución o multi-monitor (`blend_pixel`), y análisis del backend WGPU.
4. **Binario headless `vn_player`**: soporte del flag `--smoke`, `--smoke-report`, validación de argumentos CLI, mensajes de error y subsistema de Windows.
5. **Arquitectura y diseño**: análisis de componentes sobredimensionados (*god objects* como `EditorWorkbench` con más de 70 campos y 20 módulos acoplados), triplicación de cachés de assets/imágenes y redundancia con `crates/core`.

Se implementaron **12 pruebas adversarias automatizadas** en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs), confirmando **5 Bugs** (1 Crítico, 2 Altos, 1 Medio, 1 Bajo) y **4 Sospechas / Riesgos Latentes**.

---

## 2. Bugs CONFIRMADOS

### BUG-GUI-01: División por Cero y Coordenadas `NaN` en `fit_stage_rect` y `stage_scale` ante `stage_size` Nulo
- **Severidad**: Alta
- **Archivo:Línea**: [`crates/gui/src/editor/visual_composer_preview.rs:84-107`](file:///D:/Rust/audit/rustnovel/crates/gui/src/editor/visual_composer_preview.rs#L84-L107)
- **Descripción**:  
  La función pública `fit_stage_rect` calcula la relación de aspecto del stage mediante:
  ```rust
  let stage_aspect = stage_w / stage_h;
  ```
  Si `stage_size` es `(0.0, 0.0)` o tiene altura 0, `0.0 / 0.0` resulta en `NaN`.  
  Posteriormente, al comparar `available_aspect > stage_aspect`, cualquier comparación contra `NaN` en Rust evalúa a `false`, tomando la rama `else`:
  ```rust
  egui::vec2(available.width(), available.width() / stage_aspect)
  ```
  Dividir `available.width() / NaN` produce `NaN`. Luego, `egui::Rect::from_center_size(available.center(), size)` construye un rectángulo cuyas coordenadas `min.y` y `max.y` son `NaN`.  
  Asimismo, `stage_scale` calcula `stage_rect.width() / stage_w`, produciendo `f32::INFINITY`.  
  Al construir `stage_geometry`, este estado corrupto (`scale = inf`, `stage_rect` con `NaN`) se propaga a `entity_rect_with_background_fit`, generando geometrías no finitas que rompen el pipeline de dibujo de egui.
- **Test que lo reproduce**:  
  `test_fit_stage_rect_zero_dimensions_causes_nan`, `test_stage_scale_zero_stage_size_causes_infinity` y `test_stage_geometry_zero_stage_propagates_nan_and_inf` en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs).  
  *Salida de ejecución*:
  ```
  test test_fit_stage_rect_zero_dimensions_causes_nan ... ok
  test test_stage_scale_zero_stage_size_causes_infinity ... ok
  test test_stage_geometry_zero_stage_propagates_nan_and_inf ... ok
  ```
- **Fix sugerido**:  
  Validar finitez y positividad antes de operar, similar a como ya se hace en `aspect_height_for_width`:
  ```rust
  pub fn fit_stage_rect(bounds: egui::Rect, stage_size: (f32, f32), fit: StageFit) -> egui::Rect {
      let (stage_w, stage_h) = stage_size;
      let available = bounds.shrink2(egui::vec2(6.0, 6.0));
      if available.width() <= 1.0 || available.height() <= 1.0 {
          return available;
      }
      if !stage_w.is_finite() || !stage_h.is_finite() || stage_w <= 0.0 || stage_h <= 0.0 {
          return available;
      }
      // ...
  }
  ```

---

### BUG-GUI-02: Generación de Rectángulos Invertidos con Dimensiones Negativas en `fit_stage_rect`
- **Severidad**: Media
- **Archivo:Línea**: [`crates/gui/src/editor/visual_composer_preview.rs:91-100`](file:///D:/Rust/audit/rustnovel/crates/gui/src/editor/visual_composer_preview.rs#L91-L100)
- **Descripción**:  
  Si `stage_size` contiene componentes negativos (e.g. `(-1280.0, 720.0)`), `stage_aspect` es negativo (`-1.777`). La condición `available_aspect > stage_aspect` se cumple (pues un aspect ratio disponible positivo siempre es mayor que un número negativo), asignando:
  ```rust
  egui::vec2(available.height() * stage_aspect, available.height())
  ```
  El ancho resultante es negativo. Al llamar a `egui::Rect::from_center_size(center, size)`, egui genera un rectángulo invertido donde `min.x > max.x` (`!rect.is_positive()`). En egui, los rectángulos no positivos generan cálculos de layout erróneos y fallos de interacción con el puntero.
- **Test que lo reproduce**:  
  `test_fit_stage_rect_negative_dimensions_creates_inverted_rect` en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs).  
  *Salida*:
  ```
  test test_fit_stage_rect_negative_dimensions_creates_inverted_rect ... ok
  ```
- **Fix sugerido**:  
  Normalizar las dimensiones con valor absoluto o retornar fallback si son negativas:
  ```rust
  let stage_w = stage_w.abs().max(1.0);
  let stage_h = stage_h.abs().max(1.0);
  ```

---

### BUG-GUI-03: Desbordamiento a Infinito y Envenenamiento `NaN` en `zoom_to_fit_viewport` de `NodeGraph`
- **Severidad**: Alta
- **Archivo:Línea**: [`crates/gui/src/editor/node_graph/view.rs:112-126`](file:///D:/Rust/audit/rustnovel/crates/gui/src/editor/node_graph/view.rs#L112-L126)
- **Descripción**:  
  En `NodeGraph::zoom_to_fit_viewport`:
  ```rust
  let center_x = (min_x + max_x) / 2.0;
  let center_y = (min_y + max_y) / 2.0;
  // ...
  self.pan = egui::vec2(
      viewport_width / (2.0 * new_zoom) - center_x,
      viewport_height / (2.0 * new_zoom) - center_y,
  );
  ```
  Si los nodos poseen coordenadas infinitas (`f32::INFINITY` y `-f32::INFINITY`), o si las coordenadas de los nodos se desbordan durante arrastres o imports no validados:
  1. `min_x` es `-inf` y `max_x` es `+inf`.
  2. `center_x = (-inf + inf) / 2.0 = f32::NAN`.
  3. `self.pan.x` se convierte en `NaN`.
  4. Una vez que `self.pan` es `NaN`, todas las transformaciones posteriores `graph_to_screen` y `screen_to_graph` devuelven `Pos2::new(NaN, NaN)`. El viewport del editor de nodos queda completamente corrupto (pantalla gris/vacía) y no puede recuperarse sin reiniciar.
- **Test que lo reproduce**:  
  `test_node_graph_zoom_to_fit_overflow_to_infinity_and_nan` en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs).  
  *Salida*:
  ```
  Added nodes: id1=0, id2=1, count=2
  Node 0: pos=[inf 0.0]
  Node 1: pos=[-inf 0.0]
  Pan result: [NaN 1165.0]
  test test_node_graph_zoom_to_fit_overflow_to_infinity_and_nan ... ok
  ```
- **Fix sugerido**:  
  Filtrar nodos con coordenadas no finitas antes de calcular los extremos y verificar que `center_x` y `center_y` sean finitos:
  ```rust
  for (_, node, pos) in self.nodes() {
      if !pos.x.is_finite() || !pos.y.is_finite() { continue; }
      min_x = min_x.min(pos.x);
      min_y = min_y.min(pos.y);
      max_x = max_x.max(pos.x + node_visual_width(&node));
      max_y = max_y.max(pos.y + node_visual_height(&node));
  }
  if !min_x.is_finite() || !max_x.is_finite() {
      self.reset_view();
      return;
  }
  ```

---

### BUG-RT-01: Panic por Desbordamiento Entero `u32` en `BuiltinSoftwareDrawer::blend_pixel` con Buffers Grandes
- **Severidad**: Crítica
- **Archivo:Línea**: [`crates/runtime/src/render/software.rs:440-442`](file:///D:/Rust/audit/rustnovel/crates/runtime/src/render/software.rs#L440-L442)
- **Descripción**:  
  En `crates/runtime/src/render/software.rs`:
  ```rust
  fn blend_pixel(frame: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
      let idx = ((y * width + x) * 4) as usize;
  ```
  Las variables `y`, `width` y `x` son de tipo `u32`. La expresión `((y * width + x) * 4)` se evalúa completamente en aritmética de enteros de 32 bits antes del cast a `usize`.  
  Si `(y * width + x) * 4 >= 2^32` (por ejemplo, con `width = 50.000` y `y = 30.000`, `30.000 * 50.000 * 4 = 6.000.000.000 > u32::MAX`):
  1. En compilaciones de depuración (`debug_assertions`), el runtime aborta con **PANIC**: `attempt to multiply with overflow`.
  2. En compilaciones de producción (`release`), la multiplicación sufre un **wrap-around**. Como `idx` se envuelve a un valor pequeño, la guarda `if idx + 4 > frame.len()` no se activa y se sobrescribe memoria errónea al inicio del buffer, produciendo corrupción visual silenciosa.
- **Test que lo reproduce**:  
  `test_software_renderer_large_buffer_integer_overflow_panic` en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs) (anotado con `#[should_panic(expected = "attempt to multiply with overflow")]`).  
  *Salida*:
  ```
  test test_software_renderer_large_buffer_integer_overflow_panic - should panic ... ok
  ```
- **Fix sugerido**:  
  Promover los operandos a `usize` antes de multiplicar:
  ```rust
  let idx = (y as usize * width as usize + x as usize) * 4;
  ```

---

### BUG-GUI-04: Flags Críticas `--smoke` y `--smoke-report` Indocumentadas en `--help` de `vn_player`
- **Severidad**: Baja
- **Archivo:Línea**: [`crates/gui/src/bin/vn_player/launch.rs:107-111`](file:///D:/Rust/audit/rustnovel/crates/gui/src/bin/vn_player/launch.rs#L107-L111)
- **Descripción**:  
  El binario `vn_player` implementa un flujo completo de verificación de smoke headless (`run_smoke`), soportando los argumentos `--smoke`, `--smoke-report <FILE>`, `--untrusted` y `--trusted`. Sin embargo, la función `print_help()` sólo imprime:
  ```
  Usage: vn_player [script.json] [--assets-root DIR] [--manifest FILE] [--menu-config FILE] [--preferences-path FILE] [--require-manifest] [--title TITLE]
  ```
  Las opciones de smoke y seguridad están completamente ausentes. Adicionalmente, si el usuario ejecuta `vn_player --smoke-report reporte.json` sin `--smoke`, el argumento se ignora silenciosamente y se inicia la interfaz gráfica interactiva, en vez de lanzar el modo smoke o alertar del error.
- **Test que lo reproduce**:  
  `test_vn_player_cli_flags_and_help` en [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs).  
  *Salida*:
  ```
  test test_vn_player_cli_flags_and_help ... ok
  ```
- **Fix sugerido**:  
  Actualizar `print_help()` para incluir las opciones ausentes y hacer que `--smoke-report` active automáticamente el flag `smoke = true`.

---

## 3. Sospechas / Riesgos Latentes

### SOSPECHA-01: Interrupción Fatal del Runtime por Botones con Acciones `"back"` o `"menu"`
- **Archivo:Línea**: [`crates/runtime/src/lib.rs:209-220`](file:///D:/Rust/audit/rustnovel/crates/runtime/src/lib.rs#L209-L220)
- **Evidencia**:  
  En `crates/runtime/src/input.rs:122-123`, los botones de la escena con acción `"back"` o `"menu"` son reconocidos y convertidos a `InputAction::Back` y `InputAction::Menu`.  
  Sin embargo, en `RuntimeApp::handle_action`:
  ```rust
  InputAction::Back => {
      return Err(visual_novel_engine::VnError::InvalidScript(
          "input action 'back' is recognized but not implemented in runtime mode".to_string(),
      ));
  }
  InputAction::Menu => {
      return Err(visual_novel_engine::VnError::InvalidScript(
          "input action 'menu' is recognized but not implemented in runtime mode".to_string(),
      ));
  }
  ```
  En el bucle de eventos (`lib.rs:536`), cualquier error retornado por `handle_action` invoca `elwt.exit()`, cerrando el juego inmediatamente. Si un autor incluye un botón "Atrás" o "Menú" en su escena interactiva, hacer clic en él cierra abruptamente la aplicación en lugar de mostrar un menú o no hacer nada.

---

### SOSPECHA-02: Algoritmo de Desolapamiento O(N² × 48) Bloquea el Hilo Principal de la GUI
- **Archivo:Línea**: [`crates/gui/src/editor/node_graph/layout.rs:345-391`](file:///D:/Rust/audit/rustnovel/crates/gui/src/editor/node_graph/layout.rs#L345-L391)
- **Evidencia**:  
  La función `resolve_layout_overlaps` realiza hasta `AUTO_LAYOUT_OVERLAP_MAX_PASSES = 48` pasadas:
  ```rust
  for _ in 0..AUTO_LAYOUT_OVERLAP_MAX_PASSES {
      let mut pass_changed = false;
      let nodes = self.nodes().collect::<Vec<_>>();
      let len = nodes.len();
      for i in 0..len {
          for j in (i + 1)..len {
              // ...
              self.set_node_pos(*id_a, ...);
              self.set_node_pos(*id_b, ...);
          }
      }
  }
  ```
  Para un grafo con 1.000 nodos, `len * (len - 1) / 2 = 499.500` comparaciones por pasada, totalizando hasta ~24 millones de operaciones con mutaciones continuas de mapas hash. Para 5.000 nodos, supera los 600 millones de operaciones. Dado que `auto_layout_hierarchical` se ejecuta de forma síncrona dentro del método `ui(&mut self)` de egui, esto congela la ventana del editor durante decenas de segundos sin posibilidad de cancelación.

---

### SOSPECHA-03: Falsa Aceleración por Hardware en `WgpuBackend` (Software Blitter)
- **Archivo:Línea**: [`crates/runtime/src/render/hardware.rs:60-61, 132-133, 177-180`](file:///D:/Rust/audit/rustnovel/crates/runtime/src/render/hardware.rs#L60-L61)
- **Evidencia**:  
  A pesar del nombre `WgpuBackend` y de inicializar pipelines y shaders wgpu, el renderizado no se ejecuta en la GPU. `WgpuBackend` contiene internamente una instancia de `BuiltinSoftwareDrawer` y un buffer CPU `cpu_frame: Vec<u8>`.  
  En cada frame:
  ```rust
  self.drawer.draw(&mut self.cpu_frame, (width, height), render_frame);
  self.queue.write_texture(...);
  ```
  La CPU rasteriza cada píxel en RAM y luego transfiere la textura completa por el bus PCIe a la GPU. En una resolución 4K (3840x2160), esto implica transferir ~33 MB por frame (2 GB/s de ancho de banda a 60 FPS) enteramente procesados por software en CPU, degradando el rendimiento en lugar de acelerarlo.

---

### SOSPECHA-04: El Flag `windows_subsystem = "windows"` Oculta Errores en Ejecuciones Headless/CI en Windows
- **Archivo:Línea**: [`crates/gui/src/bin/vn_player.rs:1`](file:///D:/Rust/audit/rustnovel/crates/gui/src/bin/vn_player.rs#L1)
- **Evidencia**:  
  `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`  
  En binarios release en Windows, el subsistema GUI desconecta `stdout` y `stderr` por defecto. Si un script de integración continua o usuario ejecuta `vn_player --smoke ...` o se produce un fallo al cargar un script, las llamadas a `eprintln!` se descartan en el vacío y el proceso finaliza con código 1 sin diagnóstico visible en la consola.

---

## 4. Análisis Arquitectónico y Simplificaciones

### 4.1 Diagnóstico de "God Objects"

#### `EditorWorkbench` (`crates/gui/src/editor/workbench.rs:50-153`)
- **Tamaño**: Más de 70 campos en una única estructura, con lógica repartida en más de 20 archivos fuente dentro de `crates/gui/src/editor/workbench/*.rs`.
- **Problema**: Centraliza la totalidad del estado de la aplicación: configuración de proyecto, sesión de authoring, undo/redo, snapshots de script, 16 flags booleanos de visibilidad de paneles, caché de texturas egui, fallos de carga de imágenes, servicio de recursos, timeline de animación, motor de ejecución `Engine`, backend de audio, diagnósticos de linter, oráculos de reproducción (`ReproCase`), caché de compilación, catálogo de localización y docking de layout.
- **Consecuencias**: Viola gravemente el principio de responsabilidad única (*Single Responsibility Principle*). Cualquier cambio menor en un subsistema (por ejemplo, alternar un flag de visibilidad de un panel o actualizar un log de operaciones) fuerza el re-borrow de la estructura entera, impidiendo la concurrencia y haciendo el código extremadamente frágil.

#### Propuesta Concreta de Refactorización para `EditorWorkbench`:
Dividir `EditorWorkbench` en 4 componentes cohesivos e independientes:
1. `ProjectContext`: Raíz del proyecto, manifest, `VnConfig`, rutas y snapshots guardados.
2. `DocumentSession`: `NodeGraph`, `AuthoringDocumentSession` y el bus de comandos.
3. `WorkspaceLayoutManager`: Flags de visibilidad (`show_*`), estados de docking (`WorkspaceLayout`), split ratios y preferencias de layout.
4. `EditorRuntimeService`: Instancia de `Engine` para el modo player, playback de audio y cache de texturas egui.

---

### 4.2 Duplicación de Código entre Crates

1. **Triplicación de Cachés de Recursos**:
   - `crates/assets` implementa `AssetStore` con hashes y validación de seguridad.
   - `crates/gui/src/assets.rs` implementa `AssetManager` con un caché de texturas egui y presupuesto LRU.
   - `crates/gui/src/editor/image_asset_cache.rs` implementa claves de caché personalizadas para miniaturas y stage basadas en mtime/tamaño (`scene_stage_cache_key`, `thumbnail_cache_key`).
   - `crates/gui/src/editor/resource_service.rs` contiene otro servicio adicional para el editor.  
   *Propuesta*: Unificar en un único servicio `EditorAssetService` que gestione tanto bytes en crudo como handles de texturas egui con invalidación reactiva por mtime.

2. **Duplicación de `SceneFramePresenter`**:
   - `crates/gui/src/editor/scene_frame_presenter.rs` (`EguiSceneFramePresenter`)
   - `crates/runtime/src/render/scene_frame_presenter.rs` (`RuntimeSceneFramePresenter`)  
   Ambos implementan el mismo trait de `crates/core`, duplicando la validación del tema UI y el layout por defecto.

3. **Inutilidad del `UndoStack` basada en Clones Completos**:
   - `UndoStack` (`crates/gui/src/editor/undo.rs`) almacena hasta 50 clones completos de `NodeGraph`. Con grafos medianos o grandes, esto consume cientos de megabytes en RAM y produce tirones de recolección en cada edición.
   - *Paradoja*: `crates/core/src/authoring/command_bus.rs` ya provee `AuthoringDelta` y `revert_last()`, diseñado precisamente para deshacer operaciones mediante deltas atómicas sin clonar el grafo completo.

---

## 5. Mejoras Recomendadas

1. **Aritmética Segura en Renderizadores de Software**: Reemplazar todas las multiplicaciones de índices de píxel `((y * width + x) * 4)` por aritmética en `usize` con métodos de saturación o chequeos de límites previos.
2. **Desolapamiento Espacial Cuadriculado (Spatial Hashing)**: En `resolve_layout_overlaps`, sustituir el bucle O(N²) por una rejilla de celdas espaciales (*spatial grid/hash*), reduciendo la complejidad a O(N) para layouts interactivos fluidos con miles de nodos.
3. **Desacoplar Binario Headless de Windows Subsystem**: En `crates/gui/src/bin/vn_player.rs`, detectar si se ejecutan flags de consola como `--smoke` o `--help` y adjuntar la consola mediante `AttachConsole(ATTACH_PARENT_PROCESS)` en Windows para garantizar visibilidad en terminales y CI.
4. **Implementar Acciones `"back"` y `"menu"` en Runtime**: En `crates/runtime/src/lib.rs`, en lugar de retornar error fatal, emitir un evento o proveer un callback configurable por el host de la aplicación.
5. **Warnings de Clippy Detectados**:
   - Corregir implementaciones manuales de `.clamp()` en `crates/gui/src/editor/visual_composer/layers.rs:98`, `visual_composer.rs:180, 218` y `sizing.rs:22, 50`.
   - Eliminar el `.clone()` redundante en `crates/gui/src/editor/workbench/app_ui/export_wizard.rs:183`.
   - Especificar tipos explícitos `f32` (e.g. `1.0_f32`) en las llamadas a `egui::Stroke::new` para evitar advertencias de compatibilidad futura de rustc.

---

## 6. Tests Añadidos y Cómo Ejecutarlos

Se agregó la suite de pruebas de integración adversaria en:
- **Ruta**: [`crates/gui/tests/audit_gui_runtime_tests.rs`](file:///D:/Rust/audit/rustnovel/crates/gui/tests/audit_gui_runtime_tests.rs)

### Pruebas Incluidas:
1. `test_fit_stage_rect_zero_dimensions_causes_nan`: Demuestra la generación de coordenadas `NaN` ante `stage_size = (0.0, 0.0)`.
2. `test_stage_scale_zero_stage_size_causes_infinity`: Demuestra la generación de escala infinita ante stage de tamaño cero.
3. `test_stage_geometry_zero_stage_propagates_nan_and_inf`: Demuestra la propagación de cotas no finitas hacia el cálculo de rectángulos de entidades en Visual Composer.
4. `test_fit_stage_rect_negative_dimensions_creates_inverted_rect`: Demuestra la inversión de rectángulos ante dimensiones negativas.
5. `test_player_stage_viewport_zero_or_negative_returns_safe_size`: Verifica las guardas defensivas en `player_stage_viewport_size`.
6. `test_node_graph_zoom_to_fit_overflow_to_infinity_and_nan`: Demuestra el envenenamiento de `pan` a `NaN` en el grafo ante posiciones infinitas.
7. `test_node_graph_empty_and_single_node_layout`: Verifica robustez con grafos vacíos y unitarios.
8. `test_node_graph_cycles_do_not_infinite_loop`: Verifica que grafos con ciclos no causen recursión o bucles infinitos en el layout.
9. `test_node_graph_thousand_nodes_layout_performance`: Evalúa la ejecución de layout con cientos de nodos.
10. `test_software_renderer_large_buffer_integer_overflow_panic`: Fuerza y verifica el pánico por desbordamiento de entero `u32` en el software drawer.
11. `test_software_renderer_zero_size_buffer`: Verifica el comportamiento con framebuffers de tamaño 0x0.
12. `test_vn_player_cli_flags_and_help`: Verifica que `--smoke` está indocumentado en `--help` y valida el manejo de argumentos desconocidos.

### Instrucciones de Ejecución:
En PowerShell con el toolchain configurado:
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_gui --test audit_gui_runtime_tests
```
Resultado obtenido:
```
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```
