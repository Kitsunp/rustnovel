# Auditoría y Depuración de la Suite de Pruebas: `crates/core/tests`

**Repositorio:** `D:\Rust\audit\rustnovel`  
**Crate:** `visual_novel_engine` (`crates/core`)  
**Fecha:** 11 de Octubre de 2026  
**Objetivo:** Identificar aserciones tautológicas, falsos positivos, silenciamiento de errores, no determinismo, fugas de recursos e inconsistencias de contratos en la suite de pruebas existente.

---

## Resumen Ejecutivo de Hallazgos

| ID | Área / Archivo | Categoría | Severidad | Descripción Breve |
|---|---|---|---|---|
| **BUG-TEST-01** | `fuzz_tests.rs` | Falso Positivo / Silenciamiento | **CRÍTICA** | `fuzz_compile_raw_scripts` falla al compilar en el 100% de las iteraciones debido a que `ScriptRaw::arbitrary` genera etiquetas aleatorias sin `"start"`. Con `if let Ok(compiled)`, el test pasa de forma vacua sin ejecutar ninguna aserción ni prueba de roundtrip. Además, el archivo completo se omite en `cargo test` por feature gating. |
| **BUG-TEST-02** | `save_slot_store_tests.rs`, `storage_slot_contract_tests.rs` | Fuga de Archivos / Colisiones | **ALTA** | Creación manual de carpetas en `%TEMP%` sin `tempfile::TempDir`. En caso de fallo o pánico, la limpieza manual al final de la función no se ejecuta. Se confirmaron más de 92 carpetas huérfanas `vn_*` en `%TEMP%`. `now_unix_ms()` causa colisiones de nombres bajo ejecución concurrente multihilo. |
| **BUG-TEST-03** | `schema_consistency.rs` | Tautología / Efectos Secundarios | **MEDIA** | `!path.exists()` genera automáticamente el archivo de schema en el árbol de código fuente (`CARGO_MANIFEST_DIR`) y retorna `Ok` sin verificar nada. Falso positivo si se elimina el baseline y fallo en entornos CI de solo lectura. |
| **BUG-TEST-04** | `snapshot_tests.rs` (`golden_script_loop_with_condition`) | Inconsistencia de Contrato / Bucle Infinito | **ALTA** | El script del test contiene un bucle infinito real: asigna `i = 1` dentro del cuerpo en cada iteración y nunca incrementa. La condición `i < 3` jamás es falsa y `"Loop finished"` es código muerto. El test solo termina porque `run_headless(..., 10)` trunca a 10 pasos forzadamente; el snapshot golden congela una ejecución truncada. |
| **BUG-TEST-05** | `storage_slot_contract_tests.rs` | Falso Positivo / Omisión Silenciosa en Windows | **MEDIA** | Pruebas de seguridad de symlinks retornan prematuramente sin aserción en Windows cuando no se tienen privilegios elevados (`SeCreateSymbolicLinkPrivilege`), registrando un pase vacío. El archivo externo `outside.vnsav` se crea fuera del directorio raíz en `%TEMP%` y fuga en caso de fallo. |
| **BUG-TEST-06** | `differential_json_vs_compiled.rs` | Alcance Inconsistente / Falsa Cobertura | **BAJA** | El test afirma verificar paridad diferencial "JSON vs Compilado", pero el engine no tiene intérprete JSON (compila a bytecode en memoria). Solo prueba un caso trivial de 5 eventos e ignora Choice, SetFlag, Scene, Patch, Audio, ExtCall y Transition. |

---

## Detalle de Hallazgos

### 1. BUG-TEST-01: Falso Positivo Absoluto y Silenciamiento en `fuzz_tests.rs`

* **Ubicación:** `crates/core/tests/fuzz_tests.rs:73-130`
* **Mecanismo del Fallo:**
  1. `fuzz_tests.rs` está encapsulado en `#[cfg(feature = "arbitrary")] mod fuzz`. Dado que la feature `arbitrary` es opcional en `visual_novel_engine`, la ejecución estándar `cargo test -p visual_novel_engine --test fuzz_tests` ejecuta `0 passed; 0 failed`, dando la falsa impresión de que los tests de fuzzing corrieron.
  2. Al habilitarse `--features arbitrary`, `fuzz_compile_raw_scripts` ejecuta 128 iteraciones:
     ```rust
     if let Ok(script) = ScriptRaw::arbitrary(&mut u) {
         let _ = policy.validate(&script, limits);
         if let Ok(compiled) = script.compile() {
             if let Ok(bytes) = compiled.to_binary() {
                 let decoded = ScriptCompiled::from_binary(&bytes).expect(...);
                 assert_eq!(decoded.events.len(), compiled.events.len());
                 // ... todas las aserciones aquí ...
             }
         }
     }
     ```
  3. Para que `script.compile()` tenga éxito, `script.labels` debe contener obligatoriamente la clave `"start"` apuntando a un índice ejecutable.
  4. Los datos binarios arbitrarios generan cadenas Unicode aleatorias como nombres de etiquetas. La probabilidad de generar exactamente `"start"` es prácticamente nula ($1/256^5$).
  5. Por lo tanto, `script.compile()` retorna `Err(VnError::InvalidScript("missing 'start' label"))` el **100% de las veces**.
  6. Debido al `if let Ok(compiled)`, el error es silenciado y **ninguna de las aserciones se ejecuta jamás**.
* **Prueba de Reproducción:**
  Se implementó `crates/core/tests/force_verify_fuzz.rs` que comprobó:
  `arbitrary_ok: 128, compile_ok: 0`.
* **Solución Propuesta:**
  Inyectar la etiqueta `"start"` en el script antes de compilar, o implementar un generador arbitrario guiado que garantice un punto de entrada válido, y exigir explícitamente `let compiled = script.compile().expect(...)` o verificar que al menos un número significativo de casos alcanzaron el bloque de prueba:
  ```rust
  let mut script = script;
  if !script.events.is_empty() {
      script.labels.insert("start".to_string(), 0);
      let compiled = script.compile().expect("compilation of valid-labeled script must succeed");
      // ...
  }
  ```

---

### 2. BUG-TEST-02: Fuga Masiva de Archivos Temporales y Colisiones por Marcas de Tiempo en `save_slot_store_tests.rs` y `storage_slot_contract_tests.rs`

* **Ubicación:**
  * `crates/core/tests/save_slot_store_tests.rs:8-14, 54, 77, 95, 133`
  * `crates/core/tests/storage_slot_contract_tests.rs:9-14, 52, 77, 82, 110, 153, 173, 197, 231`
* **Mecanismo del Fallo:**
  1. En lugar de utilizar `tempfile::TempDir` (que ya está disponible como dev-dependency en `Cargo.toml`), ambos archivos generan rutas manuales en `%TEMP%`:
     ```rust
     let root = std::env::temp_dir().join(format!("vn_slot_store_{}", now_unix_ms()));
     ```
  2. La eliminación se realiza mediante llamadas manuales al final de cada test: `let _ = fs::remove_dir_all(root)`.
  3. Si ocurre un fallo en una aserción (`assert!`, `assert_eq!`) o un pánico antes de esa línea, el flujo salta por unwinding y `fs::remove_dir_all` nunca se ejecuta.
  4. **Evidencia empírica:** En el entorno local se encontraron **92 carpetas huérfanas `vn_*`** acumuladas en el directorio temporal de Windows (`$env:TEMP`).
  5. En `storage_slot_contract_tests.rs`, `now_unix_ms()` tiene resolución de milisegundos. Cuando cargo ejecuta los tests en paralelo (con múltiples hilos), dos pruebas que inician en el mismo milisegundo colisionan en la misma carpeta física en `%TEMP%`, provocando condiciones de carrera y escrituras cruzadas no deterministas.
  6. En las pruebas de symlinks, `outside_path` se crea con `sibling_path(&root, "outside.vnsav")` directamente en `%TEMP%`. Si la prueba falla, dicho archivo exterior queda abandonado indefinidamente.
* **Solución Propuesta:**
  Utilizar `tempfile::tempdir()` o `tempfile::TempDir::new()` en cada test. Gracias al patrón RAII / Drop guard, el directorio temporal y todos sus archivos se destruyen de forma garantizada incluso ante pánicos o fallos de aserción:
  ```rust
  let tmp = tempfile::tempdir().expect("tempdir");
  let root = tmp.path().to_path_buf();
  ```

---

### 3. BUG-TEST-03: Tautología y Mutación Silenciosa del Árbol de Fuentes en `schema_consistency.rs`

* **Ubicación:** `crates/core/tests/schema_consistency.rs:16-19`
* **Mecanismo del Fallo:**
  ```rust
  let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  path.push(format!("schema_{}.json", name));

  if env::var("UPDATE_SCHEMA").is_ok() || !path.exists() {
      fs::write(&path, schema_json).expect("failed to write schema");
      return;
  }
  ```
  1. Si un archivo de schema (`schema_event_compiled.json` o `schema_script_raw.json`) no existe (por ejemplo, en un clone limpio donde faltó incluirlo o fue eliminado por error), `!path.exists()` es verdadero.
  2. El test escribe el schema actual directamente en `crates/core/schema_{name}.json` y retorna tempranamente con éxito (`return;`).
  3. Esto constituye un **falso positivo**: la prueba pasa sin validar nada contra una versión de referencia anterior.
  4. Además, muta el árbol git de trabajo durante la ejecución normal de `cargo test`. En entornos CI con sistemas de archivos de solo lectura, falla por error de permisos de escritura (`failed to write schema`) en lugar de reportar la ausencia del archivo de esquema.
* **Solución Propuesta:**
  Exigir que la reescritura solo ocurra cuando la variable de entorno `UPDATE_SCHEMA` esté explícitamente configurada. Si el archivo no existe, la prueba debe fallar ruidosamente:
  ```rust
  if env::var("UPDATE_SCHEMA").is_ok() {
      fs::write(&path, &schema_json).expect("failed to write schema");
      return;
  }
  if !path.exists() {
      panic!("Schema baseline file not found at {:?}. Run with UPDATE_SCHEMA=1 to generate.", path);
  }
  ```

---

### 4. BUG-TEST-04: Bucle Infinito Real y Snapshot Truncado en `snapshot_tests.rs` (`golden_script_loop_with_condition`)

* **Ubicación:** `crates/core/tests/snapshot_tests.rs:41-61`
* **Mecanismo del Fallo:**
  1. El test define el script:
     ```json
     "events": [
         {"type": "set_var", "key": "i", "value": 0},
         {"type": "dialogue", "speaker": "Loop", "text": "Iteration"},
         {"type": "set_var", "key": "i", "value": 1},
         {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "i", "op": "lt", "value": 3}, "target": "loop"},
         {"type": "dialogue", "speaker": "Done", "text": "Loop finished"}
     ],
     "labels": {
         "start": 0,
         "loop": 1
     }
     ```
  2. El evento en el índice 2 asigna la constante `1` a la variable `i` en cada iteración del bucle.
  3. No existe operación de incremento (`i += 1`); por lo tanto, `i` siempre vale `1`.
  4. La condición `i < 3` se evalúa como `1 < 3`, que es **siempre verdadera**.
  5. La ejecución salta a `loop` (índice 1) indefinidamente. El evento índice 4 (`"Done: Loop finished"`) es inalcanzable (código muerto).
  6. El test no se cuelga únicamente porque invoca `run_headless(script_json, 10)`, el cual aborta forzosamente tras 10 pasos.
  7. El snapshot generado (`snapshot_tests__golden_script_loop_with_condition.snap`) congeló una secuencia interrumpida a mitad de camino (en el paso 9: `JumpIf`), institucionalizando un bucle infinito como si fuera una prueba de ciclo condicional exitosa.
* **Solución Propuesta:**
  Modificar el fixture para que las variables o condiciones simulen un avance o usar una lógica condicional finita (o si el motor no soporta operaciones aritméticas, alternar variables o flags) para que el script alcance de manera natural `"Loop finished"` y finalice por `EndOfScript`.

---

### 5. BUG-TEST-05: Omisión Silenciosa de Pruebas de Seguridad en Windows en `storage_slot_contract_tests.rs`

* **Ubicación:** `crates/core/tests/storage_slot_contract_tests.rs:210-215, 244-249`
* **Mecanismo del Fallo:**
  1. Las funciones de prueba `load_slot_rejects_symlink_primary_instead_of_external_save` y `save_slot_does_not_write_through_preexisting_tmp_symlink` intentan crear enlaces simbólicos mediante `create_file_symlink`:
     ```rust
     if !create_file_symlink(&slot_path, &outside_path) {
         eprintln!("file symlink creation not supported on this platform");
         let _ = fs::remove_dir_all(root);
         let _ = fs::remove_file(outside_path);
         return;
     }
     ```
  2. En Windows sin permisos de administrador o sin Modo de Desarrollador habilitado, la API del sistema operativo deniega la creación de symlinks (`PrivilegeNotHeld`).
  3. El test emite una línea a stderr y retorna `Ok(())` inmediatamente (`return;`).
  4. Esto significa que en máquinas Windows estándar de desarrollo o CI, **las defensas contra path traversal e inyección por symlink en el almacenamiento de partidas guardadas no se ejecutan jamás**, reportando un falso verde.
* **Solución Propuesta:**
  En plataformas o entornos donde no se soportan symlinks, registrar la prueba como ignorada condicionalmente o utilizar `cfg` / un harness adecuado en vez de silenciarla como `Ok`.

---

### 6. BUG-TEST-06: Paridad Incompleta y Cobertura Superficial en `differential_json_vs_compiled.rs`

* **Ubicación:** `crates/core/tests/differential_json_vs_compiled.rs:30-65`
* **Mecanismo del Fallo:**
  1. El archivo se llama `differential_json_vs_compiled.rs`, sugiriendo una prueba diferencial entre la ejecución directa de scripts JSON versus scripts compilados.
  2. Sin embargo, el motor no interpreta JSON en tiempo de ejecución: `Engine::new` simplemente valida y compila `ScriptRaw` en memoria a `ScriptCompiled`.
  3. La prueba solo compara una secuencia trivial de 5 eventos con una variable y un salto condicional.
  4. Tipos fundamentales del motor como `Choice`, `SetFlag`, `Scene`, `Patch`, `ExtCall`, `AudioAction` y `Transition` no son cubiertos por la verificación diferencial, dejando sin probar la equivalencia entre compilación en memoria y serialización Postcard binaria.
* **Solución Propuesta:**
  Expandir la suite diferencial para cubrir todos los tipos de eventos (`EventRaw` / `EventCompiled`) y verificar paridad exhaustiva de trazas UI y digest de estado.

---

## Verificación Experimental

Todos los tests mencionados fueron auditados y ejecutados con el toolchain estándar del entorno:
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test fuzz_tests -- --test-threads=1
cargo test -p visual_novel_engine --test force_verify_fuzz --features arbitrary -- --test-threads=1
cargo test -p visual_novel_engine --test save_slot_store_tests -- --test-threads=1
cargo test -p visual_novel_engine --test storage_slot_contract_tests -- --test-threads=1
cargo test -p visual_novel_engine --test snapshot_tests -- --test-threads=1
cargo test -p visual_novel_engine --test schema_consistency -- --test-threads=1
```

Resultados clave comprobados:
1. `fuzz_tests.rs`: Ejecuta 0 tests por defecto. Al habilitar `--features arbitrary`, `fuzz_compile_raw_scripts` pasa con 0 compilaciones exitosas de 128 (falso positivo confirmado por `force_verify_fuzz.rs`).
2. `save_slot_store_tests.rs` / `storage_slot_contract_tests.rs`: Generación de más de 92 carpetas huérfanas en `%TEMP%` confirmada vía PowerShell.
3. `snapshot_tests.rs`: Snapshot de `golden_script_loop_with_condition` confirmado como un bucle infinito que aborta en el paso 9 por límite de pasos sin alcanzar `"Done: Loop finished"`.
