# Auditoría técnica extendida y plan de corrección — RustNovel / Visual Novel Engine

Fecha de revisión: 2026-05-28  
Entrada revisada: `rustnovel-main (9).zip`  
Alcance: `core`, `gui/editor/player`, `runtime`, `py api`, `cli`, exportación, cache/performance, seguridad y tests.

> Limitación práctica: en este entorno no están disponibles `cargo`/`rustc`, por lo que no pude compilar el workspace. También intenté ejecutar la suite Python, pero `pytest` falló antes de correr pruebas porque el módulo nativo `visual_novel_engine` no estaba construido/importable. Esta auditoría se basa en inspección estática profunda del código y de la estructura del proyecto. la auditoria busca fomentar al programador a encontrar nuevos problemas y bugs que salgan cuando se corrigan estos problemas o se añadan estas nuevas cosas. por lo tanto es normal que si se implementan estas correcciones surjan nuevos problemas

---

## 1. Qué se busca vs. qué hay hoy

### Lo que se busca

El objetivo no es simplificar el motor ni borrar capacidades. Lo correcto es extenderlo y generalizarlo para que pueda crecer hacia un motor profesional de novela visual:

- **Core estable y único**: contratos iguales entre Rust, Python, GUI y CLI.
- **GUI generalizable**: tema, tipografía, layout, DPI/PPI/TPI, resolución, modo ventana/fullscreen, estilos de botones, caja de diálogo, choices, overlays y menús no deben estar acoplados a un único look.
- **Render/export profesional**: un formato de `SceneFrame` o `RenderCommand` que pueda ser consumido por editor, player nativo, runtime exportado, benchmarks y API.
- **Route Tree nativo**: árbol de rutas, avance, elecciones, ramas, finales, nodos visitados/no visitados, estado actual y previews tanto en Visual Composer como en Play Mode y juego exportado.
- **Exportación nativa coherente**: UI, API y CLI deben exponer el mismo `ExportSpec`, con plan/dry-run, bundle, ejecutable, firma/integridad, reporte JSON y fallos atómicos.
- **Tests contra falsos positivos**: no solo probar casos felices. Deben existir tests que fallen cuando se ocultan errores, cuando un stub Python está desfasado, cuando CLI devuelve éxito ante fallos, o cuando GUI no responde a escalas/resoluciones extremas.

### Lo que hay hoy

El repo ya tiene muchas piezas útiles:

- `crates/core`: modelo de script, authoring, validación, bundle/export, seguridad, repro, graph.
- `crates/gui`: editor egui, visual composer, player mode, asset browser, inspector, menu player.
- `crates/runtime`: runtime nativo con audio y backends de render.
- `crates/py`: bindings PyO3.
- `python/vnengine`: capa Python pura/wrapper.
- `tools/cli`: CLI para validate, compile, trace, authoring, package, import Ren'Py.
- `tests`: tests Rust y Python.

El problema principal es que esas piezas no están unificadas bajo contratos comunes. El core está más avanzado que GUI/API/CLI, pero incluso en core hay huecos de contrato, exportación, route tree, render frame, cache, prefetch e integridad.

---

## 2. Hallazgos P0/P1 prioritarios

| Prioridad | Área | Hallazgo | Impacto |
|---|---|---|---|
| P0 | Core/Python | Schema incompatible entre Rust y Python | Scripts aceptados por Python pueden fallar en core/export/GUI. |
| P0 | CLI | `trace` ignora errores con `let _ = ...` | Puede producir trazas falsas y exit code exitoso ante fallos reales. |
| P0 | CLI/Authoring | Loader de authoring oculta errores de parseo | Un documento authoring corrupto puede tratarse como script legacy. |
| P0 | Export | `export_executable_bundle` puede dejar bundle parcial antes de fallar | Mala UX, builds contaminados, CI engañoso. |
| P0 | Python API | `.pyi` está desfasado respecto al módulo nativo | Tipado falso, autocompletado roto, tests pueden no cubrir API real. |
| P0 | GUI | UI llama “Export Game” a lo que solo exporta script compilado | Confusión fuerte: no es exportación de juego nativo. |
| P1 | GUI | Estilos/tamaños hardcodeados, sin design system | Bloquea temas, accesibilidad, localización, DPI/resolución independiente. |
| P1 | Core/Render | `UiState`/`TextRenderer` no son frame renderer-agnóstico | No escala a motor profesional con overlays, choices, route tree y skins. |
| P1 | Core | Falta `RouteTree` nativo | Solo existe historial lineal de choices; no árbol de rutas jugable/visible. |
| P1 | API/CLI/GUI | ExportSpec no tiene paridad | CLI tiene más opciones que Python/GUI; GUI no tiene wizard/plan real. |
| P1 | Tests/CI | CI usa `unittest`, pero reglas estrictas están en `pytest` | Puede ocultar skips/falsos positivos de Python. |

---

## 3. Core: errores y huecos añadidos

### C1. Contrato de schema incompatible entre Rust y Python

**Evidencia:**

- Rust acepta solo `SCRIPT_SCHEMA_VERSION` exacto: `crates/core/src/script/raw.rs:303-305`.
- Migración Rust rechaza `script_schema_version` faltante: `crates/core/src/migration.rs:101-109`.
- Python acepta schema faltante y versiones legacy con major menor/igual: `python/vnengine/script_types.py:53-70` y `python/vnengine/script_types.py:141-151`.
- Tests Python esperan aceptar schema faltante/legacy: `tests/python/test_vnengine.py:39-47`.

**Problema:** el mismo JSON puede ser válido en Python y fallar en Rust. Esto es un falso positivo de API: el usuario cree que su script es válido porque Python lo aceptó, pero luego falla al compilar/exportar.

**Corrección:**

- Definir `SchemaPolicy` única en core: `StrictCurrent`, `LegacyReadOnly`, `Migrating`.
- Exponer esa política en Python, CLI y GUI.
- Añadir comando explícito `migrate-script` o `vnengine authoring migrate`.
- Tests golden con la misma matriz en Rust y Python:
  - schema actual válido;
  - schema faltante;
  - `0.9` legacy;
  - `2.0` futuro;
  - schema no string;
  - eventos mal formados.

---

### C2. `ScriptRaw::from_json_with_limits` duplica memoria al parsear scripts grandes

**Evidencia:** `crates/core/src/script/raw.rs:64-86` parsea a `serde_json::Value`, luego reserializa/convierte a envelope y en rutas de error genera ventanas/strings para diagnóstico.

**Problema:** aunque existe límite de bytes, el flujo hace múltiples representaciones del mismo input. En scripts grandes o en exportación batch aumenta memoria pico.

**Corrección:**

- Mantener límite de bytes antes de parsear.
- Evitar reserialización innecesaria cuando el error ya trae posición.
- Añadir benchmark de memoria para scripts con 10k, 50k y 100k eventos.
- Separar modo `diagnostic_pretty_errors` de modo `fast_parse` para CLI/export.

---

### C3. Falta modelo nativo de `RouteTree`

**Evidencia:**

- `StoryGraph` tiene nodos, edges, start y labels, pero no progreso ni decisiones: `crates/core/src/graph.rs:118-129`.
- Solo hay navegación básica por edges: `crates/core/src/graph.rs:262-275`.
- Runtime solo guarda `ChoiceHistoryEntry` lineal: `crates/core/src/engine/runtime.rs:16-24`.
- El menú de rutas del player solo muestra `choice_history()`: `crates/gui/src/app/eframe_impl.rs:263-277`.
- `set_state` limpia `choice_history` y marcas de lectura: `crates/core/src/engine/runtime.rs:320-332`.

**Problema:** no hay forma nativa de ver árbol de rutas, estado actual, ramas disponibles, ramas visitadas, choices tomadas, endings alcanzados o progreso. Esto limita Visual Composer, Play Mode, player exportado y API.

**Corrección propuesta:** añadir al core:

```rust
pub struct RouteTree {
    pub root: RouteNodeId,
    pub current: Option<RouteNodeId>,
    pub nodes: Vec<RouteNode>,
    pub edges: Vec<RouteEdge>,
    pub coverage: RouteCoverage,
}

pub struct RouteEdge {
    pub from: RouteNodeId,
    pub to: RouteNodeId,
    pub kind: RouteEdgeKind, // Sequential, Choice, Jump, Conditional, Ending
    pub label: Option<String>,
    pub condition: Option<RouteCondition>,
    pub selected: bool,
    pub discovered: bool,
}

pub struct RouteProgress {
    pub current_ip: u32,
    pub visited_ips: BTreeSet<u32>,
    pub selected_choices: Vec<ChoiceHistoryEntry>,
    pub reached_endings: BTreeSet<String>,
}
```

Debe derivarse desde `ScriptCompiled`/`StoryGraph`, actualizarse desde `Engine`, persistirse en saves si se quiere continuidad, y exponerse a GUI/API/CLI.

---

### C4. `UiState` y `RenderBackend` son demasiado pobres para un motor visual profesional

**Evidencia:**

- `UiState` solo expone `Dialogue`, `Choice`, `Scene`, `System` con strings: `crates/core/src/ui.rs:6-37`.
- `UiState::from_event` resume escenas como texto, no como frame visual: `crates/core/src/ui.rs:55-67`.
- `RenderBackend` devuelve `RenderOutput { text }`: `crates/core/src/render.rs:8-17`.
- `TextRenderer` formatea eventos como strings: `crates/core/src/render.rs:57-95`.

**Problema:** el player, editor, exportador y benchmarks necesitan una representación común de lo que se debe renderizar: fondos, personajes, capas, transiciones, caja de texto, choices, overlays, route tree, estilos y acciones. Hoy cada UI está obligada a reconstruir lógica a mano.

**Corrección:** introducir un contrato renderer-agnóstico:

```rust
pub struct SceneFrame {
    pub stage: StageSpec,
    pub layers: Vec<RenderLayer>,
    pub overlays: Vec<UiOverlay>,
    pub interactions: Vec<InteractionSpec>,
    pub route: Option<RouteTreeSnapshot>,
    pub theme_id: Option<String>,
}

pub enum RenderCommand {
    Clear(ColorToken),
    Image { asset: AssetId, rect: LayoutRect, fit: ImageFit, z: i32 },
    Text { text: SharedStr, style: TextStyleId, rect: LayoutRect },
    Panel { style: PanelStyleId, rect: LayoutRect },
    Button { id: ActionId, label: SharedStr, style: ButtonStyleId, rect: LayoutRect },
}
```

GUI, runtime y API deberían consumir ese `SceneFrame`, no strings.

---

### C5. `VisualState::set_character_position` silencia ambigüedad

**Evidencia:** si hay más de un personaje con el mismo nombre, `set_character_position` hace `return` sin error: `crates/core/src/visual.rs:85-90`.

**Problema:** una actualización de posición puede no aplicarse y el motor no reporta nada. En escenas complejas con clones/expresiones/poses, esto causa bugs visuales difíciles de depurar.

**Corrección:** devolver `VnResult<()>` o registrar diagnóstico de runtime; usar identificadores de instancia (`character_instance_id`) además de `name`; mantener compatibilidad con fallback por nombre solo si es único.

---

### C6. Prefetch lineal: no entiende jumps/choices/condiciones

**Evidencia:** `peek_next_asset_paths` recorre únicamente `events[start..end]`: `crates/core/src/engine/prefetch.rs:12-20`.

**Problema:** prefetch puede cargar recursos que no se usarán tras un `jump` y omitir recursos que sí se usarán en ramas de choice o condicionales. En export/runtime real puede afectar memoria y tiempos de carga.

**Corrección:** usar `StoryGraph`/`RouteTree` para prefetch con presupuesto:

- `PrefetchMode::Linear` para debug rápido.
- `PrefetchMode::LikelyPath` para la ruta actual.
- `PrefetchMode::BranchAware { depth, max_assets, max_bytes }`.
- Métricas: hits, misses, bytes, evictions, tiempo de decode.

---

### C7. Exportación puede dejar estado parcial y su integridad es incompleta

**Evidencia:**

- `export_bundle` construye plan y lo descarta: `crates/core/src/bundle.rs:38-40`.
- `export_executable_bundle` llama primero a `export_bundle` y después valida si hubo ejecutable: `crates/core/src/bundle.rs:258-276`.
- El HMAC cubre compiled bytes, assets manifest y project manifest, pero no launcher/runtime/report/layout completo: `crates/core/src/bundle.rs:190-209`.
- Assets se copian y luego se leen completos para hash: `crates/core/src/bundle/assets.rs:34-49`.

**Problema:** si `require_executable` falla, ya pudo escribirse un bundle incompleto. Además, la firma no cubre todos los artefactos relevantes del paquete. Para exportación profesional, el proceso debe ser planificado, verificable y atómico.

**Corrección:**

- `build_export_plan` debe ser fuente de verdad y sus `errors` deben impedir materialización.
- Materializar en directorio temporal y hacer rename atómico al final.
- Validar `require_executable` antes de escribir.
- Firmar un manifiesto Merkle que incluya scripts, assets, runtime, launcher, package report y layout version.
- Hash streaming, no `fs::read` completo para assets grandes.

---

### C8. Plan de exportación no representa bien Linux/macOS

**Evidencia:** en `build_export_plan`, `executable` solo se setea para Windows `.exe`: `crates/core/src/bundle/plan.rs:75-82`, pero `export_executable_bundle` espera `game` para Linux/macOS: `crates/core/src/bundle.rs:261-264`.

**Problema:** el plan/reporte puede no anticipar correctamente si Linux/macOS producirán ejecutable. Esto rompe dry-run y UI wizard.

**Corrección:** mover la lógica de ejecutable por plataforma a una función única:

```rust
fn expected_executable_name(target: ExportTargetPlatform) -> &'static str;
fn classify_runtime_artifact(path: &Path, target: ExportTargetPlatform) -> RuntimeArtifactKind;
```

Usarla en plan, materialización, CLI, GUI y Python.

---

### C9. Cache de assets clona bytes en cada hit

**Evidencia:** `ByteCache::get` retorna `Option<Vec<u8>>` clonando `entry.data`: `crates/assets/src/cache.rs:28-33`. `AssetStore::load_bytes` también clona bytes al insertar y devolver: `crates/assets/src/store.rs:78-107`.

**Problema:** para imágenes/audio grandes, cada hit duplica memoria y CPU. Esto afecta editor, play mode y export preview.

**Corrección:** usar `Arc<[u8]>` o `bytes::Bytes`, devolver referencias compartidas y medir:

- memoria cache actual;
- bytes clonados evitados;
- hit/miss rate;
- evictions;
- decode cache separado de byte cache.

---

### C10. `CompilationCacheKey` depende de `DefaultHasher` y mtime/len

**Evidencia:** `CompilationCacheKey` usa `DefaultHasher`, JSON serializado y metadata de assets con `len`/`modified`: `crates/gui/src/editor/workbench/compile_cache.rs:64-120`.

**Problema:** `DefaultHasher` no es contrato estable para reportes/repros. `mtime` puede cambiar sin contenido o no cambiar con suficiente precisión. Para editor puede servir, pero no para cache profesional/reproducible.

**Corrección:** separar:

- cache interactivo rápido: mtime/len;
- cache reproducible/export: hash semántico estable SHA-256/BLAKE3 del graph normalizado + manifest de assets.

---

## 4. GUI/editor/player: foco extendido

### G1. El GUI está cerrado a un estilo visual concreto

**Evidencia:**

- `DEFAULT_STAGE_SIZE = 1280x720`: `crates/gui/src/app.rs:30`.
- Resolución default y fullscreen automático por altura `<720`: `crates/gui/src/app.rs:79-95`.
- Tamaños del player menu con constantes `24`, `240`, `180`, `132`, `96`: `crates/gui/src/app.rs:252-283`.
- Cálculo de texto por `chars * 8.0 + 28.0`: `crates/gui/src/app.rs:298-307`.
- Overlays usan colores, radios, márgenes y fuentes directos: `crates/gui/src/app.rs:551-680`.
- `player_overlay.rs` tiene heurísticas hardcodeadas de caja de diálogo/choices: `crates/gui/src/player_overlay.rs:11-92`.

**Problema:** cambiar el estilo de una novela requiere tocar código Rust. No hay tokens de diseño, temas serializables ni perfiles de layout. La UI no es realmente independiente de resolución/DPI; solo usa algunos porcentajes y clamps.

**Corrección arquitectónica:** crear un sistema de UI declarativo, serializable y compatible con egui/runtime/export:

```rust
pub struct DisplayProfile {
    pub logical_size: [f32; 2],
    pub physical_size: [u32; 2],
    pub dpi_scale: f32,
    pub user_scale: f32,
    pub safe_area: Insets,
    pub window_mode: WindowMode,
}

pub struct UiTheme {
    pub id: String,
    pub typography: TypographyTokens,
    pub spacing: SpacingTokens,
    pub colors: ColorTokens,
    pub panels: BTreeMap<String, PanelStyle>,
    pub buttons: BTreeMap<String, ButtonStyle>,
    pub text_boxes: BTreeMap<String, TextBoxStyle>,
    pub breakpoints: Vec<LayoutBreakpoint>,
}

pub enum Length {
    Px(f32), Percent(f32), Em(f32), Rem(f32), Vw(f32), Vh(f32), Min(Box<Length>, Box<Length>), Max(Box<Length>, Box<Length>), Auto
}

pub struct StageViewportPolicy {
    pub reference_size: [f32; 2],
    pub fit: StageFitPolicy, // Contain, Cover, Stretch, PixelPerfect, Custom
    pub anchor: Anchor,
    pub safe_area_mode: SafeAreaMode,
}
```

Esto permite que cada novela defina su identidad visual sin recompilar el motor.

---

### G2. `PlayerMenuConfig` no alcanza para un diseño visual completo

**Evidencia:** `PlayerMenuStyleConfig` solo cubre ancho/alto, altura de botón, radio, alpha y cuatro colores: `crates/core/src/player_menu.rs:126-150`.

**Problema:** falta tipografía, escalado, espaciado, border styles, shadow, textura, animación, responsive breakpoints, estilos por componente, variantes de botones, estados hover/active/disabled, safe areas y localización.

**Corrección:** mantener `PlayerMenuConfig` como configuración funcional, pero mover estilo a `UiTheme`/`ComponentStyle`. Ejemplo:

```rust
pub struct ComponentStyle {
    pub layout: LayoutSpec,
    pub typography: TextStyleId,
    pub background: PaintToken,
    pub border: BorderSpec,
    pub radius: RadiusSpec,
    pub padding: InsetsSpec,
    pub states: BTreeMap<ComponentState, StyleOverride>,
    pub animation: Option<AnimationSpec>,
}
```

`PlayerMenuConfig` debería referenciar IDs de estilo, no contener todos los valores visuales.

---

### G3. Visual Composer, Play Mode y juego exportado deberían compartir presenter

Hoy el player standalone dibuja stage/overlays en `app.rs`, el editor tiene `VisualComposerPanel`, `scene_stage`, `player_ui`, `visual_composer_preview` y componentes propios. Eso aumenta duplicación y drift.

**Corrección:** crear:

```rust
pub trait SceneFramePresenter {
    fn present(&mut self, frame: &SceneFrame, display: &DisplayProfile, theme: &UiTheme) -> UiResponse;
}
```

Implementaciones:

- `EguiSceneFramePresenter` para editor y player egui.
- `RuntimeSceneFramePresenter` para WGPU/software.
- `HeadlessSceneFramePresenter` para tests/benchmarks.

Esto no elimina GUI existente; lo generaliza y lo reutiliza.

---

### G4. Hay varias fuentes de verdad del documento/grafo

**Evidencia:**

- `EditorWorkbench` guarda `node_graph`, `authoring_session`, `operation_log`, `verification_runs`: `crates/gui/src/editor/workbench.rs:103-174`.
- `NodeGraph` también tiene `authoring_session`: `crates/gui/src/editor/node_graph.rs:44-89`.
- `PyNodeGraph` repite `inner`, `session`, `operation_log`, `verification_runs`: `crates/py/src/bindings/editor_node_graph.rs:20-28`.

**Problema:** los estados pueden desincronizarse. Un comando aplicado en una capa puede no reflejarse igual en otra. Esto afecta undo/redo, validación, API y visual composer.

**Corrección:** una sola fuente de verdad:

```rust
pub struct EditorDocumentStore {
    pub session: AuthoringDocumentSession,
    pub view_state: EditorViewState,
}
```

`NodeGraph`, GUI y PyNodeGraph deben ser vistas/adaptadores, no dueños independientes de sesión semántica.

---

### G5. El toolbar/menu no usa un sistema de comandos unificado

**Evidencia:** toolbar en `workbench/app_ui.rs:51-89` y menú en `menu_bar.rs:4-47` duplican acciones y nombres. Además `Exportar .vnproject` y `Package Bundle` están separados sin wizard común.

**Problema:** shortcuts, enabled/disabled, tooltips, permisos, logging, telemetry y tests se vuelven inconsistentes.

**Corrección:** definir `EditorCommand`:

```rust
pub enum EditorCommand {
    OpenProject, ImportRenpy, SaveProject,
    ExportCompiledScript, ExportGameBundle, ExportNativeExecutable,
    ValidateDryRun, CompilePreview, OpenPlayerMenuSettings,
    ExportDiagnosticReport, ImportDiagnosticReport,
}
```

Cada comando debe tener label localizable, estado habilitado, shortcut, handler, logging y test.

---

### G6. La UI de exportación está mal nombrada y poco profesional

**Evidencia:** `Export Game (.vnproject)` llama `export_compiled_project()`: `crates/gui/src/editor/menu_bar.rs:39-44`. Toolbar usa `Exportar .vnproject` y `Empaquetar Bundle`: `crates/gui/src/editor/workbench/app_ui.rs:64-68`.

**Problema:** el usuario espera un juego exportado, pero recibe script compilado. El bundle real está en otro botón. Esto causa confusión y reportes falsos.

**Corrección:**

- Renombrar a `Export Compiled Script...`.
- Crear wizard `Export Game...` con pasos:
  1. target platform;
  2. runtime artifact;
  3. entry script;
  4. integrity/signing;
  5. layout version;
  6. dry-run plan;
  7. export final.
- Mostrar `ExportPlan.errors/warnings` antes de escribir.
- Ofrecer `Copy CLI command` y `Export report JSON`.

---

### G7. Configuración visual se guarda en cada cambio de slider

**Evidencia:** `render_player_menu_settings_window` persiste si `changed`: `crates/gui/src/editor/workbench/app_ui.rs:322-330`.

**Problema:** cambios intermedios generan escrituras frecuentes, sin preview transaction, Apply/Revert ni validación previa del tema completo.

**Corrección:** usar modelo draft:

```rust
struct ThemeEditorDraft {
    original: UiTheme,
    draft: UiTheme,
    dirty: bool,
    validation: Vec<UiThemeIssue>,
}
```

Botones: `Preview`, `Apply`, `Revert`, `Save as theme`, `Export theme JSON`.

---

### G8. `EditorWorkbench::update` ignora `dt`

**Evidencia:** `update(&mut self, _dt: usize)` ignora `dt` y suma `1.0`: `crates/gui/src/editor/workbench.rs:339-347`.

**Problema:** reproducción/timeline depende de frames, no de tiempo real. En máquinas lentas/rápidas, preview se desincroniza.

**Corrección:** usar `Duration`/segundos reales y `ticks_per_second` de timeline. Test: simular 30/60/144 fps y verificar misma posición temporal.

---

### G9. Persistencia de layout oculta errores

**Evidencia:** `persist_layout_prefs_if_changed` ignora errores de `create_dir_all` y `write`: `crates/gui/src/editor/workbench.rs:402-407`.

**Problema:** el usuario cree que se guardó layout, pero puede fallar silenciosamente.

**Corrección:** devolver diagnóstico visible (`ToastKind::Error`) y log estructurado. Añadir tests de ruta sin permisos.

---

### G10. Componentes aparentemente legacy/no integrados

**Evidencia:** `GraphPanel` y `ViewportPanel` se exportan en `crates/gui/src/editor/mod.rs:47-59`, pero solo aparecen en sus archivos y exports (`rg GraphPanel|ViewportPanel`).

**Problema:** no está claro si son legacy, placeholders o API pública futura. Esto complica mantenimiento.

**Corrección:** no eliminarlos de golpe. Clasificarlos:

- si son públicos: integrarlos en `EditorCommand`/layout y agregar tests;
- si son legacy: marcarlos `#[deprecated]` una release y moverlos a `legacy`;
- si son prototipos: documentar estado y dueño.

---

### G11. Localización mezclada y labels hardcodeados

Hay labels en español e inglés en toolbar/menu/player (`Exportar`, `Package Bundle`, `Continue`, `History`, etc.). Esto impide internacionalización real.

**Corrección:** `LocalizationCatalog` ya existe en workbench, pero debe subir a una capa común para editor/player/exported game. Todos los comandos/componentes deben usar keys:

```text
command.export_game.label
player.dialogue.continue
menu.routes.empty
settings.ui_scale.label
```

---

## 5. API Python / PyO3

### A1. Stub `.pyi` desfasado

**Evidencia:** `python/visual_novel_engine.pyi` declara una API mínima; el módulo nativo expone muchos métodos que no aparecen. Ejemplos:

- `NodeGraph` nativo tiene `search_nodes`, `validation_report`, `from_authoring_or_script_json`, fragments, layer ops, preview, bookmarks, autofix, etc. (`crates/py/src/bindings/editor_node_graph.rs:168-396`). El `.pyi` solo lista una fracción (`python/visual_novel_engine.pyi:72-89`).
- `.pyi` dice `choose -> StepResult` y `resume -> StepResult`: `python/visual_novel_engine.pyi:22-24`, pero Rust retorna `choose -> PyObject` y `resume -> None`: `crates/py/src/bindings/engine.rs:89-93` y `crates/py/src/bindings/engine.rs:247-250`.
- `.pyi` no lista `export_bundle`, `run_visual_novel`, `default_player_menu_config`, `validate_player_menu_config`, aunque se registran en `crates/py/src/lib.rs:32-37`.

**Corrección:** generar `.pyi` desde PyO3 o añadir test de paridad por introspección:

```python
def test_pyi_matches_native_public_api():
    # comparar métodos esperados vs dir(visual_novel_engine.Engine/NodeGraph/...)
```

---

### A2. `run_visual_novel` existe pero siempre falla

**Evidencia:** valida script/config y luego retorna `RuntimeError`: `crates/py/src/lib.rs:42-57`.

**Problema:** el nombre promete lanzar la novela. En realidad es una extensión headless.

**Corrección:**

- O renombrar a `validate_visual_novel_launch_config`.
- O feature-gate `run_visual_novel` para builds con GUI.
- Documentar en `.pyi` y tests.

---

### A3. `export_bundle` Python no tiene paridad con CLI/core

**Evidencia:** Python solo acepta target/runtime/require executable y hardcodea `integrity=None`, `layout_version=1`, `hmac_key=None`: `crates/py/src/lib.rs:60-89`.

**Problema:** CLI puede exportar con HMAC/layout version, Python no. Esto rompe automatización profesional.

**Corrección:** exponer `ExportBundleSpec` como clase Python o aceptar dict JSON con todos los campos del core. Retornar report estructurado, no solo string.

---

### A4. Métricas placeholder/falsas

**Evidencia:**

- `get_memory_usage` retorna `current_texture_bytes = 0`: `crates/py/src/bindings/engine.rs:203-209`.
- `is_loading` siempre retorna `false`: `crates/py/src/bindings/engine.rs:227-229`.

**Problema:** API de monitoreo miente por omisión. Benchmarks y UI podrían confiar en datos falsos.

**Corrección:** si no hay backend real, retornar `None`/`NotImplementedError` o implementar métricas reales. Añadir test que impida placeholders silenciosos.

---

### A5. `visual_state` pierde campos visuales

**Evidencia:** Python expone character `name/expression/position`, pero omite `x/y/scale`: `crates/py/src/bindings/engine.rs:103-117`.

**Problema:** el API no permite reproducir fielmente la escena si se usan posiciones absolutas.

**Corrección:** exponer todos los campos del `CharacterPlacementCompiled` y versionar el payload.

---

### A6. Métodos Python ocultan errores con `let _ = ...`

**Evidencia:** `PyNodeGraph.connect`, `connect_port`, `remove_node` descartan resultado de comandos: `crates/py/src/bindings/editor_node_graph.rs:58-67` y `:124-125`.

**Problema:** llamadas inválidas pueden parecer exitosas. Esto es falso positivo de API.

**Corrección:** devolver `PyResult<()>` o `bool` con error explícito; tests para ids inexistentes, puertos inválidos y nodos removidos.

---

## 6. CLI

### L1. `trace` ignora errores de ejecución

**Evidencia histórica:** `trace_script` descartaba los resultados de `choose(0)`, `resume()` y `step()` en `tools/cli/src/bin/vnengine.rs`, ocultando fallos de avance.

**Problema:** si el motor falla, la traza puede seguir/terminar sin reportarlo y el comando puede devolver éxito.

**Corrección:** propagar errores con contexto:

```rust
engine.step().with_context(|| format!("trace step {step}"))?;
```

Añadir `--choice-policy first|random|scripted` y registrar decisiones.

---

### L2. Loader de authoring oculta documentos corruptos

**Evidencia:** `load_authoring_document` intenta `AuthoringDocument::from_json`, y ante cualquier error cae a `load_authoring_document_or_script`: `tools/cli/src/bin/vnengine/authoring.rs:401-409`.

**Problema:** un `.vnauthoring` corrupto puede interpretarse como script runtime si casualmente tiene partes compatibles o producir errores confusos.

**Corrección:** fallback solo si el JSON no es envelope authoring. Si parece authoring pero falla, reportar error original con span/contexto.

---

### L3. Manifest CLI oculta errores de recorrido y usa memoria completa

**Evidencia:** `WalkDir::new(root).into_iter().filter_map(Result::ok)` descarta errores: `tools/cli/src/bin/vnengine.rs:424-430`. Luego lee cada archivo completo para hash: `tools/cli/src/bin/vnengine.rs:442-445`.

**Problema:** permisos/links/errores de filesystem pueden ocultarse; assets grandes consumen memoria innecesaria.

**Corrección:** no usar `filter_map(Result::ok)` en tooling profesional. Hashear streaming. Añadir `--exclude`, `--max-file-bytes`, `--json-report`.

---

### L4. `validate` no genera reporte machine-readable

**Evidencia:** `validate_script` solo valida y retorna `Ok(())`: `tools/cli/src/bin/vnengine.rs:331-339`.

**Problema:** CI/editor/automatización no obtienen lista estructurada de problemas, severidades ni códigos.

**Corrección:** añadir `--format text|json|sarif` y `--warnings-as-errors`.

---

### L5. Escrituras no atómicas

**Evidencia:** `compile_script`, `trace_script`, `write_json` y package escriben directo con `fs::write`: `tools/cli/src/bin/vnengine.rs:341-349`, `:404-408`, `tools/cli/src/bin/vnengine/authoring.rs:449-455`.

**Problema:** si se corta el proceso, pueden quedar archivos corruptos.

**Corrección:** `write_temp_then_rename`, con fsync opcional para export/release.

---

### L6. Package CLI no tiene plan/dry-run/JSON output

**Evidencia:** `package_project` imprime resumen textual: `tools/cli/src/bin/vnengine/package.rs:51-72`.

**Problema:** la CLI no expone de forma cómoda el `ExportPlan`, ni permite a GUI/API reusar exactamente la salida del plan.

**Corrección:** añadir:

```text
vnengine package --plan --format json
vnengine package --dry-run
vnengine package --atomic
vnengine package --report out.json
```

---

## 7. Runtime render

### R1. WGPU backend es placeholder visual

**Evidencia:** `crates/runtime/src/render/hardware.rs` configura surface y limpia color, pero no dibuja fondos/personajes/texto/choices. Además indexa `caps.formats[0]` y `caps.alpha_modes[0]` sin validar que existan.

**Problema:** no es backend de render profesional. Puede panic si capabilities vienen vacías, y aunque arranque no representa una novela visual.

**Corrección:** que el backend consuma `SceneFrame`/`RenderCommand` y valide capabilities con error recuperable.

---

### R2. Software backend panica e ignora errores de resize

**Evidencia:** `SoftwareBackend::new()` panica si `try_new` falla: `crates/runtime/src/render/software.rs:20-28`. `resize()` descarta errores de `resize_surface`/`resize_buffer`: `crates/runtime/src/render/software.rs:42-45`.

**Problema:** runtime exportado debe degradar/reportar errores, no hacer panic o ignorarlos.

**Corrección:** `new -> Result<Self>`, `resize -> Result<()>`, y propagación hacia GUI/CLI/tests.

---

## 8. Seguridad

- `ExtCall` en Python registra error si comando no está permitido, pero no necesariamente bloquea la devolución del evento; hay que definir si un `ExtCall` denegado debe parar la ejecución, requerir `resume`, emitir evento de seguridad o fallar duro. Evidencia: `crates/py/src/bindings/engine.rs:63-80`.
- Integridad de bundle debe distinguir corrupción accidental vs seguridad. CRC32 de binario sirve para corrupción, no para autenticidad.
- Bundle HMAC debe cubrir todos los artefactos o llamarse explícitamente “partial integrity”.
- GUI/API/CLI deben mostrar claramente modo `Trusted` vs `Untrusted`, manifest requerido y capabilities.

---

## 9. Optimización memoria/CPU/cache/export

### Recomendaciones concretas

1. Cambiar `Vec<u8>` clonados por `Arc<[u8]>`/`Bytes` en cache de assets.
2. Cache en capas:
   - raw bytes;
   - decoded image/audio;
   - GPU texture;
   - compiled script;
   - scene frame/layout.
3. Presupuestos por plataforma: desktop/mobile/web/export preview.
4. Métricas obligatorias:
   - `current_bytes`, `peak_bytes`, `hit_rate`, `miss_rate`, `evictions`, `decode_ms`, `upload_ms`.
5. Exportación debe usar hashing/copy streaming.
6. Prefetch debe ser route-aware, no lineal.
7. Benchmarks:
   - script 10k/50k/100k events;
   - 1k assets pequeños vs 100 assets grandes;
   - route tree grande;
   - resize/DPI changes;
   - export con HMAC.

---

## 10. Tests faltantes para evitar falsos positivos

### Core

- Paridad schema Rust/Python.
- RouteTree: loops, choices anidadas, jumps condicionales, endings, nodos inalcanzables.
- `VisualState`: duplicate character names, absolute position, patch add/update/remove.
- Prefetch branch-aware vs lineal.
- Export atomic: al fallar no debe quedar bundle parcial.
- HMAC: detectar modificación de cualquier archivo cubierto.

### GUI

- Snapshot/layout tests para 640x360, 1280x720, 1920x1080, ultrawide, portrait, fullscreen, windowed.
- DPI/user scale: 0.75, 1.0, 1.5, 2.0, 3.0.
- Text overflow en español/inglés/japonés y tokens largos.
- Theme validation: colores inválidos, fuentes faltantes, estilos incompletos.
- Command registry: cada botón/menú ejecuta el mismo `EditorCommand`.
- Export wizard: plan con warning/error, require executable, hmac faltante.

### API Python

- `.pyi` parity test por introspección.
- Métodos que hoy ocultan errores (`connect`, `remove_node`) deben fallar o devolver error.
- `visual_state` debe incluir x/y/scale.
- `export_bundle` debe aceptar spec completa.
- `is_loading`/`memory_usage` no deben devolver placeholders falsos.

### CLI

- Exit codes: validate/trace/package/manifest fallan ante errores reales.
- `trace` no debe ignorar errores.
- `authoring` loader no debe ocultar authoring corrupto.
- `manifest` no debe ignorar errores de WalkDir.
- JSON/SARIF output estable.

### CI

**Evidencia:** CI ejecuta `python -m unittest discover`: `.github/workflows/ci.yml:179-182`, pero las reglas anti-skip están en `pytest`: `tests/python/conftest.py:15-37`.

**Corrección:** usar `pytest` en CI o replicar esas reglas en unittest. Ideal:

```yaml
python -m pytest tests/python -q
```

con healthcheck del módulo nativo.

---

## 11. Arquitectura recomendada para GUI generalizable

### 11.1 Capas propuestas

```text
Core Script/Engine
  -> SceneFrameBuilder
      -> SceneFrame + RouteTreeSnapshot + UiState
          -> ThemeResolver(DisplayProfile, UiTheme)
              -> LayoutEngine
                  -> Presenter egui / presenter wgpu / presenter headless
```

### 11.2 Archivos/módulos sugeridos

```text
crates/core/src/ui_theme.rs
crates/core/src/display.rs
crates/core/src/scene_frame.rs
crates/core/src/route_tree.rs
crates/gui/src/theme_editor.rs
crates/gui/src/scene_frame_presenter.rs
crates/gui/src/export_wizard.rs
crates/py/src/bindings/export_spec.rs
crates/py/src/bindings/theme.rs
tools/cli/src/bin/vnengine/export.rs
```

### 11.3 Principio clave

El GUI no debería preguntar “qué color/tamaño uso aquí” desde código hardcodeado. Debe preguntar:

```rust
let style = theme.resolve_component("dialogue_box", display, state);
let rect = layout.resolve(style.layout, stage_rect, display.safe_area);
presenter.panel(rect, style);
```

### 11.4 Beneficios

- Cada novela puede traer su `theme.json`.
- El editor puede previsualizar tema sin recompilar.
- El runtime exportado puede usar los mismos estilos.
- Tests headless pueden validar layout sin abrir ventana.
- Soporta resolución, DPI, ventana/fullscreen, ultrawide y mobile.

---

## 12. Exportación nativa profesional

Crear un flujo único:

```rust
pub struct ExportSpec {
    pub project_root: PathBuf,
    pub output_root: PathBuf,
    pub target: ExportTargetPlatform,
    pub entry_script: Option<PathBuf>,
    pub runtime_artifact: Option<PathBuf>,
    pub require_executable: bool,
    pub integrity: BundleIntegrity,
    pub hmac_key: Option<SecretString>,
    pub output_layout_version: u16,
    pub atomic: bool,
    pub include_debug_symbols: bool,
}
```

Funciones:

```rust
pub fn plan_export(spec: &ExportSpec) -> ExportPlan;
pub fn validate_export_plan(plan: &ExportPlan) -> Result<(), ExportError>;
pub fn execute_export(plan: &ExportPlan) -> Result<ExportReport, ExportError>;
```

Consumidores:

- GUI wizard usa `plan_export` antes de escribir.
- CLI `package --dry-run --format json` usa el mismo plan.
- Python `export_bundle(spec: dict) -> dict` usa el mismo spec.
- Benchmarks prueban `plan` y `execute`.

---

## 13. Plan de implementación por fases

### Fase 0 — Congelar contratos y reproducibilidad

- Agregar tests de paridad schema Rust/Python.
- Cambiar CI Python a pytest.
- Agregar test de `.pyi` vs API nativa.
- Registrar issues de GUI/API/CLI con IDs.

### Fase 1 — Correcciones P0

- Schema policy unificada.
- CLI trace propaga errores.
- Loader authoring no oculta parse errors.
- Export executable valida antes de escribir o usa temp dir.
- Python `.pyi` actualizado.
- Renombrar UI `Export Game` mal etiquetado.

### Fase 2 — GUI design system

- `DisplayProfile`, `UiTheme`, `ComponentStyle`, `StageViewportPolicy`.
- Migrar player menu/dialogue/choice overlays a tokens.
- Editor de tema con draft/apply/revert.
- Tests de resolución/DPI/layout.

### Fase 3 — SceneFrame y presenters

- Crear `SceneFrameBuilder` en core.
- Implementar presenter egui compartido por player/editor.
- Mantener código actual como adapter durante migración.

### Fase 4 — RouteTree

- Derivar árbol desde `ScriptCompiled`/`StoryGraph`.
- Persistir/consultar progreso.
- Mostrar route tree en Visual Composer, Play Mode y Player Menu.
- Exponer en Python/CLI.

### Fase 5 — Export unificado

- `ExportSpec` único.
- Plan/dry-run/execute.
- Wizard GUI.
- CLI JSON.
- Python parity.
- Atomic export.

### Fase 6 — Performance/cache

- `Arc<[u8]>`/`Bytes` para cache.
- Hash streaming.
- Prefetch route-aware.
- Métricas reales.
- Benchmarks.

### Fase 7 — QA profesional

- Golden tests de layout/theme/route/export.
- Fuzz/property tests para scripts y graph.
- Tests de errores de filesystem/permisos.
- Matriz CI por plataforma si aplica.

---

## 14. Criterios de cierre V3, trazabilidad y evidencias

Esta sección convierte la auditoría V3 en una **Definition of Done verificable**. Los criterios no deben leerse como una lista decorativa: cada criterio cruza hallazgos, módulos, fases y pruebas. La intención es evitar que el cierre se acepte por cambios superficiales, por tests que no ejercitan el flujo real, o por resolver solo GUI/core dejando API/CLI inconsistentes.

### 14.1. Niveles de cierre

- **P0 / Bloqueante:** debe cumplirse para aceptar cualquier corrección principal. Si falla uno, V3 no puede considerarse cerrada ni segura para integrar.
- **P1 / Cierre V3 completo:** debe cumplirse para declarar terminada la actualización V3. Puede dividirse en PRs, pero no debe omitirse del alcance V3.
- **P2 / Endurecimiento V3.x:** mejora profesional recomendada. No bloquea la primera integración si queda documentada con issue, owner, riesgo y prueba pendiente.

**Regla de cierre:** V3 solo se considera completa si todos los criterios **P0** y **P1** están cumplidos, con evidencia de pruebas. Los **P2** pueden quedar como backlog técnico si tienen referencia cruzada al hallazgo, al riesgo y al test futuro. No se acepta “queda a criterio del implementador” cuando el criterio afecta core, exportación, guardado/carga, CLI, API o integridad de datos.

### 14.2. Convención de referencias cruzadas

Para mantener trazabilidad, cada criterio usa estas referencias:

- **Hallazgos Core:** `C1..C10` en §3 y `C-add1..C-add8` en §C.
- **Hallazgos GUI/editor/player:** `G1..G11` en §4 y `B/G1..B/G7` en §B.
- **Hallazgos API Python:** `A1..A6` en §5 y §D.
- **Hallazgos CLI:** `L1..L6` en §6 y §E.
- **Runtime/render:** `R1..R2` en §7.
- **Arquitectura objetivo:** §11, §12, §A, §16.7.
- **Plan de implementación:** §13/Fase 0..7 y §H.
- **Tests:** §10 y §G.

Cuando un criterio diga “Refs”, debe actualizarse también la sección referenciada si durante la implementación se descubre que el hallazgo cambió, quedó resuelto parcialmente o requiere otro test.

### 14.3. Criterios P0 / bloqueantes

| ID | Área | Criterio de cierre | Refs cruzadas | Evidencia mínima requerida |
|---|---|---|---|---|
| **CC-P0-01** | Core/API | Un script aceptado por Python debe ser aceptado por Rust, o ambos deben devolver el mismo error normalizado. No debe existir doble política silenciosa de schema. | §3/C1, §5/A1, §10/Core, §13/Fase 1 | Tests compartidos Rust/Python con scripts válidos, legacy, schema faltante, schema futuro y schema corrupto. |
| **CC-P0-02** | Core | Separar explícitamente `validate_strict`, `normalize_with_warnings` y migraciones. Ninguna normalización destructiva debe ocurrir sin warning trazable. | §3/C1, §C/C-add1, §16.2 | Tests de migración, warnings serializados y errores estables. |
| **CC-P0-03** | GUI/runtime | Play Mode, Visual Composer y player no pueden ignorar errores de `step`, `choose`, `resume`, carga, exportación o guardado. | §B/G6, §4/G5, §10/GUI | Test que fuerce choice inválido, jump inválido y resume fallido; la UI debe mostrar diagnóstico y registrar fallo. |
| **CC-P0-04** | CLI | Ningún comando CLI debe devolver exit code 0 si falló un paso crítico. Debe existir contrato de exit codes y salida JSON para flujos automatizables. | §6/L1, §6/L4, §E, §10/CLI | Golden tests de éxito/error para `validate`, `compile`, `trace`, `package`, `manifest`. |
| **CC-P0-05** | Exportación | Exportar bundle/juego debe ser atómico: si falla runtime, assets, firma, manifest o permisos, no quedan artefactos parciales publicados. | §3/C7, §12, §C/C-add7, §13/Fase 5 | Tests con filesystem temporal simulando fallo a mitad de export; staging y rollback verificados. |
| **CC-P0-06** | Integridad/seguridad | Hash/HMAC/integridad deben cubrir runtime, launcher, manifest, script y assets declarados; no solo una parte del bundle. | §8, §C/C-add8, §12 | Test que modifica runtime/asset/script después de export y verifica detección. |
| **CC-P0-07** | API Python | `.pyi`, nombres públicos y comportamiento real del módulo nativo deben estar sincronizados. APIs placeholder deben fallar explícitamente o implementar datos reales. | §5/A1, §5/A4, §D, §10/API Python | Test automático de paridad stub/runtime y casos para `get_memory_usage`, `is_loading`, `visual_state`, `export_bundle`. |
| **CC-P0-08** | Persistencia | Guardado/carga no debe perder silenciosamente progreso de lectura, ruta, choices o estado visual. | §C/C-add2, §C/C-add3, §10/Core | Tests de save/load en mitad de rama, después de choice y después de jump. |
| **CC-P0-09** | Archivos | Cualquier escritura de proyecto, authoring, export o config debe ser atómica o tener rollback/documentación de riesgo. | §4/G7, §6/L5, §B/G7, §13/Fase 1 | Tests de interrupción/fallo de escritura y validación de archivo anterior intacto. |
| **CC-P0-10** | CI/tests | La suite que corre en CI debe ejecutar realmente las reglas esperadas. No debe haber diferencia peligrosa entre `pytest` local y `unittest discover` en CI. | §10/CI, §G/API-CLI | Workflow actualizado y test que falla si skips/markers críticos no se aplican. |

### 14.4. Criterios P1 / cierre completo de V3

| ID | Área | Criterio de cierre | Refs cruzadas | Evidencia mínima requerida |
|---|---|---|---|---|
| **CC-P1-01** | GUI | Debe existir un sistema visual declarativo (`UiTheme`/tokens o equivalente) para colores, tipografía, espaciado, bordes, diálogo, choices, menú, overlays y route tree. | §4/G1, §4/G2, §B/G2, §11, §16.7 | Theme JSON de ejemplo, validador y test de cambio de tema sin recompilar. |
| **CC-P1-02** | Layout | Debe existir `DisplayProfile`/`StageProfile`/`LayoutPolicy` o equivalente para resolución, DPI/PPI/TPI efectivo, escala de usuario, fullscreen/windowed, safe area y aspect ratio. | §B/G1, §4/G1, §11, §A | Snapshot/headless tests en 320x240, 800x600, 1280x720, 1920x1080, 3440x1440 y escalas 0.75..3.0. |
| **CC-P1-03** | Presenter | Visual Composer, Play Mode y player exportado deben compartir `SceneFrame`/`SceneFramePresenter` o un adapter documentado que garantice paridad visual. | §3/C4, §4/G3, §B/G3, §7/R1, §13/Fase 3 | Test de paridad: mismo script produce mismo `SceneFrame` y mismos componentes principales en los tres modos. |
| **CC-P1-04** | Route Tree | Debe existir `RouteTree` nativo con choices, jumps, conditionals, endings, visitados, bloqueados y ruta actual; visible en editor y Play Mode, y exportable/consultable. | §3/C3, §C/C-add2, §H, §16.7 | Tests de rutas con branches anidados, ciclos controlados y endings; comando/API de consulta. |
| **CC-P1-05** | ExportService | GUI, CLI y Python deben usar un servicio común de exportación con `plan`, `dry-run`, `execute`, `report`, manifest y staging. | §3/C7, §4/G6, §5/A3, §6/L6, §12 | Golden report JSON idéntico o compatible entre GUI/API/CLI para el mismo proyecto. |
| **CC-P1-06** | API Python | Python debe exponer objetos/reportes tipados para export, route tree, layout resolve, theme validate, read model y errores estructurados. | §D, §5/A3, §5/A5, §16.4 | Tests Python usando cada API nueva y comparando con core/CLI. |
| **CC-P1-07** | CLI | CLI debe incluir `--json` global o por comando equivalente, `trace --fail-on-engine-error`, `route-tree`, `read-model`, `theme validate`, `layout resolve`, `export plan/execute`. | §E, §6/L1..L6, §13/Fase 5 | Golden tests stdout/stderr/exit code y documentación de ejemplos. |
| **CC-P1-08** | Cache/perf | Cache de assets debe evitar clones innecesarios, usar presupuesto de memoria, invalidación robusta y hashes estables/streaming para export. | §3/C2, §3/C9, §3/C10, §B/G5, §9 | Benchmarks de memoria pico, cache hit/miss, assets grandes y export streaming. |
| **CC-P1-09** | Componentes | Debe existir registro o inventario formal de componentes visuales reutilizables: dialogue, choices, menu, save slots, history, settings, route tree, toast, loading, status bar, inspector. | §F, §4/G10, §11 | Test o demo que monte componentes desde registry/config y detecte componentes huérfanos. |
| **CC-P1-10** | Documentación | El MD debe mantenerse como mapa de auditoría: cada cambio relevante debe cerrar o actualizar su hallazgo, criterio, prueba y fase. | §2, §13, §14, §H | Changelog técnico o tabla “resuelto/parcial/pendiente” con links a criterios CC. |

### 14.5. Criterios P2 / endurecimiento profesional V3.x

| ID | Área | Criterio recomendado | Refs cruzadas | Evidencia esperada |
|---|---|---|---|---|
| **CC-P2-01** | GUI tooling | Agregar `ThemeEditor`, `Layout Debug Overlay`, `SceneFrame Inspector`, `Export Report Panel` y `Profiler/Cache Panel`. | §F, §B/G4, §B/G5 | Demo o screenshots + pruebas básicas de apertura/cierre/persistencia. |
| **CC-P2-02** | Accesibilidad/localización | Separar textos hardcodeados, soporte de escalado de fuente, labels accesibles e idioma largo sin romper layout. | §4/G11, §11 | Tests con strings largos, fuente grande y cambio de idioma. |
| **CC-P2-03** | Runtime visual | Reemplazar placeholders de render por comandos visuales reales para fondos, sprites, texto, choices, transiciones y overlays. | §7/R1, §7/R2, §3/C4 | Tests visuales mínimos o snapshots de `RenderCommand`. |
| **CC-P2-04** | CI multiplataforma | Matriz CI para Windows/Linux/macOS cuando el runtime/export lo requiera. | §10/CI, §12 | Workflow y artefactos por plataforma. |
| **CC-P2-05** | Extensibilidad | Definir límites para plugins/componentes externos sin romper seguridad ni contratos de engine. | §8, §11, §A | Documento de API interna y prueba de componente externo mínimo. |

### 14.6. Checklist final antes de declarar V3 cerrada

- [ ] Todos los `CC-P0-*` están cumplidos con pruebas ejecutadas.
- [ ] Todos los `CC-P1-*` están cumplidos o divididos en PRs ya integrados dentro de V3.
- [ ] Cada hallazgo `C*`, `G*`, `A*`, `L*`, `R*`, `C-add*` y `B/G*` fue marcado como **resuelto**, **parcial** o **pendiente justificado**.
- [ ] Cada pendiente P2 tiene issue/backlog, riesgo, owner, criterio de prueba y referencia al hallazgo original.
- [ ] GUI/API/CLI/core producen errores compatibles para el mismo fallo de entrada.
- [ ] Exportación desde GUI/API/CLI produce reportes equivalentes y no deja estado parcial.
- [ ] El sistema visual puede cambiar tema/layout/resolución sin modificar lógica de narrativa.
- [ ] Route Tree, read model y progreso sobreviven save/load y son consultables desde al menos core + una interfaz externa.
- [ ] La suite CI ejecuta tests de core, GUI/headless o snapshots, API Python, CLI y export.
- [ ] El Markdown se actualizó con la evidencia de cierre, no solo con una afirmación de que “ya quedó”.

## 15. Resumen ejecutivo

El core tiene una base razonable, pero necesita contratos más estrictos y modelos nuevos para `SceneFrame`, `RouteTree`, exportación atómica e integridad completa. El GUI es la parte que más necesita generalización: hoy funciona como interfaz egui concreta, pero no como sistema visual parametrizable por tema/resolución/DPI. API y CLI tienen gaps importantes de paridad y falsos positivos. La corrección no debe ser borrar cosas, sino convertir piezas existentes en capas reutilizables: comandos comunes, export spec común, theme system común, route tree común y presenter común.

---

## 16. Ampliación v3 — GUI generalizable, más errores de Core y paridad API/CLI

Esta ampliación responde al objetivo principal: **no cerrar la novela a un estilo visual fijo**. La meta no es quitar features ni simplificar: es convertir la GUI actual en un sistema visual parametrizable, portable y testeable, donde el look, el layout, el TPI/DPI/PPI efectivo, la resolución, el modo ventana/fullscreen, los breakpoints y los componentes puedan cambiarse sin tocar la lógica del motor.

### 16.1. Definición más precisa de lo que se busca

El motor debería separar cuatro capas:

1. **Core narrativo**: script, estado, choices, rutas, save/load, seguridad, export, cache, assets y contratos estables.
2. **Modelo de presentación**: `SceneFrame`, `RenderCommand`, `UiIntent`, `RouteTreeViewModel`, `GameScreenState`. Debe ser backend-agnostic, serializable y usable por GUI, API, CLI, tests y runtime exportado.
3. **Sistema visual configurable**: `VisualNovelSkin`, `UiTheme`, `TypographyScale`, `SpacingScale`, `ComponentStyle`, `AnimationStyle`, `ViewportPolicy`, `ResponsiveProfile`, `SafeAreaInsets`, `InputProfile` y `SceneCoordinateMapper`.
4. **Adaptadores concretos**: egui/editor actual, player nativo, visual composer, export runtime, screenshots/golden tests y posibles frontends futuros.

Sin esta separación, cada mejora visual seguirá quedando mezclada con egui, menús, estado de runtime, recursos y reglas de layout. Eso vuelve caro cambiar un textbox, soportar otra resolución, adaptar a pantallas densas, permitir skins, añadir localización CJK/RTL o renderizar la misma pantalla en juego exportado.

### 16.2. Errores y huecos adicionales de Core

**C11. `ScenePatch` dice soportar clear/null, pero el schema no lo permite.** `visual.rs` comenta que para limpiar valores se use `Patch` con null explícito, pero `ScenePatchRaw.background/music` son `Option<String>`. En serde, un campo ausente y `null` terminan como `None`; por tanto no hay forma de distinguir “no tocar” de “limpiar”. Corregir con un tipo triestado (`Unset | Set(T) | Clear`) o con acciones explícitas (`clear_background`, `clear_music`). Añadir tests JSON con campo ausente, `null`, string vacío y valor válido.

**C12. `CharacterPatchRaw` no permite modificar `x/y/scale`.** `CharacterPlacementRaw` sí tiene posición precisa, pero `CharacterPatchRaw` solo tiene `name/expression/position`. Eso rompe Visual Composer cuando quiere mutar una instancia ya existente sin reinsertarla. Añadir `x`, `y`, `scale`, y una semántica clara para no tocar vs limpiar vs setear.

**C13. Duplicados de personajes producen no-op silencioso.** `set_character_position` retorna sin error si hay más de un personaje con el mismo `name`. En un motor profesional esto debe ser diagnóstico: usar `CharacterInstanceId`, o devolver `AmbiguousCharacterReference` con lista de candidatos. No debe fallar silenciosamente.

**C14. Contrato de transición inconsistente.** El compilador acepta `cut` como kind `2`, pero `UiState::transition_kind_label` solo traduce `fade` y `dissolve`; `cut` termina como `unknown`. Agregar enum tipado `TransitionKind` y tests de roundtrip raw → compiled → UI/export.

**C15. Detección de ciclos puede marcar ancestros como nodos cíclicos.** `detect_reachable_cycle_nodes` propaga `cycle_nodes.contains(target)` hacia el padre, por lo que un nodo que solo alcanza un ciclo puede aparecer como parte del ciclo. Separar `nodes_in_cycle` de `nodes_that_can_reach_cycle` usando SCC/Tarjan o Kosaraju iterativo. Evitar DFS recursivo para grafos grandes.

**C16. Save/load pierde progreso visual de ruta.** `SaveData` guarda `EngineState`, pero `set_state` limpia `choice_history`, `read_dialogue_ips` y `pending_transition`. Eso impide reconstruir route progress, backlog leído y rama actual tras cargar. Crear `EngineSessionState` con campos versionados y migrables.

**C17. Validación raw y compiled no tienen el mismo límite de labels.** Raw acepta labels con índice `== events.len()`, pero compiled rechaza `start_ip >= events.len()`. Esto crea contratos distintos entre validate/compile/runtime. Definir si `events.len()` es “end sentinel” permitido y aplicarlo igual en todos los eventos y entrypoints.

**C18. `SecurityPolicy` es demasiado estrecha.** Hoy prácticamente solo controla speaker vacío y algunos límites. Falta validar volumen/fade finitos y en rango, color de transición, duración máxima, allowlist de `ExtCall`, permisos de filesystem/red/runtime, paths por categoría y capabilities exigidas por export.

**C19. `ExecutionContract` sobredeclara soporte.** `EXT_CALL` y `SUBGRAPH_CALL` aparecen como runtime/export reales, pero el engine core no ejecuta `ExtCall` sin binding externo y no hay soporte completo de subgraph runtime. Cambiar a capability explícita: `core_pauses_for_extcall`, `runtime_requires_host_handler`, `editor_only`, `not_implemented`.

**C20. Export usa `ExportPlan` de forma decorativa.** `export_bundle` construye plan y lo descarta. Además `build_export_plan` puede acumular `errors`, pero el export real no consume un plan materializado ni un reporte de preflight. Convertir en pipeline único: `plan -> validate -> materialize -> verify -> report`.

**C21. Export no aplica la misma validación de seguridad que runtime.** Cargar y compilar para exportar no debe saltarse `SecurityPolicy::validate_raw/compiled`. El mismo script debe fallar igual en `validate`, GUI, API, CLI y export.

**C22. HMAC e integridad todavía son parciales.** La firma cubre bytes compilados, asset manifest y manifest, pero no necesariamente todo el layout final, launcher, report, runtime artifact ni metadata canónica. Definir `BundleManifest` canónico y firmarlo completo.

**C23. Cache LRU no es profesional todavía.** `LruCache` usa `VecDeque` y `HashMap<K, Vec<u8>>`; `touch` es O(n), clona buffers y no expone hits/misses/evictions/peak/pressure. Migrar a `Bytes`/`Arc<[u8]>`, lista enlazada/indexada o crate probado, métricas y budgets por tipo de asset.

**C24. `UiState` es resumen textual, no contrato de render.** `UiState` mezcla strings de presentación con estado. Debe derivar a `SceneFrame`/`GameScreenState`, con comandos estructurados para background, personajes, audio, transición, diálogo, choices, overlays y route tree.

**C25. Falta un registro central de comandos/capabilities.** `ExtCall`, audio, transición, export, route tree, theme y editor actions deberían declararse en un `CommandRegistry`/`CapabilityRegistry`, no repetirse en GUI/API/CLI.

### 16.3. GUI: hallazgos adicionales y rediseño necesario

**G12. `DisplayInfo` existe pero no se usa en la ruta principal.** `VnConfig::resolve(display)` tiene lógica para pantalla pequeña, pero `run_app` llama `config.resolve(None)`. El resultado es que el modo fullscreen automático, escala por display y adaptación a altura real no aplican.

**G13. El player sigue anclado a `DEFAULT_STAGE_SIZE = 1280x720`.** `render_player_stage` usa `player_stage_viewport_size(..., DEFAULT_STAGE_SIZE)` y `fit_rect_to_stage(..., DEFAULT_STAGE_SIZE)`. Debe venir del proyecto, manifest, theme o `StageSpec`: design resolution, aspect policy, crop/letterbox/fill, safe area y virtual coordinate system.

**G14. La geometría de overlays está hardcodeada.** `dialogue_overlay_rect`, `choice_overlay_layout` y `scene_overlay_rect` usan porcentajes, clamps y alturas fijas. Es útil como default, pero debe moverse a `ComponentLayoutPolicy` configurable por skin. El diseñador debe poder cambiar posición del textbox, ancho, altura, anchor, padding, comportamiento en ultrawide/portrait y visibilidad por breakpoint.

**G15. El cálculo de texto no usa métricas reales.** `estimate_wrapped_height` estima ancho por cantidad de caracteres. Eso falla con CJK, RTL, emojis, fonts proporcionales, accesibilidad y escalas altas. La medición debe estar en el adaptador de render, pero el modelo debe declarar constraints; los tests deben usar snapshots lógicos y golden visuales por viewport.

**G16. `set_pixels_per_point` no resuelve independencia visual completa.** Escalar egui globalmente no basta: se necesitan tokens independientes para fuente, spacing, touch target, iconos, bordes, animaciones, densidad y lectura. Agregar `DensityProfile` (`compact`, `comfortable`, `touch`, `cinematic`) y `TypographyRamp`.

**G17. `PlayerMenuStyleConfig` es parcial.** Sirve para algunos botones, pero no cubre diálogo, choices, save/load, backlog, settings, route tree, inspector, end screen, debug overlay, animaciones ni estados hover/disabled/focus. Crear `GameUiConfig` completo y versionado, con migración de la config actual.

**G18. Strings y acciones están mezclados con presentación.** Hay labels en inglés/español y acciones duplicadas entre toolbar/menu/player. Crear `CommandRegistry` con id estable, label localizable, shortcut, enabled-state, telemetry/event log y handler. La GUI renderiza comandos, no decide lógica repetida.

**G19. Visual Composer usa tamaños/posiciones default codificados.** Stage geometry fija tamaños de Character/Image/Video/Audio/Text y posiciones iniciales. Esos defaults deben venir de `NodeVisualPolicy` y `AssetPlacementPolicy`, de modo que un proyecto pueda usar estilo chibi, bust-up, full-body, mobile, cinematic o comic panel sin tocar Rust.

**G20. Falta `SceneCoordinateMapper`.** Necesitas mapear coordenadas de diseño a pantalla real, preview, screenshots, export runtime y hit testing. Debe manejar letterbox/crop/fill, safe-area, overscan, DPI/TPI, zoom del editor, pan y conversión inversa para drag/drop.

**G21. El Workbench es un god-object.** `EditorWorkbench` concentra graph, authoring session, undo, manifest, composer cache, engine, layout, validation y logs. Separar servicios: `ProjectService`, `AuthoringGraphService`, `PreviewRuntimeService`, `ExportService`, `ThemeService`, `RouteTreeService`, `ValidationService`, `CommandService`.

**G22. Layout de editor con breakpoints fijos.** `WorkspaceLayout` define panels y tamaños en Rust. Debe tener `WorkspaceLayoutSpec` serializable: panel registry, dock areas, min/max, collapse rules, presets y persistencia por proyecto/usuario.

**G23. No hay Theme Studio real.** Agregar una pantalla para editar `.vntheme.json`: tokens, fuentes, colores, spacing, radios, sombras, assets de chrome, variantes de diálogo/choices/menu, preview multi-viewport y validación en vivo.

**G24. Route Tree debe ser componente visual nativo.** No basta historial lineal. El GUI debe mostrar árbol con nodo actual, elecciones tomadas, ramas bloqueadas, finales, porcentaje de avance, preview de nodos y filtros. Debe verse en Visual Composer, Play Mode y player exportado con permisos de spoiler configurables.

**G25. Export desde GUI debe ser wizard profesional.** El usuario debe poder elegir target, runtime artifact, output, integrity, optimization profile, dry-run, incluir/excluir assets, reporte y abrir carpeta. No debe confundirse `.vnproject`/script compilado con juego exportable.

### 16.4. API Python: ampliación necesaria

**A7. `export_bundle` de Python es menos capaz que Core/CLI.** Su firma no expone todas las opciones de integridad, hmac, layout, optimization profile ni plan/dry-run. Debe aceptar/retornar `ExportBundleSpec`, `ExportPlan` y `ExportBundleReport` tipados, no solo string JSON.

**A8. `.pyi` debe generarse o validarse automáticamente.** Hay drift en firmas como `choose/resume`, y faltan métodos modernos. Añadir test que importe el módulo nativo, compare `inspect.signature` contra stubs y falle si faltan símbolos.

**A9. `visual_state` no expone todo lo que el core sabe.** Debe incluir `x/y/scale`, instance id si existe, z-index/layer futuro y metadata necesaria para Visual Composer/API tools.

**A10. Métricas placeholder deben ser explícitas.** `get_memory_usage` con `current_texture_bytes=0` e `is_loading=false` pueden inducir falsos positivos. Retornar `NotImplemented`, `None`, o métricas reales desde resource/cache/runtime.

**A11. ExtCall en Python puede quedar ambiguo.** Cuando no hay handler permitido, se almacena `last_ext_call_error`, pero la ejecución puede parecer exitosa. Normalizar comportamiento: evento `PausedForExtCall`, error estructurado, o excepción según modo.

**A12. PyNodeGraph silencia mutaciones.** Métodos que hacen `let _ = ...` o devuelven solo `bool` deben devolver `Result`/diagnóstico, con códigos estables para UI/CLI/tests.

**A13. Exponer también theme, route tree y export plan.** La API debe permitir automatizar diseño visual, validar temas, generar previews, consultar rutas y ejecutar exportaciones reproducibles.

### 16.5. CLI: ampliación necesaria

**L7. `compile` debe validar igual que `validate`.** Ahora compilar no debe saltarse seguridad. Usar el mismo pipeline `load -> schema policy -> validate_raw -> compile -> validate_compiled -> write_atomic`.

**L8. `trace` no debe ocultar errores.** Los descartes de resultados de `engine.choose/resume/step` deben convertirse en fallos o eventos de traza con `stopped_reason`. Añadir `--choice-policy first/random/seed/file`, `--max-steps`, `--json`, `--fail-on-warning`.

**L9. Loader authoring no debe esconder JSON corrupto.** Si un documento parece authoring pero falla parseo, no debe caer silenciosamente a script legacy. Reportar error original con contexto.

**L10. Manifest debe ser streaming y estricto.** No usar `filter_map(Result::ok)` para ignorar errores de walkdir; no leer archivos completos para hash si pueden ser grandes. Agregar excludes, política de symlinks, límite de tamaño y reporte JSON.

**L11. `package` necesita modo plan/dry-run/json.** Agregar `vnengine export plan`, `vnengine package --dry-run --json --report`, integridad por env/file, temp dir atómico y limpieza en error.

**L12. Faltan comandos de diagnóstico profesional.** Añadir `doctor`, `inspect script`, `inspect bundle`, `route-tree`, `theme validate`, `theme preview`, `cache stats`, `bench smoke` y `api-stub-check`.

**L13. Exit codes y output deben ser estables.** Documentar códigos: uso inválido, script inválido, seguridad, IO, export parcial, runtime trace, internal. Los tests deben verificar stdout/stderr y JSON schema.

### 16.6. Tests y benchmarks que faltan por esta ampliación

- **Core**: scene patch triestado, update de x/y/scale, duplicados ambiguos, transición `cut`, labels en `events.len()`, ciclos SCC, seguridad audio/transition/extcall, save/load de session state.
- **GUI**: snapshots lógicos de layout para 320x568, 720p, 1080p, 1440p, ultrawide, HiDPI, escala 0.75/1/2/3, fullscreen/windowed, safe-area, CJK/RTL y texto largo sin espacios.
- **Visual Composer**: drag/drop con mapper, zoom/pan, placement policy por asset, roundtrip `SceneFrame` ↔ nodos.
- **API**: paridad `.pyi`, errores estructurados, route tree, export plan, visual_state completo, métricas no-placeholder.
- **CLI**: comandos con input inválido, permisos de filesystem, output JSON, exit codes, escritura atómica, manifest con symlink/archivo grande/error de lectura.
- **Export**: dry-run no escribe, error no deja bundle parcial, bundle manifest canónico, HMAC reproducible, target sin runtime artifact.
- **Benchmarks**: parse/compile de scripts grandes, route tree, graph SCC, cache hit/miss/eviction, export con muchos assets, layout multi-viewport y manifest hashing streaming.

### 16.7. Modelos nuevos recomendados

```rust
pub struct DisplayProfile { pub physical_size: (u32,u32), pub scale_factor: f32, pub safe_area: SafeAreaInsets }
pub struct StageSpec { pub design_size: (f32,f32), pub policy: ViewportPolicy }
pub struct VisualNovelSkin { pub theme: UiTheme, pub components: ComponentStyleRegistry, pub responsive: Vec<ResponsiveProfile> }
pub struct SceneFrame { pub visual: VisualState, pub dialogue: Option<DialogueFrame>, pub choices: Vec<ChoiceFrame>, pub overlays: Vec<OverlayFrame>, pub route: Option<RouteTreeViewModel> }
pub struct ExportPipeline { pub plan: ExportPlan, pub validation: ValidationReport, pub materialization: ExportBundleReport }
```

Estos modelos permitirían que Core defina datos y contratos, GUI decida presentación concreta, API/CLI automaticen flujos, y export use exactamente el mismo pipeline que el editor/player.


---

# Addendum V3 — foco específico en GUI generalizable, API/CLI y core profesional

Esta sección complementa la auditoría anterior sin recortar hallazgos. El criterio principal es que el motor no debe quedar cerrado a un único estilo visual ni a una resolución fija. La UI debe poder cambiar de piel, tipografía, densidad, distribución, breakpoints, overlays, árbol de rutas y exportación sin reescribir cada vista de `egui`. En esta V3 se asume que “TPI” se está usando como densidad/escala percibida de pantalla, junto con DPI/PPI, factor de escala del sistema, escala del usuario y modo ventana/fullscreen.

## A. Lectura arquitectónica actual: qué hay, qué falta y dónde debe vivir

Hoy el repo mezcla tres niveles que deberían estar separados:

1. **Contrato del motor**: eventos, estado, route tree, exportación, cache, layout resuelto, escena renderizable. Debe vivir en `crates/core`, sin depender de `egui`.
2. **Presentación/adaptadores**: `egui` para editor/player, PyO3 para Python, comandos CLI. Deben consumir contratos del core, no inventar contratos paralelos.
3. **Autoría/UX**: Visual Composer, Play Mode, inspector, wizard de exportación, route tree visual, editor de temas. Deben ser herramientas sobre el mismo modelo, no forks visuales.

La corrección no es “simplificar”: es **mover decisiones comunes al core** y dejar GUI/API/CLI como consumidores. El objetivo mínimo profesional sería introducir estos contratos compartidos:

```rust
DisplayProfile     // pixels lógicos/físicos, scale_factor, dpi/ppi/tpi, user_scale, fullscreen, safe_area
StageProfile       // resolución base lógica, aspect policy, coordenadas de escena, overscan/safe frame
LayoutPolicy       // breakpoints, densidad, reglas min/max, anchors, paneles, orientación
UiTheme            // tokens: color, tipografía, spacing, radius, border, opacity, motion, z-index
ComponentStyle     // textbox, choice_list, button, route_tree, menu, save_slots, toast, inspector
SceneFrame         // salida render-agnóstica del estado actual: layers, overlays, input affordances
RouteTree          // ramas, choices, jumps, endings, visitado/no visitado, current node, progreso
ExportService      // plan/validate/execute/report común para GUI/API/CLI
```

## B. GUI: errores adicionales y plan de generalización visual

### G1. Configuración de pantalla incompleta y resolución todavía fija

**Evidencia:** `crates/gui/src/app.rs:30` fija `DEFAULT_STAGE_SIZE = (1280.0, 720.0)`. `VnConfig::resolve` usa defaults 1280x720, fullscreen automático solo si `display.height < 720`, y `asset_cache_budget_mb` default 128 MB en `app.rs:80-98`. En `run_app` se llama `config.resolve(None)` (`app.rs:164-179`), así que el `DisplayInfo` casi no entra al arranque real. Luego `VnApp::new` aplica `pixels_per_point` como `config.scale_factor * prefs.ui_scale` (`app.rs:437-439`) sin una política formal de DPI/PPI/TPI, safe area, orientación, monitor, ventana/fullscreen o accesibilidad.

**Impacto:** aunque hay intentos de responsive, la resolución base y la escala están repartidas. En pantallas chicas, ultrawide, HiDPI, docking, cambios de ventana, fullscreen o user scale alto, la UI puede verse comprimida, inconsistemente escalada o con overlays fuera de proporción.

**Corrección:** crear `DisplayProfile` y `StageProfile` en core o en un crate compartido sin `egui`. GUI debe resolverlos cada frame/resize y usar `LayoutResolver`. `DEFAULT_STAGE_SIZE` solo debe ser el default de un `StageProfile`, no una constante usada directamente en render paths.

### G2. El diseño visual está hardcodeado en rutas de render

**Evidencia:** `render_player_stage` pinta fondo `Color32::from_rgb(16,18,24)` (`app.rs:501-516`). `paint_character_labels` usa offsets, fuente, padding, radio y color fijos (`app.rs:551-582`). `render_dialogue_overlay` fija radio `6.0`, colores RGBA, stroke, shrink, speaker color y botón “Continue” (`app.rs:609-663`). El player embebido/editor repite otro estilo: `player_ui/content.rs:75-91` usa frames de color fijo; `content.rs:166-190` vuelve a definir overlay con alphas distintos; choices usan botones `200x40` y paddings fijos (`content.rs:242-268`, `272-344`).

**Impacto:** no hay un sistema de skins. Cambiar tipografía, caja de diálogo, estilo de choice, colores, padding, bordes, animación, densidad o idioma implica tocar múltiples funciones y duplicar comportamiento.

**Corrección:** introducir `UiTheme` serializable con tokens y `ComponentStyle`. Prohibir que rutas de render creen colores/fuentes/tamaños directos salvo en un `ThemeCompiler` que convierta tokens a `egui::Style`. Los componentes deben pedir `theme.component("dialogue_box")`, `theme.component("choice_list")`, etc.

### G3. Visual Composer y Play Mode duplican overlays en lugar de compartir un presenter

**Evidencia:** `crates/gui/src/editor/visual_composer/overlays.rs:189-288` reimplementa dialogue/choice overlay con valores parecidos pero no iguales al player. `player_overlay.rs:11-92` calcula geometría de overlays con clamps y ratios fijos. `visual_composer/viewport.rs:3-12` fija `STATUS_HEIGHT=28` y `VIEWPORT_VERTICAL_BUDGET=0.78`.

**Impacto:** Visual Composer puede mostrar algo distinto a Play Mode y al juego exportado. Eso rompe WYSIWYG y complica la depuración de UI.

**Corrección:** el core debe producir un `SceneFrame`; GUI debe tener un `EguiSceneFramePresenter` usado por Visual Composer, Play Mode y player exportado. Los overlays deben ser instancias de componentes (`DialogueBox`, `ChoiceList`, `RouteTreeMiniMap`, `StatusBar`) con layout resuelto por `LayoutPolicy`, no funciones sueltas.

### G4. Workbench responsive existe, pero está acoplado a paneles fijos y sliders rígidos

**Evidencia:** `workbench/layout.rs:78-86` define ratios/min/max fijos por panel; `menu_bar.rs:125-168` expone sliders con rangos quemados (`Assets 80..420`, `Graph 150..760`, `Inspector 150..520`, etc.).

**Impacto:** el editor no escala como producto profesional cuando cambian resolución, densidad, idioma o cantidad de paneles. Añadir paneles futuros —route tree, export report, theme editor, profiler— requiere tocar lógica manual.

**Corrección:** mover paneles a `WorkspaceLayoutPolicy`: panel registry, prioridades, collapse rules, breakpoints (`compact`, `normal`, `wide`, `ultrawide`), `min_content_size`, `preferred_ratio`, docking y persistencia versionada. Los sliders pueden seguir, pero como overrides sobre la policy, no como fuente primaria.

### G5. Preview/caché de assets no está listo para proyectos grandes

**Evidencia:** `EditorResourceService::load_bytes` lee todo el archivo, calcula fingerprint, clona bytes al devolver y al cachear (`resource_service.rs:63-85`). `image_for_view` clona imágenes (`resource_service.rs:102-137`). Audio crea `MemoryAssetStore` con `bytes.clone()` (`resource_service.rs:140-165`). No hay LRU real, presupuesto por tipo, invalidación por mtime+size+hash, ni `Arc<[u8]>`/`Bytes`. `fingerprint_bytes` usa `DefaultHasher` (`resource_service.rs:185-188`), útil como fingerprint interno pero no estable para reportes/reproducibilidad.

**Impacto:** memoria y CPU crecen mal con imágenes/audio grandes, varias vistas abiertas, previews, thumbnails y exportación.

**Corrección:** usar cache con budget, LRU, weak handles, `Arc<[u8]>`, hashing streaming para export, métricas reales de decode/upload, y políticas separadas para editor preview vs runtime exportado.

### G6. GUI oculta errores críticos de motor durante Play Mode

**Evidencia:** `player_ui/content.rs:261` y `341` ignoran `engine.choose(...)`; `player_ui/render.rs:127`, `250` ignoran `engine.resume()`; varias rutas hacen `if let Ok((cmd,_)) = engine.step()` sin reportar el error (`render.rs:173-176`, `233-236`, `260-264`).

**Impacto:** Play Mode puede seguir mostrando UI aunque el motor haya fallado. Eso genera falsos positivos visuales y dificulta debug.

**Corrección:** crear `EngineUiController` con métodos `advance`, `choose`, `resume` que devuelvan `UiCommandResult`, reporten toast/diagnóstico y registren el fallo en operación/repro trace.

### G7. Exportación GUI sigue siendo ambigua

**Evidencia:** el menú dice `Export Game (.vnproject)` (`menu_bar.rs:39-40`), pero `export_compiled_project` compila el script y escribe bytes a `.vnproject` (`compile_ops.rs:132-175`). `package_bundle_native` sí crea bundle, pero fuerza target al OS host, integridad `None`, `layout_version=1`, y pide runtime file manual (`compile_ops.rs:177-252`).

**Corrección:** renombrar la acción actual a “Export compiled script” o convertirla en export real. Añadir `Export Wizard`: target, runtime, assets, theme, route tree mode, integrity, dry-run, plan diff, progreso, reporte y rollback. Debe usar el mismo `ExportService` que API/CLI.

## C. Core: errores y mejoras adicionales que deben añadirse

### C-add1. `PlayerMenuConfig` no equivale a sistema visual

**Evidencia:** `PlayerMenuStyleConfig` solo cubre panel width, button size, alpha y cuatro colores (`player_menu.rs:126-150`). Defaults y límites son constantes (`player_menu.rs:6-15`). `normalized()` corrige silenciosamente valores inválidos (`player_menu.rs:327-370`).

**Corrección:** mantener `PlayerMenuConfig`, pero integrarlo dentro de `UiTheme`/`ComponentStyle`. Separar `validate_strict`, `normalize_with_warnings` y `migrate_theme`. Los warnings deben llegar a GUI/API/CLI.

### C-add2. Estado de rutas/lectura no se persiste ni se restaura

**Evidencia:** `EngineState` guarda posición, flags, vars, visual e history (`state.rs:12-20`), pero no `read_dialogue_ips`, `choice_history` ni route progress. `Engine::set_state` limpia esas estructuras (`engine/runtime.rs:320-332`).

**Impacto:** cargas de partida y route tree pueden perder progreso, marcas de leído y ruta actual.

**Corrección:** crear `ReadModelSnapshot` y `RouteProgressSnapshot` versionados, serializables y opcionales en saves. `set_state` no debe borrar progreso sin una policy explícita.

### C-add3. Saltos y carga a mitad de script no reconstruyen visual completo

**Evidencia:** `jump_to_ip_with_audio` solo aplica escena si el target inmediato es `Scene` (`engine/runtime.rs:246-255`). Si se salta a diálogo después de una escena previa, el visual puede no representar el estado canónico esperado.

**Corrección:** añadir `resolve_visual_at_ip(ip, strategy)` con checkpoints de escena, cache de snapshots o replay controlado desde último checkpoint. Usarlo para jump, load, preview, route tree y Visual Composer.

### C-add4. `VisualState` no tiene identidad robusta de personajes

**Evidencia:** `set_character_position` retorna silenciosamente si hay más de un personaje con el mismo nombre (`visual.rs:85-90`). `apply_patch.remove` crea un `Vec` y usa `contains`, O(n*m) (`visual.rs:44-52`).

**Corrección:** introducir `CharacterInstanceId` opcional, warnings para nombres ambiguos, y estructuras indexadas (`HashSet`) para removes. No debe fallar en silencio.

### C-add5. `UiState` y `TextRenderer` son demasiado textuales

**Evidencia:** `UiState` solo devuelve `Dialogue`, `Choice`, `Scene`, `System` con strings (`ui.rs:20-37`). `RenderOutput` solo tiene `text: String` (`render.rs:13-17`).

**Corrección:** crear `SceneFrame`/`RenderCommand`: layers, asset refs, dialogue payload, choices con ids, transitions, accessibility labels, route hints y input intents. `TextRenderer` debe ser un adapter de debug, no el contrato principal.

### C-add6. Prefetch es lineal y no branch-aware

**Evidencia:** `peek_next_asset_paths`/prefetch recorre un rango lineal de eventos; no considera choices, jumps, conditionals, route probability ni cache budget.

**Corrección:** prefetch guiado por `StoryGraph/RouteTree`, profundidad, heurística por probabilidad/ruta actual y presupuesto por asset type.

### C-add7. Export plan se valida pero se descarta, y la escritura no es atómica

**Evidencia:** `export_bundle` llama `build_export_plan(&spec)?` y descarta el plan (`bundle.rs:38-40`). Luego crea directorios y escribe directamente en `output_root` (`bundle.rs:45-188`). `export_executable_bundle` falla después de haber generado el bundle si no produjo ejecutable esperado (`bundle.rs:258-276`).

**Corrección:** `ExportService` debe ejecutar en staging temporal, escribir manifest completo, validar ejecutable antes de publicar, y hacer rename/swap final. Debe soportar `plan_only`, `dry_run`, `execute`, `rollback` y reporte JSON estable.

### C-add8. Integridad/HMAC cubre solo parte del paquete

**Evidencia:** HMAC actual cubre compiled bytes, assets_manifest y project manifest (`bundle.rs:190-210`), no necesariamente launcher, ejecutable/runtime, package_report ni todos los archivos finales.

**Corrección:** firmar un `bundle_file_manifest` con ruta, tamaño, sha256 y rol de cada artefacto. Declarar claramente si la firma es parcial o completa.

## D. API Python: hallazgos que faltaban enfatizar

1. `run_visual_novel` valida y luego siempre falla porque GUI no está disponible en extensión headless (`crates/py/src/lib.rs:42-58`). Debe renombrarse a `validate_runtime_config` o implementar backend real.
2. `export_bundle` Python carece de integridad, hmac, plan/dry-run, report path, layout version avanzado y devuelve `String` JSON en vez de objeto tipado (`lib.rs:60-98`).
3. `.pyi` no expone funciones nativas agregadas (`run_visual_novel`, `export_bundle`, `default_player_menu_config`, `validate_player_menu_config`) y tipa mal `choose/resume` (`python/visual_novel_engine.pyi:18-37` vs `bindings/engine.rs:89-93`, `291-296` en core). El `__getattr__` catch-all (`.pyi:16,37,70,89,93,126,147`) puede ocultar stubs incompletos.
4. `visual_state()` omite `x/y/scale` de personajes (`bindings/engine.rs:103-118`), rompiendo layout para clientes Python.
5. `get_memory_usage()` devuelve `current_texture_bytes=0` y `is_loading()` siempre `false` (`bindings/engine.rs:203-229`): son placeholders que parecen métricas reales.
6. `PyNodeGraph` ignora errores (`connect`, `remove_node`) y devuelve bools opacos (`connect_or_branch`) en vez de `PyResult` con diagnóstico (`editor_node_graph.rs:58-126`).

**Corrección API:** generar o testear stubs desde PyO3, exponer objetos tipados (`ExportReport`, `ExportPlan`, `RouteTree`, `SceneFrame`, `UiThemeValidationReport`, `LayoutResolution`), y eliminar placeholders o marcarlos como no implementados con error explícito.

## E. CLI: huecos adicionales y comandos necesarios

1. No hay `--json` global ni `CliEnvelope` común (`vnengine.rs:22-27`). Cada comando imprime distinto.
2. `validate_script` no emite reporte cuando todo está bien (`vnengine.rs:331-339`), difícil para CI.
3. `compile_script` escribe directo sin archivo temporal ni reporte (`vnengine.rs:341-349`).
4. `trace_script` corta en cualquier error de `current_event` y luego ignora errores de choose/resume/step (`vnengine.rs:373-392`), produciendo trazas falsas.
5. `Manifest` usa `WalkDir::filter_map(Result::ok)` y puede saltarse errores de traversal; además lee archivos completos para hash.
6. `authoring apply-command` solo tiene JSON local, escribe antes de imprimir outcome y no ofrece dry-run/transacción (`authoring.rs:192-214`).
7. `load_authoring_document` oculta errores de documentos authoring corruptos al caer a legacy (`authoring.rs:401-410`).
8. `CliDocumentCommand` solo expone cuatro comandos (`authoring.rs:458-495`), insuficiente para automatizar Visual Composer.
9. `package_project` imprime texto humano, no reporte JSON ni plan/dry-run (`package.rs:51-72`).

**Comandos a añadir:**

```bash
vnengine --json validate <script>
vnengine route-tree <script> --state save.vnstate --json
vnengine read-model <save-or-script> --json
vnengine theme validate theme.json --json
vnengine layout resolve --display display.json --theme theme.json --stage stage.json --json
vnengine export plan --project . --target windows --json
vnengine export execute --plan export-plan.json --atomic --json
vnengine trace <script> --fail-on-engine-error --json
```

## F. Componentes GUI faltantes o subutilizados

- `OpenRoutes` existe en `PlayerMenuAction` (`player_menu.rs:58-60`) pero no hay `RouteTreeView` real.
- No hay `ThemeEditor` ni validador visual de tokens.
- No hay `Layout Debug Overlay` que muestre stage rect, safe area, scale, breakpoint y componente seleccionado.
- No hay `Export Report Panel` integrado al editor.
- No hay `Profiler/Cache Panel` que muestre bytes, decode/upload, LRU, asset misses.
- No hay `SceneFrame Inspector` para comparar core frame vs presenter egui.
- No hay registro unificado de componentes: diálogo, choices, menu, save slots, history, settings, route tree, toast, loading, status bar, inspector.

## G. Tests que faltan para evitar falsos positivos

### GUI

- Snapshot/headless layout para 320x240, 800x600, 1280x720, 1920x1080, 3440x1440, vertical y ultrawide.
- Escalas 0.75, 1.0, 1.25, 1.5, 2.0, 3.0; user scale alto y fuentes largas.
- Pruebas de theme switching sin reiniciar.
- Visual Composer, Play Mode y player exportado deben renderizar el mismo `SceneFrame`.
- Errores de `engine.step/choose/resume` deben aparecer como diagnostic/toast y fallar test.

### Core

- RouteTree con choices anidados, jumps, conditionals, ciclos y endings.
- Save/load preserva read model y route progress.
- `resolve_visual_at_ip` para saltos/cargas a diálogo, escena, patch y branch.
- Schema matrix compartida Rust/Python.
- Export atomic: si falla ejecutable o firma, no queda bundle parcial.
- HMAC completo detecta cambios en runtime, launcher y assets.

### API/CLI

- Test automático de paridad `.pyi` vs módulo nativo.
- CLI `--json` golden para éxito/error por comando.
- `trace --fail-on-engine-error` debe fallar ante choice inválido, extcall bloqueado o jump inválido.
- `package --dry-run/plan/execute` debe ser idéntico a Python/GUI.

### Benchmarks

- Parse/migración de 10k/50k/100k eventos.
- Export con assets grandes usando hashing streaming.
- Construcción de RouteTree/SceneFrame.
- Cache hit/miss, decode/upload y memoria pico.

## H. Orden de implementación recomendado

1. P0: schema policy, errores CLI/GUI ignorados, loader authoring estricto, export atomic, stubs Python.
2. Contratos core: `DisplayProfile`, `StageProfile`, `UiTheme`, `LayoutPolicy`, `SceneFrame`, `RouteTree`.
3. Presenter GUI: sustituir colores/tamaños directos por theme tokens y `LayoutResolver`.
4. ExportService común: plan/validate/execute/report/staging.
5. API/CLI parity: objetos tipados, `--json`, comandos de route/theme/layout/export.
6. Cache/perf: `Arc<[u8]>`, LRU budget, streaming hash, benchmarks.
7. Tests de regresión y snapshots para impedir que regresen falsos positivos.

---

# Cierre de implementación y aceptación — 2026-05-28

Estado: implementado y verificado en el árbol local.

## Checklist de aceptación

- [x] P0 schema policy unificada entre Rust/Python/API/CLI/GUI: `SchemaPolicy`, validación estricta, lectura legacy explícita y migración con warnings quedan cubiertas por matriz Rust y Python.
- [x] P0 errores silenciados eliminados en `trace`, GUI/API `step/choose/resume`, authoring loaders, export/package/import y rutas críticas: los fallos devuelven diagnóstico, envelope JSON o toast/reporte según la superficie.
- [x] P0 exportación común: `ExportService` con `plan_export`, `validate_export_plan`, `execute_export`, dry-run JSON, staging temporal, publicación atómica, rollback, progreso, reportes, targets, runtime artifacts, theme/assets y HMAC sobre manifest final.
- [x] P0 Python API tipada: objetos para `ExportPlan/Report`, `RouteTree`, `SceneFrame`, `UiThemeValidationReport`, `LayoutResolution`; `run_visual_novel` ya no actúa como placeholder ambiguo; `.pyi` se prueba contra el módulo real.
- [x] P0 CLI estable: `--json` global, `CliEnvelope`, exit codes, stdout/stderr predecible, `route-tree`, `read-model`, `theme validate`, `layout resolve`, `export plan`, `export execute`, `trace --fail-on-engine-error`, reportes JSON y escrituras atómicas.
- [x] GUI generalizable: `DisplayProfile`, `StageProfile`, `LayoutPolicy`, `UiTheme`, `ComponentStyle`, `ComponentRegistry`, `SceneFrame` y `SceneFramePresenter` integran player, Play Mode, Visual Composer y exportado mediante presenter/layout resolver compartido.
- [x] GUI tooling: Theme Editor, Layout Debug Overlay, RouteTreeView y Export Report Panel quedan integrados en editor/workbench con tokens JSON validados, localización, densidad, accesibilidad, HiDPI, pantallas chicas, vertical, ultrawide y cambios de escala/ventana.
- [x] Core profesional: `RouteTree`, `ReadModelSnapshot`, `RouteProgressSnapshot`, `resolve_visual_at_ip`, `SceneFrame`/`RenderCommand`, VisualState con ids de instancia/diagnósticos de ambigüedad/removes eficientes y prefetch branch-aware.
- [x] Validación/migraciones separadas: `validate_strict`, `normalize_with_warnings` y migraciones para player menu/theme/schema exponen warnings a core/API/CLI/GUI sin normalizaciones invisibles.
- [x] Cache/performance: assets/core usan bytes compartidos, hashing streaming, presupuestos LRU y benchmarks de parse, export assets, route tree, SceneFrame y cache/memoria.

## Verificación ejecutada

Todos los comandos siguientes terminaron con exit code 0:

```powershell
cargo fmt
cargo check -p visual_novel_engine -p vnengine_assets -p vnengine_runtime -p visual_novel_gui -p vnengine_py -p vnengine_cli
cargo test -p visual_novel_engine --no-fail-fast
cargo test -p vnengine_assets -p vnengine_runtime -p vnengine_cli -p vnengine_py --no-fail-fast
cargo test -p visual_novel_gui --no-fail-fast
.venv\Scripts\maturin.exe develop --manifest-path crates\py\Cargo.toml --features extension-module
.venv\Scripts\python.exe -m pytest tests\python -q
cargo bench -p visual_novel_engine --bench core_benches -- --sample-size 10 --warm-up-time 0.1 --measurement-time 0.1
rg -n "let _ =|\.expect\(" crates\core\src crates\gui\src tools\cli\src crates\runtime\src crates\py\src
rg -n "filter_map\(Result::ok\)|unwrap\(\)" tools\cli\src crates\core\src\bundle crates\core\src\renpy_import crates\gui\src\editor
```

Notas de verificación:

- `pytest` reportó `74 passed`.
- La suite GUI reportó `238 passed` en `editor_contract_tests` más los contratos de player, layout, resources y presenter.
- Los benchmarks finales cubrieron parse de scripts grandes, `step_loop`, `choose_option`, `apply_scene`, `route_tree_1000_events`, `scene_frame_snapshot`, `lru_cache_shared_hits_1mb` y `export_assets_streaming_16x64kb`; `choose_option` mejoró y `step_loop` quedó estable tras eliminar snapshots redundantes.
- Los dos barridos `rg` finales no encontraron coincidencias.

Resultado de cierre: no quedan pendientes abiertos del alcance P0/P1/V3 descrito por este documento.
