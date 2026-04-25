# Auditoria Integral de Seguridad, Eficiencia, Calidad y Trazado

Fecha: 2026-03-30

Repositorio auditado: `C:\Users\pelju\Downloads\asd\prueba-de-uso-de-rust`

## Resumen Ejecutivo

La base tecnica del proyecto es solida: el workspace compila, la suite de tests pasa completa, hay controles reales de integridad para saves, saneamiento fuerte en `AssetStore`, trazado determinista del engine, reportes ricos del editor y una estrategia de importacion Ren'Py bastante alineada con un criterio motor-primero.

Los riesgos mas relevantes no estan en el runtime central del motor, sino en los bordes donde la disciplina cambia entre subsistemas:

1. El editor puede leer archivos fuera del root del proyecto al confiar en `entry_point` y `supported_languages` del manifest sin contencion de rutas.
2. El importador Ren'Py protege contra traversal textual, pero no aplica la misma defensa contra symlinks/junctions que si existe en `AssetStore`.
3. Los limites de recursos del script llegan despues del parseo completo del JSON, asi que un input hostil puede consumir memoria/CPU antes de que entren las guardas.
4. La observabilidad tradicional esta por detras de la trazabilidad funcional: hay `trace`, dry-runs y repros muy buenos, pero el logging operativo esta fragmentado y parece incompleto.
5. El editor paga costos de mantenibilidad y escalabilidad por recompilar y clonar estado grande en varios flujos de preview, autofix y repro.

Mi conclusion general es que el proyecto esta mas cerca de una base de producto seria que de un prototipo fragil, pero necesita homogeneizar sus invariantes de seguridad y simplificar la capa del editor para escalar con menos deuda.

## Metodo y Evidencia

- Revision estatica del workspace Rust/Python.
- Ejecucion de salud del repo:
  - `cargo check --workspace --tests`
  - `cargo test --workspace`
  - `cargo bench -p visual_novel_engine --bench core_benches -- --list`
  - `cargo run -p vnengine_cli --bin repo_report`
- Revision paralela con subagentes para seguridad, eficiencia/calidad y trazado/observabilidad.
- Lectura dirigida de modulos criticos:
  - editor/project IO, workbench y compiler
  - core/security, storage, trace y script parsing
  - renpy import y decoradores de trace
  - runtime/audio/loader
  - assets store

### Snapshot del repositorio

El reporte de lineas (`repo_report`) arroja aproximadamente:

- 168 archivos Rust
- 33,576 lineas Rust
- 14 archivos Python
- 1,742 lineas Python

Observacion: el total del repo esta fuertemente inflado por JSON de ejemplos y artefactos, por lo que las metricas agregadas del repo no deben usarse como proxy directa de complejidad del codigo ejecutable.

## Hallazgos Priorizados

### [High] Lectura arbitraria de archivos via `project.vnm` en el editor

**Impacto**

Un proyecto malicioso puede forzar al editor a leer archivos fuera del root del proyecto al abrir el manifest o al cargar localizaciones declaradas por el manifest.

**Evidencia**

- `crates/gui/src/editor/project_io.rs:20-45`
  - `load_project` carga `manifest.settings.entry_point` con `PathBuf::from(...)`.
  - Si el path es absoluto lo acepta tal cual.
  - Si es relativo, hace `parent.join(entry)` sin saneamiento ni comprobacion `within root`.
- `crates/gui/src/editor/workbench/project_ops.rs:126-145`
  - `load_localization_catalog` construye `project_root/locales/{locale}.json` con un `locale` tomado del manifest.
  - No hay validacion de traversal o canonicalizacion contra el root.

**Controles existentes**

- Migracion segura del manifest.
- Manejo de errores de IO y parseo.

**Vacio actual**

El proyecto ya tiene una defensa madura de contencion de rutas en `AssetStore` (`sanitize_rel_path`, `canonicalize_within_root`), pero esa disciplina no se reutiliza en el editor.

**Recomendacion**

Aplicar la misma politica `within root` al resolver `entry_point` y locales. Para este proyecto, el manifest no deberia poder referenciar nada fuera del arbol del proyecto.

### [High] `renpy import` bloquea traversal textual, pero no symlinks/junctions al copiar assets

**Impacto**

Un proyecto Ren'Py con enlaces simbolicos o junctions dentro del arbol escaneado podria copiar contenido externo al proyecto importado. Eso rompe la contencion de importacion y debilita la promesa de migracion segura.

**Evidencia**

- `crates/core/src/renpy_import/assets.rs:150-234`
  - `rewrite_path_and_copy` normaliza el path por componentes y bloquea `..`/absolutos.
  - Luego resuelve con `exists()` y copia con `fs::copy(&source, &destination)`.
  - No hay `canonicalize` ni verificacion final de que `source` permanezca dentro de `project_root`/`scan_root`.
- En contraste, `AssetStore` si hace esto correctamente:
  - `crates/assets/src/lib.rs:585-608`
  - `sanitize_rel_path`
  - `canonicalize_within_root`
- Hay test explicito contra escape por symlink en assets:
  - `crates/assets/src/tests/lib_tests.rs`
- No vi una defensa equivalente en el importador.

**Controles existentes**

- Traversal textual bloqueado.
- Path skipping con issue reportado.
- Tests de traversal textual para importacion.

**Vacio actual**

La defensa fuerte existe en otro subsistema, pero no en `renpy import`.

**Recomendacion**

Copiar la politica de canonicalizacion del `AssetStore` al importador y agregar tests para symlink/junction escape. En un pipeline motor-primero, el importador debe ser mas estricto que la compatibilidad superficial.

### [Medium] Los limites del script se aplican despues del parseo completo

**Impacto**

Un script JSON grande o malicioso puede consumir memoria y CPU antes de que entren `max_script_bytes`, `max_events` y otras guardas semanticas.

**Evidencia**

- `crates/core/src/script/raw.rs:59-95`
  - Primero hace `serde_json::from_str(input)`.
  - Luego migra el `Value`.
  - Luego vuelve a serializar con `serde_json::to_string_pretty`.
  - Despues recien aplica `ensure_string_budget`.
- `crates/core/src/security.rs:16-201`
  - Los chequeos de eventos, labels, longitudes y referencias son buenos, pero ocurren sobre la estructura ya parseada.

**Controles existentes**

- `ResourceLimiter`
- `ensure_string_budget`
- validacion semantica exhaustiva de labels, targets y tamaños

**Vacio actual**

No hay guardas previas al parseo ni parseo incremental/streaming para inputs no confiables.

**Recomendacion**

Meter un limite duro de bytes antes de `serde_json::from_str` y evaluar parseo acotado para inputs untrusted o provenientes de herramientas externas.

### [Medium] La autenticacion fuerte de saves existe, pero no esta integrada en el flujo normal

**Impacto**

La persistencia normal detecta corrupcion y mismatch de `script_id`, pero no autentica saves contra manipulacion local.

**Evidencia**

- `crates/core/src/storage.rs:88-138`
  - `to_authenticated_binary` y `from_authenticated_binary` implementan HMAC-SHA256.
- `crates/core/src/tests/storage_tests.rs`
  - Hay pruebas de roundtrip y tamper detection.
- Pero `SaveSlotStore` usa el flujo no autenticado:
  - `crates/core/src/storage.rs:271-332`
  - `save_slot`, `quicksave`, `load_slot`, `quickload` pasan por `to_binary` / `from_binary`.
- La GUI tambien persiste sin HMAC:
  - `crates/gui/src/persist.rs:54-65`

**Controles existentes**

- `script_id`
- checksum
- versionado
- backup y recovery en `SaveSlotStore`

**Vacio actual**

El modo autenticado existe como capacidad, no como politica activa.

**Recomendacion**

Definir si el producto necesita autenticacion por defecto, opcional por build o solo para bundles firmados. Hoy el contrato es mas debil que la implementacion disponible.

### [Medium] `ExtCall` en Python cruza la frontera host/script sin politica de capacidades

**Impacto**

Si el embedding Python carga scripts no confiables y registra handler, `ExtCall` queda en manos del host sin allowlist ni politicas por comando.

**Evidencia**

- `crates/core/src/security.rs:173-181`
  - Solo limita longitud de `command` y `args`.
- `crates/py/src/bindings/engine.rs:54-75`
  - Si el evento es `ExtCall` y existe handler, lo ejecuta con `handler.call1(py, (command, args.clone()))`.
  - Si falla, loguea con `eprintln!` pero no corta el step.

**Controles existentes**

- El callback es opt-in.
- El runtime estandar no ejecuta `ExtCall` arbitrario.
- El importador Ren'Py decora degradaciones con trazas estructuradas, lo que reduce opacidad.

**Vacio actual**

No hay modo `deny extcall`, lista permitida ni namespace de comandos confiables.

**Recomendacion**

Mantener `ExtCall` como escape hatch explicito y no como API abierta. Para postura motor-primero, el importador deberia degradar a `ExtCall` solo como artefacto transitorio y auditable.

### [Medium] El editor recompila y reclona estado grande en demasiados flujos

**Impacto**

La experiencia puede degradarse con grafos medianos/grandes y la mantenibilidad cae porque varias operaciones repiten el mismo pipeline de compilacion/validacion/sincronizacion.

**Evidencia**

- `crates/gui/src/editor/workbench/compile_ops.rs:37-85`
  - `run_dry_validation` recompila, reemplaza `current_script`, `last_dry_run_report` y `validation_issues`.
- `crates/gui/src/editor/workbench/compile_ops.rs:197-296`
  - `export_dry_run_repro` vuelve a compilar y vuelve a sincronizar el mismo estado.
- `crates/gui/src/editor/workbench/compile_ops.rs:251-296`
  - `build_repro_case_from_current_graph` vuelve a compilar otra vez.
- `crates/gui/src/editor/workbench/compile_ops.rs:453-502`
  - `sync_graph_to_script` repite de nuevo el pipeline completo.
- `crates/gui/src/editor/workbench/quick_fix_ops.rs:22`, `:62`, `:173`, `:213`
  - Hay clones completos de `validation_issues` y `node_graph` para planeacion, preview y revert.
- `crates/gui/src/editor/workbench/ui.rs:387`
  - El undo empuja `self.node_graph.clone()`.

**Controles existentes**

- El diseño favorece seguridad funcional: preview antes de aplicar, rollback y trazabilidad de fixes.
- Hay tests extensivos del editor y dry-run.

**Vacio actual**

El costo de esa seguridad funcional se paga con muchas copias grandes y con logica repetida entre operaciones vecinas.

**Recomendacion**

Consolidar un solo pipeline de compilacion/cache de resultado y separar:

- snapshot estructural necesario
- estado derivado recompilable
- diffs/patches pequeños para preview/autofix

### [Medium] `AssetStore` y audio runtime duplican buffers completos en cache

**Impacto**

El proyecto ya tiene cache, pero la implementacion actual copia `Vec<u8>` al leer, al insertar y al reutilizar audio. Eso reduce la ganancia real de cache en assets grandes.

**Evidencia**

- `crates/assets/src/lib.rs:189-194`
  - `ByteCache::get` devuelve `entry.data.clone()`.
- `crates/assets/src/lib.rs:335-363`
  - `load_bytes` lee bytes, verifica manifest y luego inserta `bytes.clone()` en cache.
- `crates/runtime/src/audio.rs:99-107`
  - `load_audio_bytes_cached` hace `cached.clone()` en hits y `bytes.clone()` al insertar en cache.

**Controles existentes**

- Hay presupuesto de cache.
- LRU simple y testeado.
- Los tests de cache y dedup pasan.

**Vacio actual**

El cache protege IO, pero no elimina suficientemente la presion de memoria/copia.

**Recomendacion**

Migrar estas rutas calientes a buffers compartidos (`Arc<[u8]>` o equivalente) o al menos separar cache de ownership temporal.

### [Medium] La observabilidad tradicional esta fragmentada; la trazabilidad funcional es mucho mejor

**Impacto**

El repositorio es fuerte para reproducibilidad y auditoria funcional, pero mas debil para diagnostico operativo en ejecucion real.

**Evidencia**

- Traza determinista del core:
  - `crates/core/src/trace.rs`
- Dry-run y `PhaseTrace` del editor:
  - `crates/gui/src/editor/compiler.rs:55-138`
- Reporte diagnostico rico del editor:
  - `crates/gui/src/editor/workbench/report_ops.rs:6-91`
- Trace envelope del importador y parser en validator:
  - `crates/core/src/renpy_import/decorators.rs:7-54`
  - `crates/gui/src/editor/validator/context.rs:51-150`
- Instrumentacion/logging clasico:
  - `crates/gui/src/editor/player_ui.rs:143-182`
  - `crates/gui/src/editor/player_ui.rs:334-336`
  - `crates/gui/src/Cargo.toml:16-17`
- Pero no encontre inicializacion efectiva de `tracing-subscriber` en el workspace tras busqueda repo-wide.
- Ademas, varios caminos usan `eprintln!`:
  - `crates/runtime/src/audio.rs`
  - `crates/runtime/src/lib.rs`
  - `crates/py/src/bindings/engine.rs:64`

**Controles existentes**

- Muy buena trazabilidad de dry-run, repro, import fallback y auditoria de quick-fix.
- Tests diferenciales y snapshots del trace del engine.

**Vacio actual**

- No hay una columna vertebral unica entre logs, spans, traces del core y reportes del editor.
- El sistema es excelente para QA y debugging determinista, pero no igual de fuerte para operacion y telemetria local.

**Recomendacion**

Unificar la historia de observabilidad:

- inicializar `tracing-subscriber`
- reducir `eprintln!`
- definir un contrato minimo de correlacion entre runtime/editor/import/repro

### [Low-Medium] Cobertura de benchmarks acotada al core; faltan mediciones del editor y del importador

**Impacto**

El proyecto ya mide parseo, compilacion, stepping y `apply_scene`, pero no las superficies donde hoy parece vivir el mayor costo: editor/workbench, autofix, preview, importador y cargas de assets reales.

**Evidencia**

- `crates/core/benches/core_benches.rs`
  - `parse_json_to_raw`
  - `compile_to_compiled`
  - `step_loop`
  - `choose_option`
  - `apply_scene`
- `cargo bench -p visual_novel_engine --bench core_benches -- --list` confirma esa cobertura.

**Controles existentes**

- La base de benchmarks del core es una buena primera linea.

**Vacio actual**

- No hay benchmarks dedicados para:
  - `compile_project_with_project_root`
  - `enumerate_choice_routes`
  - flujos de autofix/diff/undo
  - importacion Ren'Py con arboles de assets reales
  - cache hit/miss de audio e imagen con tamanos grandes

**Recomendacion**

Agregar mediciones sobre editor/importador antes de optimizar a ciegas.

## Fortalezas Relevantes

### Seguridad

- `AssetStore` tiene el endurecimiento mas consistente del repo:
  - saneamiento de paths
  - canonicalizacion dentro del root
  - manifest/hash/size en modo untrusted
- `SaveSlotStore` tiene recovery con backup y `script_id`.
- `export_bundle` ya prueba traversal y HMAC de bundle.

### Calidad y correccion

- `cargo check --workspace --tests` y `cargo test --workspace` pasaron completos durante la auditoria.
- Hay buena cobertura de core, gui, runtime, assets, CLI y bindings Python.
- Existen tests diferenciales entre JSON y compiled runtime.
- Hay snapshot tests para trazas y tests de regresion para quick-fixes, validator, repros e importacion.

### Trazado y QA

- El proyecto ya tiene una cultura fuerte de reproducibilidad:
  - `UiTrace`
  - dry-run reports
  - repro cases
  - envelopes de trace en `renpy import`
  - auditoria de quick-fix

## Evaluacion Especifica de `renpy import` con Criterio Motor-Primero

### Veredicto general

La orientacion actual del importador esta bastante bien alineada con el objetivo de producto que definiste:

> Ren'Py debe acomodarse a tu motor, no tu motor a Ren'Py.

El importador produce `ScriptRaw`, `ProjectManifest`, assets reescritos y `ExtCall` decorados cuando algo no encaja. Eso es, en esencia, una traduccion hacia tu IR y no una emulacion del runtime de Ren'Py.

### Mini-matriz de evaluacion

| Eje | Evaluacion | Comentario |
| --- | --- | --- |
| Seguridad de importacion | Mixta | Buen bloqueo textual de paths, pero falta contencion fuerte contra symlink/junction al copiar assets |
| Fidelidad util de migracion | Buena | Importa bastante material util y mantiene el script compilable |
| Pureza del modelo nativo | Buena con reservas | Traduce a primitivas del motor; la reserva es que los `ExtCall` decorados pueden quedarse como artefactos visibles si no se resuelven luego |
| Trazabilidad de degradaciones | Muy buena | `trace_id`, envelope v2, parser en validator, reportes y tests uno-a-uno |
| Costo de mantenimiento | Medio | La canalizacion ya tiene varias capas, y cada nueva feature Ren'Py puede empujar mas logica especial si no se cuida el scope |

### Clasificacion motor-primero

**Compatibilidad sana**

- `import_renpy_project` termina en `ScriptRaw` y `ProjectManifest`, no en un runtime Ren'Py embebido.
- El modo estricto rechaza degradaciones.
- El importador parchea targets y mantiene el output compilable.

**Degradacion aceptable**

- Los bloques no soportados se degradan a `ExtCall` decorado con envelope estructurado.
- Cada degradacion queda trazada y enlazable a `ImportIssue`.
- Tests:
  - `crates/core/src/renpy_import/tests.rs:417-470`
  - `crates/core/src/renpy_import/tests.rs:473-532`

**Fuga de modelo**

- Baja por ahora, pero existe una senal: el editor/validator sabe parsear envelopes especificos de importacion Ren'Py.
- Eso es aceptable mientras siga siendo una capa de migracion auditada y no una dependencia permanente del runtime/editor para operar historias normales.

**Riesgo operativo**

- Es el area mas urgente: paths, symlink/junction, manejo de assets y disciplina uniforme de contencion.

### Recomendacion de producto

Para sostener un criterio motor-primero en el tiempo:

1. Mantener un subset explicito de Ren'Py que mapea limpio a tu IR.
2. Degradar con trazabilidad fuerte lo que no encaje.
3. Rechazar en modo estricto lo que rompa seguridad, determinismo o pureza del modelo.
4. Evitar meter semantica Ren'Py nueva en runtime/editor salvo como metadata de migracion.

## Cobertura de Tests y Huecos

### Lo que ya esta bien cubierto

- Core engine
- saves, recovery y compatibilidad
- snapshots de trace
- differential testing JSON vs compiled
- `renpy import` funcional y de trazabilidad
- asset store y traversal
- validator, quick-fix, dry-run y repro en GUI
- contratos de audio/runtime
- bindings Python del editor

### Huecos importantes

- No vi pruebas del editor contra manifests maliciosos que intenten escapar del root via `entry_point` o locale names.
- No vi test de `renpy import` contra symlink/junction escape al copiar assets.
- No hay evidencia de pruebas de observabilidad clasica porque el logging no parece estar inicializado de forma consistente.
- Faltan benchmarks del editor/importador.

## Backlog de Remediacion

### Now

1. Endurecer resolucion de rutas en `project_io` y `load_localization_catalog`.
2. Llevar canonicalizacion `within root` al copiado de assets de `renpy import`.
3. Definir una politica de producto para `ExtCall` en embeddings no confiables.

### Next

1. Meter limite duro de bytes antes del parseo completo de script JSON.
2. Activar o estandarizar la autenticacion de saves segun modo de distribucion.
3. Unificar logging en torno a `tracing` y retirar `eprintln!` de caminos importantes.
4. Agregar benchmarks del editor/importador.

### Later

1. Reducir clones amplios del editor mediante snapshots mas pequenos o cache estructural.
2. Migrar caches de bytes a ownership compartido.
3. Revisar si los envelopes de importacion deben vivir mas en tooling/QA que en flujos persistentes del editor.

## Conclusiones

El proyecto ya tiene varias piezas de nivel producto: pruebas amplias, trazado determinista, control de integridad, bundles, importacion con reportes y una estrategia de degradacion bastante auditable. El mayor problema no es falta de ingenieria, sino falta de uniformidad: algunas capas viven con invariantes muy fuertes y otras aun no heredan la misma disciplina.

La lectura mas importante para tu objetivo es positiva: `renpy import` va en la direccion correcta para un producto motor-primero. Donde mas conviene invertir ahora es en endurecer sus bordes operativos y evitar que el editor cargue deuda accidental por trabajar con proyectos importados o manifests no confiables.
