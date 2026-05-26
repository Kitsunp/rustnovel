# RustNovel unified audit

Fecha: 2026-05-26

Raiz auditada: `C:\Users\pelju\Downloads\rustnovel-main (8)\rustnovel-main`

Este documento unifica las cuatro auditorias ubicadas en `docs/audits/`:

- `rustnovel_traceability_repro_i18n_closure_audit_2026-04-29.md`
- `rustnovel_causal_visual_composer_closure_audit_2026-04-30.md`
- `rustnovel_forced_bug_architecture_audit_2026-05-18.md`
- `rustnovel_unfinished_duplicate_migration_flow_audit_2026-05-26.md`

No reemplaza el historial fuente; lo consolida en una sola auditoria operativa.

## Resultado ejecutivo

RustNovel avanzo de una arquitectura con muchos riesgos de trazabilidad, preview y mutaciones GUI hacia un core authoring mas fuerte: diagnosticos v2, fingerprints separados, reportes GUI/CLI/Python, operation logs, Visual Composer por capas, subgrafos, strict export y repro/dry-run mas explicables.

El cierre no esta completo como producto. Las cuatro auditorias convergen en un mismo problema: el core ya concentra bastante semantica, pero los bordes siguen duplicando decisiones o degradando silenciosamente. Los bordes mas riesgosos son Python packaging, CLI trace, ejemplos no validables, assets/audio repartidos en caches distintas, Composer/Player/runtime reconstruyendo escena por rutas separadas, undo basado en snapshots completos, export sin plan unico y fixtures no compartidos entre CLI/Python/GUI.

Estado global:

| Area | Estado integrado | Riesgo residual |
| --- | --- | --- |
| Diagnosticos/reportes | Implementado v2 con catalogo, ids versionados, target, field path, semantic values, docs refs y stale checks | Algunas evidencias pueden seguir siendo sinteticas si el resolver real no produce todos los atomos |
| Repro/dry-run | ExtCall simulado visible, repro con contexto diagnostico | Puede parecer ejecucion real si UI/docs no distinguen capability declarada vs callback ejecutado |
| Visual Composer | Capas, ownership por provenance, overlays, preview jugable, tests de parity parciales | Composer, Player y runtime aun reconstruyen escena por rutas distintas |
| Subgrafos/fragments | Modelo core/export y strict export implementados | Falta stress fuerte de composicion, UI contextual completa y namespace por call instance probado a escala |
| OperationLog | Mutaciones centrales auditadas y fingerprints before/after | Todavia no es el bus unico de mutacion; undo conserva snapshots completos |
| Python | Extension actual expone APIs y pasa con wheel local | `python -m pytest -q` puede importar paquete global obsoleto y fallar o falsear resultados |
| CLI | Authoring/report/package operativos | `trace` escribe YAML aunque la salida puede llamarse `.json` |
| Ejemplos/assets | Hay fixtures y assets | Ejemplos principales referencian asset inexistente y contienen duplicados reales |
| Dependencias | Tests/lint/audit pasan en el estado observado | `cargo tree -d` muestra versiones duplicadas; limpiar requiere convergencia controlada |

## Informacion externa complementaria

Fuentes consultadas para ajustar criterios de esta auditoria:

- Cargo Book, `cargo tree`: `cargo tree -d` muestra dependencias que aparecen en multiples versiones; esto respalda tratar la duplicacion de dependencias como trabajo de convergencia medible, no como limpieza manual. Fuente: https://doc.rust-lang.org/cargo/commands/cargo-tree.html
- Cargo Book, resolver: Cargo unifica versiones compatibles y puede resolver versiones incompatibles en paralelo; los duplicados son especialmente relevantes cuando tipos publicos cruzan limites de crate. Fuente: https://doc.rust-lang.org/nightly/cargo/reference/resolver.html
- Maturin User Guide: `maturin develop` esta pensado para construir e instalar rapidamente en un virtualenv; esto respalda que los tests Python deben usar entorno aislado o wheel local, no `site-packages` global. Fuente: https://www.maturin.rs/local_development
- Python Packaging User Guide: los virtualenvs aislan instalaciones por proyecto y evitan interferencia entre paquetes. Fuente: https://packaging.python.org/en/latest/guides/installing-using-pip-and-virtual-environments/
- Python virtual environment specification: un entorno virtual se detecta por `sys.prefix != sys.base_prefix`, util para healthchecks de CI. Fuente: https://packaging.python.org/en/latest/specifications/virtual-environments/
- RustSec `cargo audit`: audita dependencias contra la RustSec Advisory Database y permite ignorar advisories cuando hay justificacion. Fuente: https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md
- OWASP Path Traversal: las rutas de archivos controladas por input deben evitar traversal/absolutas y validarse por allowlist. Esto aplica directamente a assets, import/export y manifests. Fuente: https://owasp.org/www-community/attacks/Path_Traversal
- OWASP File Upload Cheat Sheet: recomienda limitar caracteres, evitar secuencias tipo traversal y definir permisos/limites de almacenamiento. Esto complementa la politica de assets y bundles. Fuente: https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html

Tambien se preservan referencias arquitectonicas de la auditoria del 2026-05-18: egui `TextureHandle`, Bevy `AssetServer`/`Handle`, Godot `ResourceLoader`, Ren'Py y Yarn Spinner como modelos de recursos, handles, authoring y prediccion de assets.

## Evolucion por auditoria fuente

### 2026-04-29: Traceability/Repro/I18n closure

Resultado: la auditoria de trazabilidad quedo cerrada para esa pasada de implementacion.

Puntos cerrados:

- Catalogo diagnostico ES/EN por codigo.
- `docs_ref` reales hacia `docs/diagnostics/authoring.md#...`.
- Reportes sin fingerprint importados como untrusted/stale.
- Fingerprint semantico separado de build metadata.
- Bloqueo de quick-fix en reportes stale/untrusted.
- Quick-fix audit con SHA-256 canonico y operation ids.
- ReproCase con diagnostic id, semantic fingerprint, operation id, capabilities, plugin list, asset manifest hash, seed y validation profile.
- Dry-run trata solo `EndOfScript` como cierre limpio; otros errores son diagnostico runtime.
- SceneProfile sin node id falso.
- Report export/import preserva envelopes y mensajes localizados.
- Diagnostic IDs versionados y con phase/code/node/event/edge/asset/context.
- ExtCall simulado reportado con `DRY_EXTCALL_SIMULATED`.
- CLI y Python exponen envelope v2 y localizacion.

Riesgos que pasaron a auditorias posteriores:

- Python en Windows podia usar `.pyd` viejo.
- `cargo audit` dependia de red/sandbox en esa ejecucion.
- SARIF/Fluent seguian como mejoras futuras.

### 2026-04-30: Causal diagnostics and Visual Composer closure

Resultado: se consolido diagnostico causal v2, fingerprints separados, operation log para mutaciones centrales, Visual Composer con capas WYSIWYG y primer modelo de subgrafos.

Puntos cerrados:

- Targets granulares, semantic values y evidence traces.
- IDs no colisionables por opcion/layer/asset/campo/evento.
- Reportes v2 compartidos por GUI/CLI/Python y lectura legacy v1.
- Stale semantico por `story_semantic_sha256`.
- VerificationRun con introducidos/resueltos.
- OperationLog con before/after fingerprint y field paths.
- Visual Composer 2 con `StageLayerKind`, `LayeredSceneObject`, overrides de visibilidad/lock/z-order y panel de capas.
- Stage con background, personajes, dialogo, choices, safe area, transition/debug.
- Drag sin cruce de ownership para personajes duplicados.
- Composer puede avanzar y elegir choices.
- Subgrafos authoring con fragments, ports, portal nodes, decision hubs y labels deterministas.
- `to_script_strict()` bloquea no exportables, drafts, targets rotos y placeholders.
- EndOfScript tipado en player UI.

Riesgos residuales:

- UI completa de subgrafos aun debe crecer.
- OperationLog no cubre todos los controles menores.
- Renderer runtime avanzado queda fuera.
- Clientes externos deben migrar a v2 para no perder target/evidence.

### 2026-05-18: Forced bug, duplication and architecture audit

Resultado: no fue una auditoria de cierre sino de presion. Forzo bugs compuestos entre subsistemas y encontro que el problema no era tamano de archivos, sino duplicacion de rutas.

Hallazgos arquitectonicos:

- Assets visuales repartidos entre `AssetManager`, Asset Browser, Scene Stage y Workbench.
- Audio repartido entre browser duration cache, preview store y runtime cache.
- Composer, Player y runtime reconstruyen escena con modelos distintos.
- GUI authoring adapter y undo hacen clones/snapshots completos.
- EvidenceTrace existe, pero algunos atomos pueden ser sinteticos si no nacen del resolver real.
- OperationLog cubre mucho, pero no es el bus unico.
- Layout responsive mejora, pero falta docking por contrato de panel.
- Export ejecutable y capabilities no estan cerrados como producto.
- Fragments necesitan stress de composicion/namespacing.
- Python/CLI/GUI necesitan fixtures comunes, no fixtures ad hoc por crate.

Fallo dinamico destacado:

```text
test_python_can_inspect_branch_hub_connections_and_positions
AssertionError: Items in the first set but not the second: 3
Items in the second set but not the first: 4
```

Interpretacion integrada: el comportamiento real de `connect_or_branch` parecia coherente con IDs 0-based, pero el test Python esperaba otro id. Eso revelo que el contrato publico Python de ids/posicion generada no estaba suficientemente congelado por fixtures compartidos.

### 2026-05-26: Unfinished, duplicate, migration and flow audit

Resultado: Rust principal estaba verde, pero los bordes seguian fallando.

Verificaciones observadas:

- `cargo test --workspace --all-targets --no-fail-fast`: paso.
- `cargo fmt --check`: paso.
- `cargo clippy --workspace --all-targets -- -D warnings`: paso.
- `cargo audit -D warnings --ignore RUSTSEC-2024-0436`: paso.
- `python -m pytest -q`: fallo 25/62 por paquete global obsoleto.
- Wheel local via maturin + `PYTHONPATH=target\audit-pythonpath;python`: paso 62/62.
- CLI trace escribio YAML en archivo `.json`.
- `authoring validate` de ejemplos fallo por assets faltantes.
- `manifest examples\scripts\assets` mostro PNGs duplicados por hash.

Hallazgos nuevos:

- Python actual esta migrado en codigo, pero el flujo por defecto puede importar una instalacion global vieja.
- `trace` serializa con `serde_norway` y no declara formato.
- `examples/scripts/*.json` referencian `bg/classroom.png`, inexistente en el proyecto.
- `demo_story.json` contiene label fuera de rango `node_23: 5` con 5 eventos; `validate` no lo denuncia.
- `ExtCall` es runtime-real bajo callback, pero dry-run/repro lo simulan.
- `StoryNode` esta migrado al core; `NodeGraph` GUI es wrapper necesario, no basura directa.
- Hay duplicados de dependencias y datos, pero no deben eliminarse sin pruebas.

## Hallazgos consolidados

### P0 - Python packaging y CI local no pueden depender del ambiente global

El codigo actual de `crates/py` registra las APIs esperadas, pero `python -m pytest -q` puede importar `visual_novel_engine` desde `site-packages` global. Esto reproduce fallos que no corresponden al codigo actual.

Decision:

- Tests Python deben construir e instalar la extension local en virtualenv o path aislado.
- El test runner debe fallar temprano si `visual_novel_engine.__file__` apunta fuera del workspace/venv esperado.
- El healthcheck debe imprimir `sys.prefix`, `sys.base_prefix`, `visual_novel_engine.__file__` y public API names.

Base externa:

- PyPA recomienda virtualenvs para aislar paquetes por proyecto.
- Maturin documenta `maturin develop` para instalar rapido en virtualenv.

Checks objetivo:

```powershell
py -m venv target\py-audit-venv
target\py-audit-venv\Scripts\python -m pip install --upgrade pip
target\py-audit-venv\Scripts\python -m pip install maturin
target\py-audit-venv\Scripts\maturin develop --manifest-path crates\py\Cargo.toml --features extension-module
target\py-audit-venv\Scripts\python -m pytest -q
```

### P1 - CLI trace debe tener contrato de formato

`vnengine trace --output target\audit-cli-trace.json` escribe YAML. El comando no falla, pero el archivo no es JSON parseable.

Decision:

- Agregar `--format json|yaml`.
- Default recomendado: `json`, por coherencia con reportes authoring/repro/manifest.
- Si se conserva YAML como default por compatibilidad, validar extension o emitir warning fuerte.

Checks objetivo:

```powershell
cargo run -p vnengine_cli --bin vnengine -- trace examples\scripts\script.json --format json --output target\trace.json
python -m json.tool target\trace.json
cargo run -p vnengine_cli --bin vnengine -- trace examples\scripts\script.json --format yaml --output target\trace.yaml
```

### P1 - Ejemplos deben pasar runtime y authoring validation

Los ejemplos pasan runtime `validate`, pero authoring detecta assets faltantes. Tambien hay labels fuera de rango no denunciadas.

Decision:

- El entrypoint de ejemplo debe pasar `validate` y `authoring validate`.
- Corregir `bg/classroom.png` hacia un asset existente o agregar el asset esperado.
- Definir si labels no referenciadas fuera de rango son error, warning o metadata tolerada.
- Incluir estos comandos en CI smoke.

Checks objetivo:

```powershell
cargo run -p vnengine_cli --bin vnengine -- validate examples\scripts\demo_story.json
cargo run -p vnengine_cli --bin vnengine -- authoring validate examples\scripts\demo_story.json --project-root examples\scripts --output target\example-authoring-report.json
```

### P1 - Separar simulacion, preview y ejecucion real

`ExtCall` puede ser runtime real con callback autorizado, pero dry-run/repro lo simulan. Import Ren'Py degrada unsupported a `ExtCall`/Generic con trace. Scene preview puede pintar fallbacks.

Decision:

- Cada modo debe declarar fidelity: `runtime_real`, `headless_simulated`, `preview_only`, `fallback_degraded`.
- Todo reporte dry-run/repro con `ExtCall` debe exponer `external_call_simulated`.
- Import GUI debe mostrar conteo de degradaciones y ofrecer strict mode.

Checks objetivo:

- Tests que fallen si desaparece `DRY_EXTCALL_SIMULATED`.
- Test Python de callback real autorizado.
- Test GUI/CLI de import strict que falle ante fallback.

### P1 - Unificar presentacion para evitar divergencia Composer/Player/runtime

Las auditorias coinciden en que Composer, Player y runtime siguen reconstruyendo escena desde rutas distintas.

Decision:

- Crear `PresentationSnapshot` en core con visual state, overlays, choices/dialogue, transition, layer objects, provenance, safe area y layout calculado.
- GUI Composer, Player UI y runtime preview deben consumir ese snapshot.
- El snapshot debe tener fixtures dorados compartidos.

Checks objetivo:

- `presentation_snapshot_parity`: misma entrada produce mismo snapshot en Composer y Play.
- Tests con personajes duplicados por nombre y poses distintas.
- Tests con choices largos y transiciones.

### P1 - Resources deben centralizarse: visual, audio y asset metadata

Las auditorias detectan caches separadas:

- `AssetManager`
- Asset Browser `image_cache`, `image_failures`, `audio_duration_cache`
- Scene Stage `AssetStore`
- Workbench `composer_image_cache`, `composer_image_failures`, `audio_duration_cache`
- runtime audio cache
- audio preview store

Decision:

- Crear `EditorResourceService`.
- Separar cache de bytes, imagen decodificada, textura GPU, metadata audio y handles.
- Invalidar por fingerprint de archivo, no por path solamente.
- Decodificar fuera del hilo UI.
- Exponer metricas: bytes, hits, misses, evictions, decode_ms, upload_ms.

Checks objetivo:

- `asset_cache_multiview_stress`: browser/composer/player no decodifican tres veces el mismo background.
- `audio_metadata_invalidation`: reemplazar audio en misma ruta actualiza duracion.

### P2 - OperationLog debe evolucionar a CommandBus

OperationLog ya existe, pero no es aun la unica ruta de mutacion. Undo usa snapshots completos y algunas acciones dependen de estado GUI.

Decision:

- Introducir `AuthoringCommandBus`.
- Cada mutacion produce comando, delta, operation log entry, invalidacion por dominio y verification run.
- Undo/redo debe operar por deltas con checkpoints, no por snapshots completos en cada accion.

Checks objetivo:

- Reproducir log headless.
- `undo_delta_memory_contract` con 1000 nodos y movimientos repetidos.
- Test de replay para crear nodo, conectar, editar, importar asset, mover layer y revertir.

### P2 - EvidenceTrace debe nacer de resolvers reales

Los envelopes v2 existen, pero algunas trazas pueden ser armadas al final. Para diagnosticos criticos, el resolver debe producir atomos reales.

Decision:

- Asset resolver emite atomos: input raw, normalized, canonical path, candidates, result, failure.
- Validation rule consume esos atomos y agrega consecuencia/fix.
- CLI `authoring explain` y Python deben mostrar la misma cadena causal.

Checks objetivo:

- `evidence_trace_asset_resolver_chain`: operacion -> field -> resolver -> candidatos -> regla -> fallo -> fix.
- Reporte importado mantiene trace y stale status.

### P2 - Export debe ser un `ExportPlan`

Bundle/export, package, executable y capability warnings existen, pero GUI/CLI/Python pueden divergir.

Decision:

- Crear `ExportPlan` headless en core.
- Entradas: project root, authoring/runtime script, runtime artifact, target platform, integrity/protection, capability policy.
- Salidas: layout, hashes, executable, warnings, capabilities, missing runtime/artifact errors.
- GUI/CLI/Python solo serializan/ejecutan el plan.

Checks objetivo:

- Mismo fixture exportado por GUI/CLI/Python genera mismo plan.
- Test con `ExtCall`, audio fade, transition y missing runtime artifact.

### P2 - Fragments/subgrafos necesitan stress de composicion

El modelo existe, pero los casos dificiles son nested calls, multiple exits, stale ports y namespaces.

Decision:

- Interface hash por fragment.
- Namespace determinista por call node.
- Diagnosticos cross-fragment con ruta causal.
- UI contextual completa para agrupar/desagrupar/entrar/salir.

Checks objetivo:

- `fragment_nested_namespace_stress`.
- Dos llamadas al mismo fragment no colisionan labels internos.
- Borrar nodo interno invalida ports y bloquea strict export.

### P2 - Dependencias duplicadas: convergencia controlada, no limpieza ciega

`cargo tree -d` confirma duplicados como `image 0.24/0.25`, `toml 0.8/0.9`, `windows 0.54/0.62`, `thiserror 1/2` y familias `windows-sys`.

Decision:

- Priorizar duplicados directos controlados (`image`, `toml`).
- No forzar `windows` mientras rodio/cpal/wgpu tengan constraints incompatibles.
- Medir antes/despues con `cargo tree -d`.

Base externa:

- Cargo resolver puede mantener multiples versiones incompatibles.
- `cargo tree -d` es el comando oficial para localizar duplicados por version.

Checks objetivo:

```powershell
cargo tree -d
cargo test --workspace --all-targets --no-fail-fast
```

### P2 - Seguridad de paths y assets debe mantenerse como politica estricta

El repo ya tiene tests de traversal/symlink en assets/export/import. OWASP refuerza que rutas controladas por input son superficie de ataque.

Decision:

- Mantener allowlist y canonicalizacion.
- No aceptar paths absolutos o traversal en manifests, imports, export bundles o locale files.
- Para assets importados, validar extension, tamano, fingerprint y ubicacion bajo project root.

Checks objetivo:

- Tests actuales de symlink/path traversal deben seguir siendo gating.
- Agregar fixtures con `%2e%2e`, backslashes Windows y rutas UNC si aplican.

### P3 - Codigo basura potencial: borrar solo con requisitos duros

Candidatos:

- `allow(unused_assignments)` en crates base.
- `allow(dead_code)` en error/validator/inspector.
- `allow(unused_imports)` para `SkipMode`.
- `_content_unused` en save.
- `_type_anchor`.
- Reexports Python/Rust y adapters.

Decision:

No eliminar por grep simple. Muchos pueden ser compatibilidad, API publica, soporte de tests externos o serializacion legacy.

Criterio de borrado:

1. Sin referencias en codigo, tests, docs, ejemplos y scripts.
2. No forma parte de API publica Rust/Python/CLI.
3. No participa en serde de documentos antiguos.
4. No es fallback/migracion.
5. Tiene test de flujo que antes podia usarlo.
6. Pasa `cargo test`, `clippy`, pytest relevante y smoke CLI.

## Arquitectura objetivo

| Dominio | Estado actual | Objetivo |
| --- | --- | --- |
| Recursos visuales | Caches en browser/stage/workbench/runtime | `EditorResourceService` con handles, LRU e invalidacion por fingerprint |
| Audio | Duration cache, preview store y runtime cache separados | `AudioAssetService` con metadata cache, streaming y canales |
| Presentacion | ComposerSnapshot, SceneState, VisualState y Player UI separados | `PresentationSnapshot` unico en core |
| Grafo | Core graph + GUI wrapper + clones/adapters | Core graph + `GraphViewState`; GUI solo view/interaccion |
| Mutaciones | OperationLog + undo snapshots + UI actions | `AuthoringCommandBus` con deltas, verification runs y replay headless |
| Reportes | v2 estable pero evidencia parcialmente sintetica | Resolver-driven evidence traces y fixtures comunes |
| Export | Bundle/package/capabilities por rutas | `ExportPlan` headless consumido por GUI/CLI/Python |
| Fixtures | Pruebas ad hoc por crate | `tests/fixtures/authoring_contract/` compartido por CLI/Python/GUI |

## Roadmap priorizado

### Fase 1 - Bordes que rompen uso real

1. Aislar Python tests con maturin/venv y healthcheck de modulo nativo.
2. Agregar contrato de formato para `vnengine trace`.
3. Arreglar ejemplos para que pasen `validate` y `authoring validate`.
4. Decidir contrato de labels fuera de rango.

Salida esperada: usuarios y CI ejecutan los mismos artefactos que el repo construye.

### Fase 2 - Fidelidad de preview/runtime

1. Crear `PresentationSnapshot`.
2. Hacer que Composer y Player consuman snapshot comun.
3. Mantener `ExtCall` simulado visible por modo.
4. Agregar fixture con scene/patch/dialogue/choice/transition/personajes duplicados.

Salida esperada: preview deja de ser reconstruccion paralela.

### Fase 3 - Recursos y performance

1. Crear `EditorResourceService`.
2. Crear `AudioAssetService`.
3. Unificar invalidacion por fingerprint.
4. Agregar metricas de cache y tests stress.

Salida esperada: menos stutter, menos decodificacion duplicada, invalidacion correcta.

### Fase 4 - Mutaciones, undo y evidencia causal

1. Introducir `AuthoringCommandBus`.
2. Convertir undo/redo a deltas con checkpoints.
3. Hacer resolver-driven evidence traces.
4. Reproducir operation logs headless.

Salida esperada: QA puede explicar y reproducir cambios sin depender de estado GUI.

### Fase 5 - Export/product hardening

1. Crear `ExportPlan`.
2. Homologar GUI/CLI/Python.
3. Convergencia controlada de dependencias.
4. Limpieza estricta de codigo/datos basura.

Salida esperada: producto empaquetable y auditable de forma uniforme.

## Matriz de pruebas integrada

| Test recomendado | Cubre | Estado |
| --- | --- | --- |
| `python_native_module_origin_healthcheck` | Evitar site-packages obsoleto | Nuevo |
| `cli_trace_json_contract` | Salida JSON real para `.json` | Nuevo |
| `example_entrypoint_authoring_validate` | Usabilidad de ejemplos | Nuevo |
| `label_out_of_range_contract` | Labels huerfanas/fuera de rango | Nuevo |
| `presentation_snapshot_parity` | Composer vs Play/runtime | Nuevo |
| `asset_cache_multiview_stress` | Cache visual compartida | Nuevo |
| `audio_metadata_invalidation` | Cache audio por fingerprint | Nuevo |
| `undo_delta_memory_contract` | Memoria en grafos grandes | Nuevo |
| `evidence_trace_asset_resolver_chain` | Causalidad real | Nuevo |
| `fragment_nested_namespace_stress` | Subgrafos complejos | Nuevo |
| `export_plan_cli_py_gui_parity` | Export uniforme | Nuevo |
| `contract_fixture_cli_py_gui` | Misma semantica en clientes | Nuevo |
| `dependency_convergence_snapshot` | `cargo tree -d` antes/despues | Nuevo |

## Comandos de aceptacion

Rust:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets --no-fail-fast
cargo audit -D warnings --ignore RUSTSEC-2024-0436
```

Python aislado:

```powershell
py -m venv target\py-audit-venv
target\py-audit-venv\Scripts\python -m pip install --upgrade pip
target\py-audit-venv\Scripts\python -m pip install maturin pytest
target\py-audit-venv\Scripts\maturin develop --manifest-path crates\py\Cargo.toml --features extension-module
target\py-audit-venv\Scripts\python -m pytest -q
```

CLI/examples:

```powershell
cargo run -p vnengine_cli --bin vnengine -- validate examples\scripts\demo_story.json
cargo run -p vnengine_cli --bin vnengine -- authoring validate examples\scripts\demo_story.json --project-root examples\scripts --output target\example-authoring-report.json
cargo run -p vnengine_cli --bin vnengine -- trace examples\scripts\script.json --format json --output target\trace.json
python -m json.tool target\trace.json
```

Dependencias:

```powershell
cargo tree -d
```

Seguridad de paths/assets:

```powershell
cargo test --workspace --all-targets --no-fail-fast symlink
cargo test --workspace --all-targets --no-fail-fast traversal
```

## Decisiones de producto pendientes

1. `trace` default: cambiar a JSON o mantener YAML con `--format`.
2. Labels fuera de rango: error estricto, warning authoring o metadata tolerada.
3. Ren'Py import default: seguir degradando por defecto o pedir confirmacion en GUI.
4. ExtCall en export: permitir capability declarada sin runtime plugin o bloquear strict export.
5. Ejemplos con assets duplicados: fixtures intencionales de dedupe o basura a limpiar.
6. Python compatibility fallback: conservar para modulos viejos o exigir nativo actual.
7. Docking/layout: sistema propio de contratos de panel o adopcion de layout tree.

## Conclusiones integradas

La direccion tecnica es correcta: mover semantica al core, hacer reportes versionados, separar fingerprints, registrar operaciones y exponer contratos por CLI/Python/GUI. Las auditorias anteriores no se contradicen; juntas muestran una frontera clara entre "cerrado a nivel de modulo" y "cerrado como producto".

El siguiente trabajo no deberia concentrarse en agregar mas UI aislada. Debe cerrar los bordes donde hoy hay duplicacion o simulacion ambigua: Python packaging, formato CLI, ejemplos, recursos compartidos, snapshot de presentacion, command bus, evidencia causal real y export plan.

La limpieza de codigo o assets debe ser la ultima fase. Hay basura probable, pero tambien compatibilidad, fixtures y adapters que sostienen flujos indirectos. La regla operativa es simple: primero fixtures y pruebas compartidas; despues convergencia; al final eliminacion.

## Auditorias fuente completas

Esta seccion no resume ni sustituye los informes anteriores. Conserva el contenido operativo de las cuatro auditorias que estaban separadas para que el archivo unificado no pierda evidencia, comandos, riesgos ni decisiones pendientes.

## Fuente 1 - RustNovel Traceability/Repro/I18n Closure Audit

Date: 2026-04-29

Scope: closure of `rustnovel_auditoria_trazabilidad_repro_i18n.md`, focused on diagnostics, reports, fingerprints, dry-run/repro, quick-fix audit, CLI, Python bindings, and local verification.

### Executive Result

The traceability audit is closed for the requested implementation pass. Diagnostics now have a structured catalog, real docs references, versioned diagnostic IDs, envelopes shared by core/GUI/CLI/Python, semantic fingerprints separated from build metadata, stronger imported-report trust handling, SHA-256 quick-fix audit hashes, repro diagnostic context, and visible ExtCall simulation warnings.

The main extra bug found during closure was in the local Python CI path on Windows: `maturin develop` could report success while the old installed `.pyd` stayed in `site-packages`, causing new binding tests to skip critical APIs. `scripts/ci-local.ps1` now runs Python tests against the freshly built local artifact first on Windows.

### Audit Points

| ID | Status | Closure |
|---|---|---|
| T-01 generic diagnostic explanations | Closed | `DiagnosticCatalog` provides per-code ES/EN title, what, root cause, why, consequence, fixes, expected/actual, and action steps. |
| T-02 broken docs_ref | Closed | Diagnostics point to `docs/diagnostics/authoring.md#...`; tests verify referenced anchors exist. |
| T-03 missing fingerprint not stale | Closed | GUI imports reports without fingerprints as untrusted/stale and blocks fixes while keeping issues readable. |
| T-04 fingerprint mixed with environment | Closed | Semantic fingerprint is separate from build profile/OS/arch; stale checks compare semantic data. |
| T-05 selected fix on stale report | Closed | Central GUI guard blocks selected, automatic, and batch fixes for stale/untrusted reports. |
| T-06 unstable quick-fix hash | Closed | Quick-fix audit uses SHA-256 over canonical authoring documents and records operation ids. |
| T-07 ReproCase lacks diagnostic/project refs | Closed | Repro cases include diagnostic id, semantic fingerprint, operation id, capabilities, plugin list, asset manifest hash, seed, and validation profile. |
| T-08 dry-run hides current_event errors | Closed | Dry-run treats only `EndOfScript` as clean finish; other errors become runtime diagnostics. |
| T-09 SceneProfile node_id=0 | Closed | SceneProfile asset validation no longer emits a fake node id. |
| T-10 imported report loses explanation | Closed | Report export/import preserves diagnostic envelope fields and localized actual messages. |
| T-11 diagnostic_id collision risk | Closed | Diagnostic ids are versioned and include phase, code, node, event ip, edge, asset, and blocked-flow context. |
| T-12 ExtCall simulation only note | Closed | Dry-run emits `DRY_EXTCALL_SIMULATED` warning plus simulation step metadata. |
| T-13 CLI report poorer than GUI | Closed | CLI uses core `AuthoringValidationReport` with `DiagnosticEnvelopeV2`. |
| T-14 Python generic/no dynamic locale | Closed | Python exposes envelope data and `localized(locale)`. |
| T-15 diagnostic localization separate/hard-coded | Closed | Diagnostic catalog is separate from narrative localization and uses stable message keys. |

### Tests Added Or Strengthened

- Core traceability tests cover catalog specificity, docs references, ExtCall simulation diagnostics, and verification-run introduced/resolved diagnostics.
- GUI report tests cover untrusted missing-fingerprint imports, semantic fingerprint comparison across build metadata, and stale report fix blocking.
- GUI diagnostic report tests now assert the v2 diagnostic id contract.
- CLI tests assert enriched authoring report envelopes and authoring-aware commands.
- Python tests now discover all `tests/python/test_*.py` files, exercise NodeGraph/StoryNode bindings, localized diagnostic envelopes, authoring save/load, JumpIf two-port roundtrip, and project-root validation.
- Repro tests assert diagnostic context, capabilities, seed, validation profile, plugins, and asset-manifest hash.

### Extra Fixes Found During Closure

- `scripts/ci-local.ps1` now discovers all Python tests instead of only two modules.
- Windows local Python tests prefer the freshly built `target/debug/visual_novel_engine.pyd`, avoiding stale global installs.
- Python binding tests now write temporary files under `target/python-test-tmp`, keeping generated test artifacts ignored by Git and inside the workspace.
- `scripts/ci-local.ps1` moved the cargo-audit database to `target/ci-local/audit-db` to avoid inherited Git ownership issues.

### Verification

Passed:

- `ruff format --check .`
- `ruff check .`
- `cargo fmt --check`
- `cargo check --workspace --all-targets --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked --verbose`
- `cargo test -p visual_novel_engine --features arbitrary --test fuzz_tests --locked --verbose`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job python-tests`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job matrix-smoke`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job sbom-policy -SkipToolInstall`
- `powershell -ExecutionPolicy Bypass -File scripts/ci-local.ps1 -Job reproducible-smoke`
- `cargo build -p vnengine_py --profile python --features extension-module --locked --verbose`
- `cargo bench -p visual_novel_engine --bench core_benches --locked -- --warm-up-time 0.1 --measurement-time 0.1 --sample-size 10`
- Source file size audit: no checked source file over 500 lines.

Not fully verified locally:

- `cargo audit` reached the correct RustSec fetch path after the cache fix, but network access was blocked by the sandbox. Escalation was requested and rejected by the environment usage limit, so this specific online fetch could not be completed locally.

### Residual Risks

- `cargo bench` completed successfully, but some smoke measurements reported small regressions versus local prior baselines. The bench job is non-gating and noisy with 10 samples; no functional failure was observed.
- Full SARIF export and Fluent/FTL migration remain future enhancements. The current implementation keeps stable envelopes and message keys so those can be added without changing existing diagnostic semantics.

## Fuente 2 - RustNovel - Cierre auditoria causal y Visual Composer

Fecha: 2026-04-30

Alcance: cierre implementado sobre la auditoria causal/Visual Composer. El objetivo fue consolidar identidad diagnostica v2, trazabilidad causal, fingerprints separados, operation log real para mutaciones del editor, Visual Composer con capas WYSIWYG y un primer modelo determinista de subgrafos en authoring.

### Estado por punto

| Punto original | Estado final | Subsistemas/archivos | Tests |
| --- | --- | --- | --- |
| Core diagnostics con targets granulares, valores semanticos y evidencia | Implementado | `crates/core/src/authoring/diagnostics.rs`, `lint.rs`, `validation.rs`, `validation/assets.rs`, `validation/scene.rs` | `granular_targets_make_same_node_choice_diagnostics_distinct`, `evidence_trace_explains_asset_jump_and_generic_failures` |
| IDs no colisionables por opcion/layer/asset/campo/evento | Implementado para diagnosticos authoring actuales | `lint.rs`, `diagnostics.rs`, reglas de validacion | `granular_targets_make_same_node_choice_diagnostics_distinct` |
| `message_args` tipado y textos desde evidencia | Implementado como payload tipado por target/field/semantic/evidence; catalogo ES/EN/dev se mantiene centralizado | `diagnostics.rs`, `diagnostics/catalog.rs` | tests de catalogo y envelope v2 |
| Reportes GUI/CLI/Python v2 y lectura legacy v1 | Implementado; salida nueva usa `vnengine.authoring_validation_report.v2`, GUI lee v1/v2 legacy | `validation_report.rs`, `report_ops.rs`, `tools/cli`, `crates/py` | `workbench_diagnostic_report_json_contains_bilingual_fields`, `report_v2_import_preserves_target_field_path_and_stale_state`, CLI authoring tests, Python binding test |
| Fingerprints separados | Implementado: story semantic, layout, assets y full document | `report_fingerprint.rs`, `operation_log.rs`, `report_ops.rs` | `fingerprints_split_story_layout_assets_and_document_hashes` |
| Stale semantico basado en `story_semantic_sha256` | Implementado | `report_fingerprint.rs`, GUI report import | `report_v2_import_preserves_target_field_path_and_stale_state` |
| VerificationRun/Repro diagnostic ids no colisionables | Implementado para `VerificationRun`; repro mantiene diagnostico estable desde la capa existente | `operation_log.rs`, report ops | `verification_run_tracks_resolved_and_introduced_diagnostics` |
| OperationLog para mutaciones del editor | Implementado para mutaciones centrales observables: quick-fix, cambios del grafo, drag/composer, revert/verification path existente | `workbench.rs`, `workbench/ui.rs`, `quick_fix_ops.rs`, `report_ops.rs` | `editor_mutation_operation_log_records_before_after_fingerprints` |
| OperationLog con before/after fingerprint, field paths y valores | Implementado con compatibilidad de lectura de entradas viejas | `operation_log.rs`, GUI workbench | core/gui operation tests |
| Visual Composer 2 con capas | Implementado: `StageLayerKind`, `LayeredSceneObject`, overrides de visibilidad/lock/z-order y panel de capas | `visual_composer.rs`, `scene_stage.rs`, `workbench.rs` | `layered_scene_objects_include_runtime_overlays_and_source_paths`, composer tests |
| Stage WYSIWYG con background, personajes, dialogo, choices, safe area, transition/debug | Implementado en el preview/editor con overlays de dialogo/choice/transicion y stage painter compartido | `visual_composer.rs`, `scene_stage.rs`, player UI | composer tests |
| Drag sin cruce de ownership para personajes duplicados | Implementado por provenance de entidad/campo, no por nombre de speaker | `composer_ops.rs`, `visual_composer.rs` | `composer_owner_map_keeps_duplicate_character_instances_separate` |
| Composer puede avanzar y elegir choices como Play | Implementado en overlay del Composer; los controles de edicion siguen separados | `visual_composer.rs`, tests existentes de runtime preview | `composer_runtime_preview_can_start_from_selected_node_and_advance` |
| Subgrafos authoring | Implementado primer modelo headless: `GraphFragment`, `FragmentPort`, `PortalNode`, `DecisionHub`, `GraphStack`; export genera labels deterministas sin colision | `graph.rs`, `script_sync.rs`, `report_fingerprint.rs` | `graph_fragments_are_stable_authoring_metadata` |
| Agrupar/desagrupar sin perder conexiones externas | Implementado como metadata core con puertos de entrada/salida detectados; UI contextual queda como riesgo residual | `graph.rs` | fragment test |
| Reachability/cycles a traves de fragmentos | Cubierto como flattening authoring determinista: el runtime/analisis sigue usando el grafo plano con metadata de fragmento | `graph.rs`, `script_sync.rs` | fragment/reachability test |
| `to_script_strict()` bloquea no exportables, drafts, targets rotos, placeholders | Implementado | `script_sync.rs`, validation | `strict_export_blocks_unreachable_drafts_and_generic_payloads` |
| EndOfScript tipado | Implementado en UI de player: no se detecta por texto | `player_ui/render.rs` | `end_ui_uses_typed_end_of_script_error` |
| Capability limitada para ExtCall/audio/transitions simulados | Documentado y emitido para ExtCall dry-run; audio/transitions dependen del backend y conservan contrato de capability | `compiler/dry_run.rs`, diagnostics catalog/docs | `dry_run_reports_extcall_as_simulated_capability` |

### Bugs adicionales encontrados y corregidos

- El test del Composer usaba la firma vieja de `Engine::new`; se corrigio para pasar `SecurityPolicy` y `ResourceLimiter`.
- El `OperationLogEntry` no era tolerantemente deserializable si faltaban los campos nuevos; se agregaron defaults serde.
- El reporte GUI conservaba un bloque JSON legacy comentado dentro del flujo nuevo; se elimino para reducir ambiguedad.
- La validacion de assets armaba evidencia por mutacion posterior del ultimo issue; se dejo como construccion directa del issue.
- Los fragmentos inicialmente eran solo metadata. Se amplio `create_fragment` para calcular puertos externos y `to_script` para emitir labels fragmentados deterministas.

### Placeholders y capabilities limitadas

| Marcador | Estado | Evidencia |
| --- | --- | --- |
| ExtCall simulado en dry-run | Capability limitada documentada con diagnostic/test | `DRY_EXTCALL_SIMULATED`, `dry_run_reports_extcall_as_simulated_capability` |
| Opciones `Option N` | Placeholder bloqueante en strict export | `VAL_CHOICE_PLACEHOLDER`, strict export test |
| Audio capabilities por backend | Capability limitada documentada por contrato runtime previo; no se simula como exito silencioso en diagnostics nuevos | audio capability tests previos |
| Transitions | Estado observable en runtime/preview previo; Composer muestra overlay de transicion | composer/runtime tests |
| Subgrafos GUI contextual | Riesgo residual: modelo core/export listo, falta menu contextual completo para agrupar/desagrupar desde GUI | fragment test cubre core |

### Riesgos residuales

- La UI completa de subgrafos todavia debe crecer sobre el modelo core: menu contextual, doble click para entrar/salir y comandos visuales de desagrupar.
- OperationLog ya cubre mutaciones centrales del editor, pero algunas ediciones menores de controles especificos pueden requerir field paths mas finos si se agregan nuevos paneles.
- Visual Composer comparte el modelo de escena/player y soporta overlays jugables, pero WebGPU/runtime renderer avanzado queda fuera de esta auditoria.
- Reportes v1 siguen importando, pero los clientes externos deberian migrar a v2 para no perder target/evidence.

## Fuente 3 - RustNovel - Auditoria de fallos forzados, duplicacion y arquitectura

Fecha: 2026-05-18

Alcance: lectura estatica del workspace local, busqueda dirigida de rutas de presion y comparacion con arquitecturas open source/permisivas. Esta auditoria no corrige codigo: su objetivo es forzar bugs compuestos, detectar componentes que prometen mas de lo que cumplen y preparar una ruta de consolidacion.

### Fuentes externas usadas

- egui `TextureHandle`: referencia util para el modelo de handles de textura y vida util de recursos GPU. Fuente: https://docs.rs/egui/latest/egui/struct.TextureHandle.html
- Bevy `AssetServer`/`Handle`: arquitectura basada en handles y cache central de assets. Fuente: https://docs.rs/bevy/latest/bevy/asset/struct.AssetServer.html y https://docs.rs/bevy/latest/bevy/asset/enum.Handle.html
- Godot `ResourceLoader`: carga de recursos con politica explicita de cache. Fuente: https://docs.godotengine.org/en/stable/classes/class_resourceloader.html
- Ren'Py: referencia de motor VN maduro para separacion de guion, displayables y prediccion de recursos. Fuente: https://www.renpy.org/doc/html/
- Yarn Spinner: referencia de authoring por nodos/labels y saltos textuales. Fuente: https://docs.yarnspinner.dev/3.1/write-yarn-scripts/scripting-fundamentals/jumps
- Licencias permisivas consultadas: Bevy MIT/Apache (https://github.com/bevyengine/bevy/blob/main/LICENSE-MIT), egui MIT/Apache (https://github.com/emilk/egui/blob/main/LICENSE-MIT), Godot MIT (https://github.com/godotengine/godot/blob/master/LICENSE.txt) y Yarn Spinner MIT (https://github.com/YarnSpinnerTool/YarnSpinner/blob/main/LICENSE.md).

### Metodo

Se forzaron fallos por combinacion de subsistemas, no solo por casos unitarios. Los comandos de auditoria buscaron duplicacion de cache, reconstruccion de scripts, clones pesados, rutas de preview, trazabilidad y placeholders:

- `rg "AssetStore::new|TextureHandle|image_cache|audio_duration_cache|composer_image_cache"`
- `rg "NodeGraph|to_script|to_script_strict|CompilationCache|OperationLog|EvidenceTrace"`
- `rg "ComposerSnapshot|SceneStage|PlayerVisual|visual_state|ExtCall|pending_transition"`
- conteo de `.clone(` por archivo y chequeo de archivos Rust mayores a 500 lineas.

Resultado de tamano: no se detectaron archivos Rust principales por encima de 500 lineas en `crates` y `tools`. El riesgo actual no es tamano por archivo, sino duplicacion entre rutas.

### Ejecucion dinamica realizada

Despues de la lectura inicial se ejecutaron pruebas reales y jobs locales para separar sospechas de fallos reproducibles.

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

- `python -m pytest tests/python/test_vnengine_composer_report_bindings.py -q -rs` salto 10 tests porque el modulo instalado local no exponia los bindings GUI.
- `python -m pytest tests/python/test_vnengine_graph_bindings.py -q -rs` paso 2 y salto 9 por falta de bindings/API en el modulo instalado local.
- `python -m pytest tests/python/test_vnengine_bindings.py -q -rs` paso 3 y salto 5 por falta de APIs nativas.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\ci-local.ps1 -Job python-tests` construyo e instalo el binding con `maturin`, ejecuto 62 tests y fallo 1.

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

Interpretacion: el comportamiento de `connect_or_branch` parece coherente con el modelo 0-based de `NodeGraph`, pero el test Python espera un id `4` y una posicion que no coincide con `NODE_VERTICAL_SPACING`. Esto no es solo un test mal escrito: revela que el contrato publico Python de IDs/posicion generada no esta congelado con fixture cross-cliente. El mismo flujo pasa en core/GUI, pero falla en la suite Python real al usar el binding construido.

### Hallazgo A - assets duplicados y carga sincrona en GUI

Evidencia local:

- `crates/gui/src/assets.rs` define `AssetManager` con cache propia.
- `crates/gui/src/editor/asset_browser.rs` mantiene `image_cache`, `image_failures`, `audio_duration_cache` y crea `AssetStore`.
- `crates/gui/src/editor/scene_stage.rs` crea otro `AssetStore` y resuelve/carga imagenes para el stage.
- `crates/gui/src/editor/workbench.rs` mantiene `composer_image_cache`, `composer_image_failures` y `audio_duration_cache`.
- `crates/runtime/src/audio.rs` mantiene `audio_cache` separado.

Bug compuesto a forzar:

1. Crear un proyecto con 80-150 PNG grandes y 20 audios.
2. Abrir Asset Browser, seleccionar una escena con background, activar Visual Composer y luego Play.
3. Cambiar calidad Draft/Balanced/High y fit Cover/Contain varias veces.
4. Reimportar el mismo archivo bajo el mismo path y cambiar de escena sin reiniciar.

Senal esperada:

- Decodificacion repetida en el hilo UI.
- Texturas duplicadas por cache key de thumbnail, scene stage y player.
- Stutter visible al seleccionar escena o al abrir el browser.
- Invalidacion global de caches despues de importar un asset, aunque solo cambio un recurso.

Direccion:

Crear un `EditorResourceService` compartido: resolver path una vez, devolver handles ref-counted, decodificar en background, mantener LRU por bytes y separar cache de bytes, cache de imagen decodificada y cache de textura GPU. Bevy/Godot son buenos modelos: handle estable + cache central + politica explicita.

### Hallazgo B - Composer, Player y runtime aun reconstruyen la misma escena

Evidencia local:

- `crates/core/src/authoring/composer.rs` produce `ComposerSnapshot`.
- `crates/gui/src/editor/workbench/player_mode_ops.rs` reconstruye `SceneState` desde `VisualState`.
- `crates/gui/src/editor/player_ui/render.rs` vuelve a renderizar desde contexto propio.
- `crates/gui/src/editor/scene_stage.rs` renderiza stage y assets con otra ruta.

Bug compuesto a forzar:

1. Crear `Scene` con background y dos personajes duplicando nombre pero con poses distintas.
2. Agregar `ScenePatch` que cambia solo una pose y luego un `Choice` con opciones largas.
3. En Composer, avanzar el preview, elegir opcion y volver al modo de nodo aislado.
4. Comparar objetos visibles, owners, dialogo, choices y safe area contra Play.

Senal esperada:

- Objetos o backgrounds heredados no coinciden entre Composer y Play.
- Choice visible en runtime pero no en snapshot aislado, o texto largo sale del overlay.
- Ownership por fallback usa nombre/path cuando falta provenance completa.

Direccion:

Fusionar hacia un `PresentationSnapshot` unico en core: visual state + overlays + layer objects + provenance + layout de dialogo/choices. GUI y runtime deberian consumir ese snapshot, no reconstruirlo localmente.

### Hallazgo C - doble grafo y clones pesados en authoring/GUI

Evidencia local:

- `crates/gui/src/editor/authoring_adapter.rs` clona el grafo de core para compatibilidad.
- `crates/gui/src/editor/undo.rs` guarda snapshots completos de `NodeGraph`.
- `crates/gui/src/editor/workbench/operation_ops.rs` calcula fingerprint creando `AuthoringDocument` y `to_script()`.
- Archivos con mas clones encontrados: `diagnostics.rs`, `trace.rs`, `script_sync.rs`, `report_fingerprint.rs`, `asset_browser.rs`, `project_ops.rs`, `player_mode_ops.rs`, `operation_ops.rs`.

Bug compuesto a forzar:

1. Generar un grafo con 1000 nodos, 200 choices y 40 fragments.
2. Hacer marquee select, mover grupo, crear fragment, deshacer y rehacer.
3. Importar reporte y aplicar quick-fix de revision.
4. Medir tiempo de `current_authoring_fingerprint`, memoria de undo y latencia de seleccion.

Senal esperada:

- Undo retiene multiples copias completas.
- Cambios puramente visuales disparan recomputo semantico completo.
- Python/CLI vuelven a serializar o clonar objetos grandes para operaciones simples.

Direccion:

Usar grafo unico con view-state GUI separado, versiones dirty por dominio (`semantic_version`, `layout_version`, `asset_version`), fingerprints incrementales y undo por operaciones/deltas. Guardar snapshots completos solo como checkpoint espaciado.

### Hallazgo D - trazabilidad causal existe, pero puede ser sintetica

Evidencia local:

- `core::authoring::diagnostics` ya tiene `EvidenceTrace`, `DiagnosticTarget`, `FieldPath` y envelopes v2.
- `operation_ops.rs` registra operaciones y verification runs, pero valida con `validate_authoring_graph_no_io` y puede no pasar por resolver real de assets.
- Los imports de reportes pueden bloquear autofix por stale, pero el usuario necesita explicacion causal legible.

Bug compuesto a forzar:

1. Importar asset externo, asignarlo a escena, moverlo en Composer.
2. Borrar el archivo fisico fuera del editor.
3. Ejecutar validacion con project root, exportar reporte, modificar layout e importar de nuevo.
4. Pedir `explain(diagnostic_id)` desde CLI/Python.

Senal esperada:

- El diagnostic puede decir "asset missing", pero no demostrar la cadena: operacion -> campo -> resolver -> candidato -> regla -> fallo -> fix.
- Layout stale y semantic stale pueden mezclarse en la UX.

Direccion:

Convertir el resolver de assets y targets en productor de evidencia. Cada regla de validacion debe emitir `TraceAtom` real, no solo construir envelope al final.

### Hallazgo E - OperationLog cubre mucho, pero no es aun el bus unico

Evidencia local:

- Hay `OperationKind` tipado y `VerificationRun`.
- `workbench/ui_actions.rs` traduce acciones del Composer.
- `undo.rs` todavia opera sobre snapshots de grafo y no sobre operaciones semanticas.

Bug compuesto a forzar:

1. Crear nodo con click derecho, conectar, editar campo, importar asset, arrastrar objeto visual, cambiar layer lock y hacer Ctrl+Z.
2. Exportar `AuthoringDocument`.
3. Reproducir el log en una sesion headless.

Senal esperada:

- Algunas acciones no son replayables porque dependen de estado GUI.
- Undo restaura posicion completa del grafo, no la intencion del usuario.
- VerificationRun no siempre representa el resolver usado por GUI.

Direccion:

Hacer que toda mutacion pase por `AuthoringCommandBus`: produce operacion, aplica delta, invalida caches por dominio y registra verification run. GUI, Python y CLI deben llamar el mismo bus.

### Hallazgo F - layout responsive mejora, pero falta arquitectura de docking

Evidencia local:

- Ya no hay archivos grandes y hay tests de layout.
- El Workbench aun decide paneles principales desde una funcion de layout y varios paneles compiten por espacio fijo/minimo.
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

Migrar a `WorkspaceLayoutTree`: paneles con min/preferred/max, prioridad de colapso, overflow por tabs y virtualizacion de grids. Cada panel debe declarar su contrato, no negociar implicitamente en Workbench.

### Hallazgo G - export ejecutable y capacidades no estan cerrados como producto

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

- Export puede generar bundle valido, pero no necesariamente un producto ejecutable autocontenido con reporte de capacidades uniforme.
- GUI/Python/CLI pueden explicar distinto las limitaciones.

Direccion:

Crear `ExportPlan`: entradas, runtime artifact, politica de proteccion, capabilities, warnings, launcher y hashes. GUI/Python/CLI solo serializan y ejecutan el plan.

### Hallazgo H - fragments/subgrafos necesitan stress de composicion

Evidencia local:

- Core tiene fragments, ports y `SubgraphCall`.
- CLI/Python exponen partes del modelo.
- El GUI ya tiene tests, pero el caso dificil es la composicion repetida con entradas/salidas multiples.

Bug compuesto a forzar:

1. Crear fragment A con dos salidas, fragment B que llama A, y dos llamadas a A desde ramas diferentes.
2. Renombrar puertos, borrar un nodo interno y refrescar ports.
3. Exportar strict y validar reachability/cycles.

Senal esperada:

- Labels internos pueden crecer o colisionar si el namespace no incluye call instance de forma consistente.
- Stale ports pueden quedar visibles pero no bloqueantes en algun cliente.

Direccion:

Tratar fragments como modulos compile-time: ownership unico, interface hash, namespace determinista por call node y diagnosticos cross-fragment con ruta causal completa.

### Hallazgo I - audio preview y metadata estan separados del modelo de assets

Evidencia local:

- `asset_browser.rs` calcula duracion y offset.
- `player_audio_ops.rs` resuelve previews.
- `runtime/src/audio.rs` cachea bytes.
- `workbench/audio_preview_store.rs` crea otro acceso a assets.

Bug compuesto a forzar:

1. Importar un audio grande, abrir browser, mover slider de offset, previsualizar BGM/SFX/Voice y luego Play.
2. Reemplazar el archivo por otro con misma ruta y distinta duracion.
3. Repetir preview desde inspector y browser.

Senal esperada:

- Duracion cacheada puede quedar vieja.
- Decode/carga se repite en rutas distintas.
- Preview y runtime pueden resolver distinto paths absolutos/relativos.

Direccion:

Unificar `AudioAssetService`: metadata cache por fingerprint de archivo, streaming para bytes grandes, handles por canal y eventos de invalidacion.

### Hallazgo J - paridad Python/CLI/GUI aun necesita contrato de fixture comun

Evidencia local:

- Python ya expone `NodeGraph`, report v2, Composer y fragments.
- CLI tiene comandos authoring.
- Muchas pruebas construyen fixtures ad hoc por crate.

Bug compuesto a forzar:

1. Crear un `.vnauthoring` con scene layers, choice largo, fragment, ExtCall y asset faltante.
2. Validar desde CLI, Python y GUI.
3. Exportar reporte v2, generar repro, aplicar cambio de layout y reimportar.

Senal esperada:

- JSON puede ser compatible, pero los mensajes, stale status, capabilities o operation ids pueden diferir.

Direccion:

Crear `tests/fixtures/authoring_contract/` con documentos dorados y helpers compartidos. Cada cliente debe probar contra los mismos JSON, no contra copias locales simplificadas.

### Componentes candidatos a fusionar

| Grupo | Componentes actuales | Fusion propuesta |
| --- | --- | --- |
| Assets visuales | `AssetManager`, `AssetBrowserPanel`, `SceneStagePainter`, caches del Workbench | `EditorResourceService` con handles, LRU e invalidacion por fingerprint |
| Audio | browser duration cache, preview store, runtime audio cache | `AudioAssetService` con metadata, stream/cache y canales |
| Presentacion | `ComposerSnapshot`, `SceneState`, `VisualState`, Player UI | `PresentationSnapshot` unico en core |
| Grafo | core `NodeGraph`, GUI wrapper, adapter clones | core graph + `GraphViewState` GUI |
| Operaciones | undo snapshots, operation log, UI actions | `AuthoringCommandBus` tipado |
| Reportes | GUI import, CLI report, Python report wrappers | serializer/deserializer core + fixtures compartidos |
| Export | bundle, package, executable, capability warnings | `ExportPlan` headless consumido por GUI/CLI/Python |

### Mejoras de memoria y procesos

1. Evitar clones completos de grafo en cada mutacion; usar deltas y checkpoints.
2. No llamar `to_script()` para calcular fingerprint de cambios solo visuales.
3. Separar cache de bytes, imagen decodificada y textura GPU.
4. Decodificar imagen/audio fuera del hilo UI y publicar handles listos.
5. Incluir budget por cache y telemetria: bytes, entradas, hits, misses, evictions, decode_ms y upload_ms.
6. Internar strings repetidas de labels, paths y speaker names en authoring.
7. Usar fingerprints por dominio para invalidar solo lo necesario.
8. En Python, devolver wrappers/JSON bajo demanda en vez de clonar listas grandes para cada llamada.

### Pruebas nuevas recomendadas

No deben ser tests de relleno. Deben forzar interaccion entre subsistemas:

- `asset_cache_multiview_stress`: mismo background en browser, composer y player no debe decodificarse tres veces.
- `presentation_snapshot_parity`: Composer y Play renderizan el mismo snapshot para scene/patch/dialogue/choice/transition.
- `undo_delta_memory_contract`: 1000 nodos + 50 movimientos no debe multiplicar memoria por 50 snapshots completos.
- `evidence_trace_asset_resolver_chain`: asset faltante debe explicar operacion, field path, resolver lookup, candidatos y fix.
- `fragment_nested_namespace_stress`: llamadas repetidas a fragments no colisionan labels internos.
- `audio_metadata_invalidation`: reemplazar audio en misma ruta actualiza duracion y no usa cache vieja.
- `contract_fixture_cli_py_gui`: mismo fixture produce mismos codes, targets, fingerprints y stale status en CLI/Python/GUI.
- `window_layout_matrix`: paneles no se superponen en varias resoluciones y escalas.

### Riesgos priorizados

1. Stutter de UI por carga sincrona de assets: impacto alto, probabilidad alta.
2. Divergencia Composer/Play: impacto alto, probabilidad media-alta.
3. Memoria por undo/clones en grafos grandes: impacto medio-alto, probabilidad alta en proyectos reales.
4. Reportes explicables pero no causalmente verificables: impacto alto para QA.
5. Export ejecutable sin contrato unico de capabilities: impacto alto para producto final.

### Cierre

La arquitectura va en buena direccion: core ya tiene authoring, reportes v2, fragments, composer headless y operation log. El problema actual es que el GUI y algunos bindings todavia operan como clientes con caches, reconstrucciones y atajos propios. El siguiente cierre no deberia ser "agregar mas botones"; debe ser consolidar servicios: recursos, presentacion, operaciones, reportes y export.

## Fuente 4 - Rustnovel unfinished, duplicate, migration and flow audit

Date: 2026-05-26

Scope: static analysis, CLI execution, tests and report consolidation focused on unfinished components, non-usable components, duplicate components, duplicate non-functional components, old-version remnants, partially migrated components, components wired to the wrong place, simulated actions, unused code risk and end-to-end flow integrity.

Repository root used:

```text
C:\Users\pelju\Downloads\rustnovel-main (8)\rustnovel-main
```

### Executive summary

The Rust workspace itself is in a relatively healthy state after the previous implementation pass: `cargo test --workspace --all-targets --no-fail-fast`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo audit -D warnings --ignore RUSTSEC-2024-0436` completed successfully in the audited run.

The project is not closed as a product. The hard failures and ambiguity are concentrated at boundaries:

- Python direct test execution imports an obsolete globally installed `visual_novel_engine` package instead of the local wheel unless the dedicated local build path is used.
- CLI `trace --format json` still writes YAML-like output through `serde_norway`, so the CLI promises JSON but emits a format that JSON tooling rejects.
- Example scripts are not usable as real authoring examples because they reference missing assets such as `bg/classroom.png`.
- Some flows are intentionally simulated, but the product does not always separate simulation, preview and real execution clearly enough for users or external clients.
- There are duplicated assets, duplicated dependency versions and duplicated service concepts that should be consolidated only after compatibility tests exist.
- The previous migration to shared core concepts is real, but there are wrappers/adapters that make some components look fully migrated while still carrying old ownership or API compatibility.

### Main findings table

| Priority | Area | Finding | Impact | Evidence |
| --- | --- | --- | --- | --- |
| P0 | Python packaging | `python -m pytest -q` can run against a stale global package instead of the local Rust/Python binding. | Local test results are misleading; missing APIs appear as product bugs or are skipped depending on environment. | Python import path pointed to `C:\Users\pelju\AppData\Local\Programs\Python\Python312\Lib\site-packages\visual_novel_engine\__init__.py`; direct pytest produced `25 failed, 37 passed`. |
| P1 | CLI trace | `trace --format json` writes YAML-style output. | Automation and users expecting JSON cannot parse the generated file. | `python -m json.tool target\trace.json` failed with `JSONDecodeError`. |
| P1 | Examples | Example authoring scripts reference missing files. | The first real user path fails validation. | `VAL_ASSET_NOT_FOUND` for `bg/classroom.png`. |
| P1 | Runtime simulation | `ExtCall` is marked as runtime-real but dry-run/repro simulate it. | Export/validation can appear more complete than the real runtime behavior. | `DRY_EXTCALL_SIMULATED`, dry-run metadata, repro simulated effects. |
| P2 | Assets | Duplicate visual assets are identical binary blobs under alias paths. | Storage and manifest noise; possible intentional fixture but should be declared. | Manifest duplicate hash `2931604f...`, size `2002227`. |
| P2 | Migration | `StoryNode` moved to core; GUI wrapper remains. `NodeGraph` exists in core and GUI adapter. | Mostly migrated, but public surface can hide old ownership paths. | `types.rs`, `node_types.rs`, `graph.rs`, `node_graph.rs`. |
| P2 | Dependencies | Multiple versions of `image`, `toml`, `pixels`, `wgpu`, `rodio`, `windows` family appear. | Larger build graph and possible integration friction. | `cargo tree -d`, direct Cargo.toml references. |
| P3 | Potential garbage | Several allow suppressions, anchors and fallback compatibility paths remain. | Some are legitimate compatibility; deletion needs strict proof. | `allow(dead_code)`, `_content_unused`, `_type_anchor`, compatibility exports. |

### Files and local state observed

Modified files in the working tree at audit time were treated as intentional local changes and were not reverted. Relevant observed areas:

- CI and script files.
- `Cargo.lock`.
- `crates/core/src/protected_content.rs`.
- `crates/gui/runtime/Cargo.toml`.
- `pyproject.toml`.
- Python tests and `tests/python/conftest.py`.
- CLI implementation in `tools/cli`.

Untracked relevant paths:

- `.serena/`.
- `tests/python/conftest.py`.

The audit document itself was the only document artifact created during that pass.

### Component map

| Layer | Components | Responsibility | Status |
| --- | --- | --- | --- |
| Core | `Script`, `Engine`, authoring graph, diagnostics, validation, fingerprints, dry-run, repro | Product semantics and validation contracts. | Mostly real and tested, but some simulated capability paths remain. |
| CLI | `vnengine` commands: validate, trace, authoring validate, manifest, package/export | Headless user workflows and automation. | Usable but `trace --format json` contract is wrong. |
| GUI | Workbench, Visual Composer, player preview, asset browser, report import | Interactive authoring. | Functionally broad, but resource/presentation/operation paths duplicate ownership. |
| Python | `visual_novel_engine` native binding plus wrapper package | Scripting and external integration. | Local wheel path works; default Python import path is unsafe for testing. |
| Examples | `examples/scripts` stories and assets | First-run and regression fixtures. | Not fully usable because missing asset references break authoring validation. |
| Packaging/export | bundle/package/capability report | Product delivery. | Needs a single `ExportPlan` contract before claiming executable product closure. |

### Critical flow diagram

```text
Authoring JSON / GUI graph
        |
        v
Core NodeGraph / AuthoringDocument
        |
        +--> validation/report v2/fingerprints
        |
        +--> to_script / to_script_strict
        |
        v
Script / Engine / dry-run / trace / replay
        |
        +--> CLI outputs
        +--> Python bindings
        +--> GUI player/composer preview
        +--> export/package
```

The weakest links in that chain are not the core structures. They are the conversion and client boundaries: Python environment selection, CLI output format, examples as fixtures, simulated `ExtCall`, duplicate resource services and export capability reporting.

### Dynamic verification - Rust

Commands recorded as passing in the audit pass:

```powershell
cargo test --workspace --all-targets --no-fail-fast
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo audit -D warnings --ignore RUSTSEC-2024-0436
```

Interpretation:

- The broad Rust workspace test suite did not expose a current failing regression.
- Formatting and clippy were clean under `-D warnings`.
- RustSec audit was clean except for the explicitly ignored advisory `RUSTSEC-2024-0436`; the ignore should remain documented and periodically revisited.

### Dynamic verification - Python

Direct command:

```powershell
python -m pytest -q
```

Observed result:

```text
25 failed, 37 passed
```

Critical import evidence:

```text
C:\Users\pelju\AppData\Local\Programs\Python\Python312\Lib\site-packages\visual_novel_engine\__init__.py
```

The public names available from that imported package were only:

```text
Engine
PyEngine
ScriptBuilder
VnConfig
run_visual_novel
visual_novel_engine
```

This is not the local current binding surface. When the local wheel/maturin path and `PYTHONPATH` override were used, the local Python suite passed:

```text
62 passed
```

Interpretation:

- The Python tests are not intrinsically failing as a product behavior when run against the correct artifact.
- The default developer command is unsafe because it can import a global stale package.
- This is a P0 workflow bug because it can create false failures, false skips and false confidence.

Required closure:

- Keep `tests/python/conftest.py` as an import guard.
- Fail fast if `visual_novel_engine.__file__` is outside the repository or the expected local wheel path.
- Document the required local build command.
- Make CI use the exact same import path as local verification.

### Dynamic verification - CLI

Commands exercised:

```powershell
cargo run -p vnengine_cli --bin vnengine -- --help
cargo run -p vnengine_cli --bin vnengine -- validate examples\scripts\script.json
cargo run -p vnengine_cli --bin vnengine -- trace examples\scripts\script.json --format json --output target\trace.json
python -m json.tool target\trace.json
cargo run -p vnengine_cli --bin vnengine -- authoring validate examples\scripts\demo_story.json --project-root examples\scripts --output target\example-authoring-report.json
cargo run -p vnengine_cli --bin vnengine -- authoring manifest examples\scripts
```

Findings:

- CLI help exists and the command surface is discoverable.
- Basic runtime validation can pass on simple script paths.
- `trace --format json` writes a file that JSON tooling rejects.
- `authoring validate` on examples fails with missing assets.
- `authoring manifest` exposes duplicate hashes in the example assets.

Trace failure:

```text
JSONDecodeError
```

Authoring validation failure:

```text
VAL_ASSET_NOT_FOUND
bg/classroom.png
```

Duplicate manifest signal:

```text
2931604f...
size: 2002227
```

### Detailed finding P0 - Python uses a global obsolete package

Evidence:

- `pyproject.toml:30-33` defines the local package/build surface.
- `tests/python/conftest.py:4-21` exists specifically to steer/import-guard the local module.
- `crates/py/src/lib.rs:17-36` exposes the native binding surface expected by current tests.
- `python/vnengine/native.py:8-32` wraps native import behavior.
- `python/vnengine/engine.py:130-131` and `python/vnengine/engine.py:138-142` have compatibility fallback logic.

Why this matters:

- A user running `python -m pytest -q` expects to test the checked-out repository.
- Instead, Python can resolve `visual_novel_engine` from global `site-packages`.
- The result is noisy and dangerous: current APIs appear missing, tests can skip critical APIs, and fixes may target a stale installed package instead of the repository.

Recommendation:

- Treat local Python import provenance as a test precondition.
- Use a dedicated venv or repo-local wheel path.
- Add a small healthcheck command that prints module path, native extension path and exposed version.
- Make the direct failure message actionable: "you are importing global site-packages, run the local build command".

### Detailed finding P1 - CLI trace writes YAML while promising JSON

Evidence:

- `tools/cli/src/bin/vnengine.rs:50` exposes trace format selection.
- `tools/cli/src/bin/vnengine.rs:142-143` handles trace command output.
- `tools/cli/src/bin/vnengine.rs:343` uses the serialization path that produced YAML-style output.
- `python -m json.tool target\trace.json` failed.

Why this matters:

- CLI output formats are public contracts.
- A `.json` file that is not valid JSON breaks automation, CI consumers and downstream tooling.
- The command name and output extension create false confidence.

Recommendation:

- Implement explicit format dispatch: `json` through `serde_json`, `yaml` through `serde_norway`/YAML serializer.
- Add CLI tests that parse the emitted file with `serde_json` for JSON and YAML parser for YAML.
- Consider defaulting to JSON for machine-readable trace output.

### Detailed finding P1 - examples reference missing assets

Evidence:

- `examples/scripts/script.json:6` and `examples/scripts/script.json:22` reference a classroom background path.
- `examples/scripts/demo_story.json:5` and `examples/scripts/demo_story.json:21` reference the same missing asset concept.
- `examples/scripts/project.toml:18-35` lists project asset metadata.
- `examples/scripts/project.toml:16` anchors the project root behavior.
- CLI authoring validation emitted `VAL_ASSET_NOT_FOUND`.

Why this matters:

- Examples are both documentation and regression fixtures.
- If the first validation path fails, users cannot distinguish a broken install from intentionally incomplete demo content.
- Missing example assets also hide whether manifest/dedupe behavior is fixture-driven or accidental.

Recommendation:

- Either add the missing asset or change the story references to existing assets.
- Add an examples smoke test that runs both runtime validation and authoring validation.
- If duplicate assets are intentional, declare them as dedupe fixtures with names and comments.

### Detailed finding P1 - labels can point to event length boundary

Evidence:

- `examples/scripts/demo_story.json:35` contains `"node_23": 5`.
- The script has 5 events.
- Validation passed even though the label points to the end boundary.
- Inspection showed:

```text
events 5
labels {'node_23': 5}
```

Why this matters:

- A label at exactly `events.len()` may be valid as an end sentinel or invalid as an off-by-one target depending on engine contract.
- The current behavior should be made explicit, otherwise future migrations can break story flow silently.

Recommendation:

- Decide contract:
  - Valid: document label-at-end as EOF marker and test it.
  - Invalid: emit diagnostic and reject in strict export.

### Detailed finding P1 - ExtCall is runtime-real but dry-run/repro simulate it

Evidence:

- `execution_contract.rs:62` and `execution_contract.rs:113` classify capabilities.
- `dry_run.rs:65-83` and `dry_run.rs:110` simulate or warn on ExtCall behavior.
- `vnengine.rs:330` exposes CLI behavior around trace/repro.
- Existing diagnostics include `DRY_EXTCALL_SIMULATED`.

Why this matters:

- Simulation is acceptable if visible and contractually separated.
- It is dangerous if export or validation implies that a real runtime integration exists.

Recommendation:

- Keep simulation warnings, but make capability classification explicit in all clients.
- Export should include a capability gap report for `ExtCall`.
- Repro should record simulated effects separately from real effects.

### Detailed finding P2 - Ren'Py import degrades by default

Evidence:

- `tools/cli/src/bin/vnengine.rs:104-105` exposes import command behavior.
- `tools/cli/src/bin/vnengine.rs:168-175` controls fallback/degradation behavior.
- `crates/gui/src/editor/workbench/import_ops.rs:31` shows GUI import path.

Why this matters:

- A best-effort import is useful for exploration.
- A default degraded import can look like a correct migration while losing semantics.

Recommendation:

- Surface degradation count and unsupported constructs clearly.
- Use strict import by default for product workflows; allow best-effort import behind an explicit flag.
- Store import warnings in authoring metadata and reports.

### Detailed finding P2 - duplicate assets

Evidence:

- Authoring manifest showed identical hashes for different asset paths.
- Duplicate hash:

```text
2931604f...
```

- Duplicate size:

```text
2002227
```

Why this matters:

- Duplicate blobs can be valid alias fixtures.
- They are also likely storage noise if not declared.
- Deleting them blindly can break tests, examples or dedupe coverage.

Recommendation:

- Create an asset manifest test that marks intentional duplicates.
- If duplicates are examples, name them as alias/dedupe fixtures.
- If they are accidental, update references first and delete only after validation and manifest tests pass.

### Detailed finding P2 - StoryNode migrated to core, but wrappers remain

Evidence:

- `types.rs:20`.
- `node_types.rs:6`.
- `graph.rs:53`.
- `node_graph.rs:42-45`.
- `node_graph.rs:43`.
- `node_graph.rs:314-325`.

Interpretation:

- The migration is not fake; the core owns the main authoring concepts.
- The GUI still carries wrapper/adaptation code for compatibility and view state.
- The risk is not the existence of a wrapper by itself, but uncontrolled semantic drift between wrapper and core.

Recommendation:

- Keep core as the only semantic owner.
- Make GUI state explicitly view-only.
- Add fixture tests that compare core/GUI/Python/CLI serialization for the same graph.

### Detailed finding P2 - duplicated dependencies

Evidence:

- `crates/assets/Cargo.toml:11` uses `image 0.25`.
- `crates/gui/Cargo.toml:31` uses `image 0.24`.
- `crates/core/Cargo.toml:22` uses `toml 0.8`.
- `crates/gui/Cargo.toml:23` uses `toml 0.9.11`.
- Runtime/GUI graph includes duplicated `pixels`, `wgpu`, `rodio` and `windows` family versions.
- `cargo tree -d` shows duplicate dependency families.

Why this matters:

- Duplicate dependency versions are not always bugs.
- In a GUI/runtime project, they can increase compile time, binary size and resource type mismatch risk.

Recommendation:

- Use `cargo tree -d` as the tracking command.
- Converge direct dependencies first where APIs are compatible.
- Avoid forcing a global upgrade that breaks runtime crates.
- Document dependency duplicates that are unavoidable because of upstream constraints.

### Detailed finding P3 - potential garbage code and suppression markers

Observed patterns:

- `allow(unused_assignments)`.
- `allow(dead_code)`.
- `SkipMode`.
- `_type_anchor`.
- `_content_unused`.
- Compatibility exports and fallback wrappers.

Why this matters:

- Some of this is likely removable.
- Some of it is compatibility glue for external clients, serialization or future features.
- Removing all of it as "garbage" can break indirect behavior.

Hard requirements before deletion:

1. Search references in Rust, Python, tests, examples and docs.
2. Check serialization compatibility and public API exposure.
3. Add or update tests proving the behavior is unused.
4. Remove one category at a time.
5. Run workspace tests and client-specific tests.
6. Keep a changelog entry for public API removals.

### Components simulated, incomplete or not fully real

| Component/path | Current behavior | Why it is not fully closed | Required closure |
| --- | --- | --- | --- |
| Dry-run `ExtCall` | Emits simulation warning and metadata. | Does not execute external call. | Capability report must distinguish simulated from real. |
| Repro `ExtCall` | Can replay simulated effect. | Repro can look complete while external side effect is absent. | Store simulated effects separately. |
| Python `run_visual_novel` fallback | Compatibility path may return wrapper behavior. | Can hide missing native feature. | Healthcheck and explicit native requirement for advanced APIs. |
| Ren'Py import | Best-effort/degraded import. | Unsupported constructs can be silently approximated if user does not inspect warnings. | Strict import mode and persisted warnings. |
| Scene preview fallbacks | GUI preview may synthesize state from local routes. | Composer/Play parity is not guaranteed for every edge. | Shared `PresentationSnapshot`. |

### Components duplicated or suspicious

| Area | Duplicated components | Risk |
| --- | --- | --- |
| Visual resources | Asset manager, asset browser cache, scene stage cache, workbench composer cache | Repeated decoding, inconsistent invalidation, memory growth. |
| Audio | Browser duration cache, preview store, runtime audio cache | Stale metadata and repeated decode paths. |
| Graph | Core graph, GUI adapter/wrapper, undo snapshots | Semantic drift and heavy memory use. |
| Presentation | Composer snapshot, visual state, player render state, scene stage render path | Composer and Play can diverge. |
| Operations | UI actions, operation log, undo snapshots | Not all actions are replayable. |
| Reports | GUI import/export, CLI authoring report, Python wrappers | Contract drift without shared fixtures. |
| Export | Bundle, package, capability report | No single end-to-end export plan. |

### Components migrated, partly migrated or only apparently migrated

| Component | Migration status | Evidence interpretation | Next step |
| --- | --- | --- | --- |
| Diagnostics v2 | Migrated | Envelopes and catalog exist across clients. | Keep shared fixtures to prevent drift. |
| Fingerprints | Migrated by domain | Story/layout/assets/document hashes exist. | Use dirty versions to avoid full recompute. |
| StoryNode | Migrated to core | GUI imports/wraps core type. | Ensure GUI adds only view state. |
| NodeGraph | Mostly core-owned | GUI wrapper remains. | Define adapter boundary and reduce clones. |
| Visual Composer | Functionally migrated to layered model | Runtime and preview still rebuild some scene state. | Move to `PresentationSnapshot`. |
| OperationLog | Implemented but not bus | Mutations are logged, but undo still snapshot-based. | Promote to `AuthoringCommandBus`. |
| Python bindings | Broad but environment-sensitive | Correct local wheel passes; global import fails. | Enforce local import provenance. |
| CLI trace | Not fully migrated to format contract | `--format json` still uses YAML output path. | Format-specific serializer and tests. |

### Prioritized backlog from this audit

1. Fix Python local import provenance and make wrong-package imports fail fast.
2. Fix CLI trace format dispatch and parse output in tests.
3. Repair examples so runtime validation and authoring validation both pass.
4. Decide label-at-EOF contract and test it.
5. Make `ExtCall` simulation boundaries visible in export, trace and repro.
6. Add shared contract fixtures for CLI/Python/GUI.
7. Consolidate visual and audio resource services.
8. Define `PresentationSnapshot` as the single source for Composer/Play parity.
9. Promote `OperationLog` into an `AuthoringCommandBus`.
10. Converge duplicate direct dependencies where low risk.
11. Mark intentional duplicate assets or delete accidental duplicates after tests.
12. Remove potential garbage code only after reference, API and serialization checks.

### Strict criteria for deleting code or assets

Deletion should be treated as a separate cleanup phase. Code or data can be removed only when all of these are true:

1. No references from Rust, Python, examples, docs or generated fixtures.
2. No public serialization compatibility requirement.
3. No public Python or CLI API exposure.
4. No test relies on the alias, fallback or fixture.
5. A replacement path is already tested.
6. The workspace passes tests after removal.
7. The diff does not mix cleanup with behavior changes unless necessary.

### Acceptance matrix

| Area | Current status | Acceptance condition |
| --- | --- | --- |
| Rust workspace | Passing in audited run. | Keep `cargo test`, `fmt`, `clippy -D warnings`, `cargo audit` green. |
| Python | Correct local path passes; default path unsafe. | Direct local command fails fast on wrong import and passes against built local artifact. |
| CLI trace | Emits invalid JSON for JSON mode. | `python -m json.tool` or `serde_json` parse succeeds. |
| Examples | Authoring validation fails. | Example runtime and authoring validation pass. |
| Assets | Duplicate blobs observed. | Intentional duplicates documented or accidental duplicates removed with tests. |
| Simulation | Warnings exist, but product boundary still ambiguous. | Export/repro/trace all distinguish simulated and real capability effects. |
| Migration | Core migration mostly real. | Shared fixtures prove CLI/Python/GUI parity. |
| Cleanup | Potential garbage identified. | Deletion follows hard criteria and does not break indirect features. |

### Conclusions from source 4

The project is not in a broken general state. The core test suite and many authoring capabilities are real. The remaining risk is concentrated in user-facing honesty and integration boundaries: what is truly executed, what is simulated, what is only previewed, what is imported from a stale package, and what is duplicated because clients still own their own caches or wrappers.

The next implementation should avoid broad cleanup first. The right order is:

1. Make the observable contracts truthful.
2. Add cross-client fixtures.
3. Consolidate duplicated services.
4. Then remove unused or duplicated code with strict proof.
