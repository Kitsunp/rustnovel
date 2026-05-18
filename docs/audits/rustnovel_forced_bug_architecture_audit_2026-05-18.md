# RustNovel - Auditoria de fallos forzados, duplicacion y arquitectura

Fecha: 2026-05-18

Alcance: lectura estatica del workspace local, busqueda dirigida de rutas de
presion y comparacion con arquitecturas open source/permisivas. Esta auditoria
no corrige codigo: su objetivo es forzar bugs compuestos, detectar componentes
que prometen mas de lo que cumplen y preparar una ruta de consolidacion.

## Fuentes externas usadas

- egui `TextureHandle`: referencia util para el modelo de handles de textura y
  vida util de recursos GPU. Fuente: https://docs.rs/egui/latest/egui/struct.TextureHandle.html
- Bevy `AssetServer`/`Handle`: arquitectura basada en handles y cache central de
  assets. Fuente: https://docs.rs/bevy/latest/bevy/asset/struct.AssetServer.html
  y https://docs.rs/bevy/latest/bevy/asset/enum.Handle.html
- Godot `ResourceLoader`: carga de recursos con politica explicita de cache.
  Fuente: https://docs.godotengine.org/en/stable/classes/class_resourceloader.html
- Ren'Py: referencia de motor VN maduro para separacion de guion, displayables y
  prediccion de recursos. Fuente: https://www.renpy.org/doc/html/
- Yarn Spinner: referencia de authoring por nodos/labels y saltos textuales.
  Fuente: https://docs.yarnspinner.dev/3.1/write-yarn-scripts/scripting-fundamentals/jumps
- Licencias permisivas consultadas: Bevy MIT/Apache
  (https://github.com/bevyengine/bevy/blob/main/LICENSE-MIT), egui MIT/Apache
  (https://github.com/emilk/egui/blob/main/LICENSE-MIT), Godot MIT
  (https://github.com/godotengine/godot/blob/master/LICENSE.txt) y Yarn Spinner
  MIT (https://github.com/YarnSpinnerTool/YarnSpinner/blob/main/LICENSE.md).

## Metodo

Se forzaron fallos por combinacion de subsistemas, no solo por casos unitarios.
Los comandos de auditoria buscaron duplicacion de cache, reconstruccion de
scripts, clones pesados, rutas de preview, trazabilidad y placeholders:

- `rg "AssetStore::new|TextureHandle|image_cache|audio_duration_cache|composer_image_cache"`
- `rg "NodeGraph|to_script|to_script_strict|CompilationCache|OperationLog|EvidenceTrace"`
- `rg "ComposerSnapshot|SceneStage|PlayerVisual|visual_state|ExtCall|pending_transition"`
- conteo de `.clone(` por archivo y chequeo de archivos Rust mayores a 500
  lineas.

Resultado de tamano: no se detectaron archivos Rust principales por encima de
500 lineas en `crates` y `tools`. El riesgo actual no es tamano por archivo,
sino duplicacion entre rutas.

## Ejecucion dinamica realizada

Despues de la lectura inicial se ejecutaron pruebas reales y jobs locales para
separar sospechas de fallos reproducibles.

Comandos ejecutados con resultado correcto:

- `cargo test -p visual_novel_gui --locked asset_browser -- --nocapture`
  - 8 tests pasaron.
- `cargo test -p visual_novel_gui --locked scene_stage -- --nocapture`
  - 8 tests pasaron.
- `cargo test -p visual_novel_gui --locked layout -- --nocapture`
  - 20 tests pasaron.
- `cargo test -p visual_novel_gui --locked composer_tests -- --nocapture`
  - 15 tests pasaron.
- `cargo test -p visual_novel_engine --test authoring_evidence_trace_tests --locked -- --nocapture`
  - 1 test paso.
- `cargo test -p visual_novel_engine --test authoring_fragment_view_tests --locked -- --nocapture`
  - 3 tests pasaron.
- `cargo test -p visual_novel_engine --test authoring_choice_connection_tests --locked -- --nocapture`
  - 1 test paso.
- `cargo test -p vnengine_cli --test authoring_command_tests --locked -- --nocapture`
  - 8 tests pasaron.
- `cargo test -p vnengine_cli --test package_command_tests --locked -- --nocapture`
  - 3 tests pasaron.
- `cargo test -p visual_novel_engine --locked --test operation_log_contract_tests -- --nocapture`
  - 1 test paso.

Comandos con senales utiles:

- `python -m pytest tests/python/test_vnengine_composer_report_bindings.py -q -rs`
  salto 10 tests porque el modulo instalado local no exponia los bindings GUI.
- `python -m pytest tests/python/test_vnengine_graph_bindings.py -q -rs`
  paso 2 y salto 9 por falta de bindings/API en el modulo instalado local.
- `python -m pytest tests/python/test_vnengine_bindings.py -q -rs`
  paso 3 y salto 5 por falta de APIs nativas.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\ci-local.ps1 -Job python-tests`
  construyo e instalo el binding con `maturin`, ejecuto 62 tests y fallo 1.

Fallo dinamico reproducido:

```text
test_python_can_inspect_branch_hub_connections_and_positions
AssertionError: Items in the first set but not the second: 3
Items in the second set but not the first: 4
```

Probe directo en la venv creada por el job:

```text
ids 0 1 2
branch1 True
after1 [0, 1, 2] [(0, 0, 1)]
branch2 True
after2 [0, 1, 2, 3] [(0, 0, 3), (3, 0, 1), (3, 1, 2)]
nodes [(0, 'Dialogue', 0.0, 0.0), (1, 'End', -120.0, 180.0), (2, 'End', 120.0, 180.0), (3, 'Choice', 0.0, 90.0)]
```

Interpretacion: el comportamiento de `connect_or_branch` parece coherente con el
modelo 0-based de `NodeGraph`, pero el test Python espera un id `4` y una
posicion que no coincide con `NODE_VERTICAL_SPACING`. Esto no es solo un test
mal escrito: revela que el contrato publico Python de IDs/posicion generada no
esta congelado con fixture cross-cliente. El mismo flujo pasa en core/GUI, pero
falla en la suite Python real al usar el binding construido.

## Hallazgo A - assets duplicados y carga sincrona en GUI

Evidencia local:

- `crates/gui/src/assets.rs` define `AssetManager` con cache propia.
- `crates/gui/src/editor/asset_browser.rs` mantiene `image_cache`,
  `image_failures`, `audio_duration_cache` y crea `AssetStore`.
- `crates/gui/src/editor/scene_stage.rs` crea otro `AssetStore` y resuelve/carga
  imagenes para el stage.
- `crates/gui/src/editor/workbench.rs` mantiene `composer_image_cache`,
  `composer_image_failures` y `audio_duration_cache`.
- `crates/runtime/src/audio.rs` mantiene `audio_cache` separado.

Bug compuesto a forzar:

1. Crear un proyecto con 80-150 PNG grandes y 20 audios.
2. Abrir Asset Browser, seleccionar una escena con background, activar Visual
   Composer y luego Play.
3. Cambiar calidad Draft/Balanced/High y fit Cover/Contain varias veces.
4. Reimportar el mismo archivo bajo el mismo path y cambiar de escena sin
   reiniciar.

Senal esperada:

- Decodificacion repetida en el hilo UI.
- Texturas duplicadas por cache key de thumbnail, scene stage y player.
- Stutter visible al seleccionar escena o al abrir el browser.
- Invalidacion global de caches despues de importar un asset, aunque solo cambio
  un recurso.

Direccion:

Crear un `EditorResourceService` compartido: resolver path una vez, devolver
handles ref-counted, decodificar en background, mantener LRU por bytes y separar
cache de bytes, cache de imagen decodificada y cache de textura GPU. Bevy/Godot
son buenos modelos: handle estable + cache central + politica explicita.

## Hallazgo B - Composer, Player y runtime aun reconstruyen la misma escena

Evidencia local:

- `crates/core/src/authoring/composer.rs` produce `ComposerSnapshot`.
- `crates/gui/src/editor/workbench/player_mode_ops.rs` reconstruye
  `SceneState` desde `VisualState`.
- `crates/gui/src/editor/player_ui/render.rs` vuelve a renderizar desde contexto
  propio.
- `crates/gui/src/editor/scene_stage.rs` renderiza stage y assets con otra ruta.

Bug compuesto a forzar:

1. Crear `Scene` con background y dos personajes duplicando nombre pero con
   poses distintas.
2. Agregar `ScenePatch` que cambia solo una pose y luego un `Choice` con opciones
   largas.
3. En Composer, avanzar el preview, elegir opcion y volver al modo de nodo
   aislado.
4. Comparar objetos visibles, owners, dialogo, choices y safe area contra Play.

Senal esperada:

- Objetos o backgrounds heredados no coinciden entre Composer y Play.
- Choice visible en runtime pero no en snapshot aislado, o texto largo sale del
  overlay.
- Ownership por fallback usa nombre/path cuando falta provenance completa.

Direccion:

Fusionar hacia un `PresentationSnapshot` unico en core: visual state + overlays +
layer objects + provenance + layout de dialogo/choices. GUI y runtime deberian
consumir ese snapshot, no reconstruirlo localmente.

## Hallazgo C - doble grafo y clones pesados en authoring/GUI

Evidencia local:

- `crates/gui/src/editor/authoring_adapter.rs` clona el grafo de core para
  compatibilidad.
- `crates/gui/src/editor/undo.rs` guarda snapshots completos de `NodeGraph`.
- `crates/gui/src/editor/workbench/operation_ops.rs` calcula fingerprint creando
  `AuthoringDocument` y `to_script()`.
- Archivos con mas clones encontrados: `diagnostics.rs`, `trace.rs`,
  `script_sync.rs`, `report_fingerprint.rs`, `asset_browser.rs`,
  `project_ops.rs`, `player_mode_ops.rs`, `operation_ops.rs`.

Bug compuesto a forzar:

1. Generar un grafo con 1000 nodos, 200 choices y 40 fragments.
2. Hacer marquee select, mover grupo, crear fragment, deshacer y rehacer.
3. Importar reporte y aplicar quick-fix de revision.
4. Medir tiempo de `current_authoring_fingerprint`, memoria de undo y latencia
   de seleccion.

Senal esperada:

- Undo retiene multiples copias completas.
- Cambios puramente visuales disparan recomputo semantico completo.
- Python/CLI vuelven a serializar o clonar objetos grandes para operaciones
  simples.

Direccion:

Usar grafo unico con view-state GUI separado, versiones dirty por dominio
(`semantic_version`, `layout_version`, `asset_version`), fingerprints
incrementales y undo por operaciones/deltas. Guardar snapshots completos solo
como checkpoint espaciado.

## Hallazgo D - trazabilidad causal existe, pero puede ser sintetica

Evidencia local:

- `core::authoring::diagnostics` ya tiene `EvidenceTrace`,
  `DiagnosticTarget`, `FieldPath` y envelopes v2.
- `operation_ops.rs` registra operaciones y verification runs, pero valida con
  `validate_authoring_graph_no_io` y puede no pasar por resolver real de assets.
- Los imports de reportes pueden bloquear autofix por stale, pero el usuario
  necesita explicacion causal legible.

Bug compuesto a forzar:

1. Importar asset externo, asignarlo a escena, moverlo en Composer.
2. Borrar el archivo fisico fuera del editor.
3. Ejecutar validacion con project root, exportar reporte, modificar layout e
   importar de nuevo.
4. Pedir `explain(diagnostic_id)` desde CLI/Python.

Senal esperada:

- El diagnostic puede decir "asset missing", pero no demostrar la cadena:
  operacion -> campo -> resolver -> candidato -> regla -> fallo -> fix.
- Layout stale y semantic stale pueden mezclarse en la UX.

Direccion:

Convertir el resolver de assets y targets en productor de evidencia. Cada regla
de validacion debe emitir `TraceAtom` real, no solo construir envelope al final.

## Hallazgo E - OperationLog cubre mucho, pero no es aun el bus unico

Evidencia local:

- Hay `OperationKind` tipado y `VerificationRun`.
- `workbench/ui_actions.rs` traduce acciones del Composer.
- `undo.rs` todavia opera sobre snapshots de grafo y no sobre operaciones
  semanticas.

Bug compuesto a forzar:

1. Crear nodo con click derecho, conectar, editar campo, importar asset,
   arrastrar objeto visual, cambiar layer lock y hacer Ctrl+Z.
2. Exportar `AuthoringDocument`.
3. Reproducir el log en una sesion headless.

Senal esperada:

- Algunas acciones no son replayables porque dependen de estado GUI.
- Undo restaura posicion completa del grafo, no la intencion del usuario.
- VerificationRun no siempre representa el resolver usado por GUI.

Direccion:

Hacer que toda mutacion pase por `AuthoringCommandBus`: produce operacion,
aplica delta, invalida caches por dominio y registra verification run. GUI,
Python y CLI deben llamar el mismo bus.

## Hallazgo F - layout responsive mejora, pero falta arquitectura de docking

Evidencia local:

- Ya no hay archivos grandes y hay tests de layout.
- El Workbench aun decide paneles principales desde una funcion de layout y
  varios paneles compiten por espacio fijo/minimo.
- Asset Browser puede ocupar demasiado espacio cuando hay thumbnails grandes.

Bug compuesto a forzar:

1. Abrir a 480x360, 900x600, 1366x768 y 4K con escala Windows 150%.
2. Activar Asset Browser, Graph, Composer, Inspector, Validation y Timeline.
3. Arrastrar splitters, cerrar/reabrir paneles, entrar a Play y volver.

Senal esperada:

- Paneles desaparecen o se superponen en tamanos bajos.
- El browser domina el ancho por tarjetas/miniaturas.
- El estado de splitters se guarda, pero no hay contrato de docking por panel.

Direccion:

Migrar a `WorkspaceLayoutTree`: paneles con min/preferred/max, prioridad de
colapso, overflow por tabs y virtualizacion de grids. Cada panel debe declarar
su contrato, no negociar implicitamente en Workbench.

## Hallazgo G - export ejecutable y capacidades no estan cerrados como producto

Evidencia local:

- Existen bundle/export, capability report y packaging.
- Tambien existen rutas de `ExtCall`, audio, transitions y runtime artifact.
- La API necesita un plan unico para GUI, CLI y Python.

Bug compuesto a forzar:

1. Crear authoring con `ExtCall`, audio fade, transition y asset importado.
2. Empaquetar desde GUI, CLI y Python.
3. Ejecutar sin runtime artifact, luego con artifact Windows.
4. Comparar warnings/capabilities y salida del bundle.

Senal esperada:

- Export puede generar bundle valido, pero no necesariamente un producto
  ejecutable autocontenido con reporte de capacidades uniforme.
- GUI/Python/CLI pueden explicar distinto las limitaciones.

Direccion:

Crear `ExportPlan`: entradas, runtime artifact, politica de proteccion,
capabilities, warnings, launcher y hashes. GUI/Python/CLI solo serializan y
ejecutan el plan.

## Hallazgo H - fragments/subgrafos necesitan stress de composicion

Evidencia local:

- Core tiene fragments, ports y `SubgraphCall`.
- CLI/Python exponen partes del modelo.
- El GUI ya tiene tests, pero el caso dificil es la composicion repetida con
  entradas/salidas multiples.

Bug compuesto a forzar:

1. Crear fragment A con dos salidas, fragment B que llama A, y dos llamadas a A
   desde ramas diferentes.
2. Renombrar puertos, borrar un nodo interno y refrescar ports.
3. Exportar strict y validar reachability/cycles.

Senal esperada:

- Labels internos pueden crecer o colisionar si el namespace no incluye call
  instance de forma consistente.
- Stale ports pueden quedar visibles pero no bloqueantes en algun cliente.

Direccion:

Tratar fragments como modulos compile-time: ownership unico, interface hash,
namespace determinista por call node y diagnosticos cross-fragment con ruta
causal completa.

## Hallazgo I - audio preview y metadata estan separados del modelo de assets

Evidencia local:

- `asset_browser.rs` calcula duracion y offset.
- `player_audio_ops.rs` resuelve previews.
- `runtime/src/audio.rs` cachea bytes.
- `workbench/audio_preview_store.rs` crea otro acceso a assets.

Bug compuesto a forzar:

1. Importar un audio grande, abrir browser, mover slider de offset, previsualizar
   BGM/SFX/Voice y luego Play.
2. Reemplazar el archivo por otro con misma ruta y distinta duracion.
3. Repetir preview desde inspector y browser.

Senal esperada:

- Duracion cacheada puede quedar vieja.
- Decode/carga se repite en rutas distintas.
- Preview y runtime pueden resolver distinto paths absolutos/relativos.

Direccion:

Unificar `AudioAssetService`: metadata cache por fingerprint de archivo,
streaming para bytes grandes, handles por canal y eventos de invalidacion.

## Hallazgo J - paridad Python/CLI/GUI aun necesita contrato de fixture comun

Evidencia local:

- Python ya expone `NodeGraph`, report v2, Composer y fragments.
- CLI tiene comandos authoring.
- Muchas pruebas construyen fixtures ad hoc por crate.

Bug compuesto a forzar:

1. Crear un `.vnauthoring` con scene layers, choice largo, fragment, ExtCall y
   asset faltante.
2. Validar desde CLI, Python y GUI.
3. Exportar reporte v2, generar repro, aplicar cambio de layout y reimportar.

Senal esperada:

- JSON puede ser compatible, pero los mensajes, stale status, capabilities o
  operation ids pueden diferir.

Direccion:

Crear `tests/fixtures/authoring_contract/` con documentos dorados y helpers
compartidos. Cada cliente debe probar contra los mismos JSON, no contra copias
locales simplificadas.

## Componentes candidatos a fusionar

| Grupo | Componentes actuales | Fusion propuesta |
| --- | --- | --- |
| Assets visuales | `AssetManager`, `AssetBrowserPanel`, `SceneStagePainter`, caches del Workbench | `EditorResourceService` con handles, LRU e invalidacion por fingerprint |
| Audio | browser duration cache, preview store, runtime audio cache | `AudioAssetService` con metadata, stream/cache y canales |
| Presentacion | `ComposerSnapshot`, `SceneState`, `VisualState`, Player UI | `PresentationSnapshot` unico en core |
| Grafo | core `NodeGraph`, GUI wrapper, adapter clones | core graph + `GraphViewState` GUI |
| Operaciones | undo snapshots, operation log, UI actions | `AuthoringCommandBus` tipado |
| Reportes | GUI import, CLI report, Python report wrappers | serializer/deserializer core + fixtures compartidos |
| Export | bundle, package, executable, capability warnings | `ExportPlan` headless consumido por GUI/CLI/Python |

## Mejoras de memoria y procesos

1. Evitar clones completos de grafo en cada mutacion; usar deltas y checkpoints.
2. No llamar `to_script()` para calcular fingerprint de cambios solo visuales.
3. Separar cache de bytes, imagen decodificada y textura GPU.
4. Decodificar imagen/audio fuera del hilo UI y publicar handles listos.
5. Incluir budget por cache y telemetria: bytes, entradas, hits, misses,
   evictions, decode_ms y upload_ms.
6. Internar strings repetidas de labels, paths y speaker names en authoring.
7. Usar fingerprints por dominio para invalidar solo lo necesario.
8. En Python, devolver wrappers/JSON bajo demanda en vez de clonar listas grandes
   para cada llamada.

## Pruebas nuevas recomendadas

No deben ser tests de relleno. Deben forzar interaccion entre subsistemas:

- `asset_cache_multiview_stress`: mismo background en browser, composer y player
  no debe decodificarse tres veces.
- `presentation_snapshot_parity`: Composer y Play renderizan el mismo snapshot
  para scene/patch/dialogue/choice/transition.
- `undo_delta_memory_contract`: 1000 nodos + 50 movimientos no debe multiplicar
  memoria por 50 snapshots completos.
- `evidence_trace_asset_resolver_chain`: asset faltante debe explicar operacion,
  field path, resolver lookup, candidatos y fix.
- `fragment_nested_namespace_stress`: llamadas repetidas a fragments no colisionan
  labels internos.
- `audio_metadata_invalidation`: reemplazar audio en misma ruta actualiza duracion
  y no usa cache vieja.
- `contract_fixture_cli_py_gui`: mismo fixture produce mismos codes, targets,
  fingerprints y stale status en CLI/Python/GUI.
- `window_layout_matrix`: paneles no se superponen en varias resoluciones y
  escalas.

## Riesgos priorizados

1. Stutter de UI por carga sincrona de assets: impacto alto, probabilidad alta.
2. Divergencia Composer/Play: impacto alto, probabilidad media-alta.
3. Memoria por undo/clones en grafos grandes: impacto medio-alto, probabilidad
   alta en proyectos reales.
4. Reportes explicables pero no causalmente verificables: impacto alto para QA.
5. Export ejecutable sin contrato unico de capabilities: impacto alto para
   producto final.

## Cierre

La arquitectura va en buena direccion: core ya tiene authoring, reportes v2,
fragments, composer headless y operation log. El problema actual es que el GUI
y algunos bindings todavia operan como clientes con caches, reconstrucciones y
atajos propios. El siguiente cierre no deberia ser "agregar mas botones"; debe
ser consolidar servicios: recursos, presentacion, operaciones, reportes y export.
