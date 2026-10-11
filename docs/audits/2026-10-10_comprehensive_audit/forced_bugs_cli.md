# Reporte de Auditoría: Forzado de Bugs en vnengine CLI

**Área auditada:** `tools/cli` (`vnengine.rs`, `dispatch.rs`, `commands.rs`, `authoring.rs`, `atomic_io.rs`, `output.rs`, `package.rs`)  
**Suite de pruebas de verificación:** `tools/cli/tests/force_cli_bugs.rs`  
**Comando de ejecución:**
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p vnengine_cli --test force_cli_bugs -- --test-threads=1
```
**Resultado de la suite:** 14 pruebas ejecutadas, 14 pasadas (100% verificado y reproducible).

---

## Respuestas Específicas a las Preguntas de Auditoría

### 1. Códigos de salida en fallos (`validate`, `compile`, y supresión silenciosa de errores)

* **Validación y compilación de scripts inválidos:**
  * Se verificaron scripts con sintaxis JSON corrupta, scripts vacíos, scripts sin clave `script_schema_version`, scripts sin etiqueta `"start"`, scripts con etiquetas fuera de rango (`index > events.len()`), opciones de elección vacías, y hablante vacío con la política de seguridad predeterminada.
  * **Comportamiento en `validate` y `compile`:** Ambos subcomandos retornan un código de salida `!= 0` (específicamente código `2`, correspondiente a `"engine_error"`). No se encontró supresión silenciosa de errores en la ruta directa de `validate` o `compile` de scripts estándar.
  * **Verificado en:** `area1_validate_and_compile_exit_codes_on_invalid_scripts`.

* **BUG FORZADO #1: Supresión silenciosa y falso éxito en `vnengine theme validate`:**
  * **Comportamiento Anómalo:** Cuando se ejecuta `vnengine theme validate <theme.json>` sobre un tema con errores fatales de validación (por ejemplo, identificador de tema vacío `id: ""`, código de color inválido `colors: {"bg": "not_a_color"}`, o espaciado negativo `spacing: {"pad": -15.0}`):
    1. Sin el flag `--json`: El comando termina con **código de salida 0 (ÉXITO FALSO)** y emite exactamente **0 bytes** a `stdout` y **0 bytes** a `stderr`. Silencio absoluto.
    2. Con el flag `--json`: El sobre JSON retornado declara falsamente `"ok": true, "code": "ok"`, a pesar de que en su payload interno `data.valid` es `false` y `data.errors` contiene los 3 errores fatales de validación.
  * **Causa Raíz:** En `tools/cli/src/bin/vnengine/commands.rs:68-75`, `theme_validate_command` recibe el reporte de `validate_ui_theme(&theme)` pero nunca inspecciona `report.valid` ni `report.errors`. Retorna ciegamente `Ok(Some(serde_json::to_value(report)?))`. En `tools/cli/src/bin/vnengine.rs:385-395`, `main()` interpreta `Ok(_)` como éxito total (`ExitCode::SUCCESS`), e imprime el sobre solo si `json == true`.
  * **Verificado en:** `area1_bug_theme_validate_silent_suppression_and_fake_success`.

* **BUG FORZADO #2: Supresión total de salida en `route-tree` y `read-model` sin `--json`:**
  * **Comportamiento Anómalo:** Los comandos `vnengine route-tree <script>` y `vnengine read-model <script>` tienen como propósito documentado en su `--help`:
    * `route-tree`: *"Print the native route tree for a script."*
    * `read-model`: *"Print read/progress snapshots from a script or save."*
    Sin embargo, al ejecutarlos sin `--json`, terminan con código 0 y **no imprimen absolutamente nada a stdout ni stderr (0 bytes)**.
  * **Causa Raíz:** En `commands.rs:3-11` y `commands.rs:13-66`, ambas funciones retornan `Ok(Some(serde_json::Value))` pero no escriben a consola. En `vnengine.rs:387-394`, `main()` solo imprime `data` si `json && !suppress_envelope`. Al omitir `--json`, los datos computados son completamente descartados.
  * **Verificado en:** `area1_bug_route_tree_and_read_model_silent_output_without_json`.

---

### 2. Subcomando `trace`: Límites de pasos (`--steps 0`) y bucles infinitos

* **Comportamiento con `--steps 0`:**
  * El comando no se cuelga. Ejecuta el bucle `0..0` y genera inmediatamente el archivo de traza.
  * **BUG FORZADO #3A:** El sobre de traza resultante contiene:
    ```json
    {
      "trace_format_version": 1,
      "script_schema_version": "1.0",
      "partial": false,
      "stopped_reason": null,
      "trace": { "steps": [] }
    }
    ```
    El motor declara falsamente `"partial": false` y `"stopped_reason": null` para un script de 2 diálogos en el que se ejecutaron 0 pasos. El consumidor de la traza asume incorrectamente que el guion terminó de manera natural y completa.
  * **Verificado en:** `area2_bug_trace_steps_zero_falsely_reports_complete_trace`.

* **Comportamiento con bucles infinitos:**
  * Se probó un guion con bucle infinito `Dialogue -> Jump("start")` y un bucle cerrado de solo saltos `Jump("start")` apuntando a sí mismo.
  * **Respeto de límites:** El motor **respeta estrictamente** el límite `--steps <N>` y no se cuelga indefinidamente. Termina en milisegundos tras registrar exactamente $N$ pasos.
  * **BUG FORZADO #3B:** A pesar de haber sido interrumpido forzosamente por alcanzar el límite de pasos `--steps 10` sin haber llegado al final del guion (`position != engine.script().events.len()`), el sobre de traza generado reporta `"partial": false` y `"stopped_reason": null`. No hay indicación alguna de que la ejecución fue truncada por el límite de pasos en un bucle infinito.
  * **Causa Raíz:** En `tools/cli/src/bin/vnengine/commands.rs:214-260`, las variables `partial` y `stopped_reason` solo se modifican si ocurre un error dentro de `engine.current_event()` o `engine.step()` cuando `allow_partial_trace` está activo. Cuando el bucle `for step in 0..steps` finaliza por agotamiento de pasos, no se valida si `engine.state().position as usize == engine.script().events.len()`.
  * **Verificado en:** `area2_bug_trace_infinite_loop_falsely_reports_complete_trace` y `area2_trace_infinite_jump_loop_is_bounded_and_does_not_hang`.

---

### 3. Subcomando `verify-save`: Archivos corruptos y discrepancia de `script_id`

* **Verificación de archivo corrupto (bytes aleatorios o cabecera truncada):**
  * Retorna código de salida `!= 0` (código `1`).
  * En modo `--json`, emite un mensaje estructurado:
    ```json
    {
      "ok": false,
      "code": "error",
      "data": null,
      "error": "save data too small"
    }
    ```
  * **BUG FORZADO #4 (Clasificación de Error Inadecuada):**
    * La CLI clasifica el error como código genérico `"error"` (código de salida `1`) en lugar de `"engine_error"` (código de salida `2`).
    * **Causa Raíz:** En `tools/cli/src/bin/vnengine.rs:414-429`, la función `classify_error` únicamente busca en la cadena de errores `visual_novel_engine::VnError` y `std::io::Error`. Sin embargo, `SaveData::from_any_binary` retorna `visual_novel_engine::SaveError`, el cual es un enum independiente que no se convierte en `VnError`. Como resultado, cae en el fallback genérico `("error", 1)`.
  * **Verificado en:** `area3_verify_save_corrupt_save_file_unclassified_error_code`.

* **Verificación con `script_id` diferente:**
  * Al verificar una partida guardada legítima contra un script binario compilado diferente:
  * Retorna código de salida `1` (`!= 0`).
  * En modo `--json`, emite el mensaje: `"error": "save does not match current script"`.
  * **Verificado en:** `area3_verify_save_mismatched_script_id`.

* **Paso de script JSON en lugar de binario a `--script`:**
  * `verify-save` falla con error de formato binario: `"binary format: invalid magic bytes"`.
  * Retorna código de salida `2` (`"engine_error"`).
  * **Verificado en:** `area3_verify_save_rejects_json_script_as_compiled`.

---

### 4. Subcomando `package`: Staging temporal, residuos y propagación de errores

* **Empaquetar con runtime inexistente (`--runtime-artifact nonexistent.exe`):**
  * La CLI falla antes de escribir el paquete final y retorna código de salida `!= 0`.
  * El error describe adecuadamente el diagnóstico bloqueante: `export.runtime_artifact.unreadable`.
  * El directorio de salida `dist` no se publica en el destino final.
  * **Verificado en:** `area4_package_missing_runtime_and_missing_entry_behavior`.

* **Empaquetar con `entry-script` inexistente:**
  * Falla con código de salida `!= 0` indicando que no se pudo canonicalizar el archivo de script dentro de la raíz del proyecto.
  * No deja archivos publicados en el destino final.
  * **Verificado en:** `area4_package_missing_runtime_and_missing_entry_behavior`.

* **BUG FORZADO #5: Residuos permanentes de rollback y mensaje espurio de limpieza de staging:**
  1. **Residuos de rollback en actualizaciones:** En `crates/core/src/bundle.rs:175-182` (`publish_staged_bundle`), al actualizar un bundle existente, el directorio previo se renombra a `.<output>.rollback-<nonce>`. Si la posterior eliminación de este directorio (`fs::remove_dir_all(backup)`) falla (por ejemplo, por un archivo bloqueado en Windows por antivirus o indexador), `publish_staged_bundle` retorna `Err`, pero el directorio de respaldo queda abandonado permanentemente en el disco como residuo oculto.
  2. **Intento incondicional de limpieza en staging inexistente:** En `crates/core/src/bundle.rs:146-153` (`export_bundle_atomic`), si `export_bundle_materialized` falla antes de llamar a `fs::create_dir_all(&staging_root)` (por ejemplo, en fallos de canonicalización previa), el manejador de error ejecuta incondicionalmente `fs::remove_dir_all(&staging_root)` sin verificar `staging_root.exists()`. En Windows, esto falla con `os error 2` ("El sistema no puede encontrar el archivo especificado"), contaminando el error del usuario con la frase:
     `; additionally failed to remove staging '...': The system cannot find the path specified`.
  * **Verificado en:** `area4_package_creates_parent_directory_before_validation`.

---

### 5. Flags y argumentos inválidos (`""`, argumentos inesperados, contaminación de stdout)

* **Rutas vacías `""`:**
  * Probadas en `validate ""`, `compile "" -o out`, `compile script -o ""`, `trace "" -o out`, `trace script -o ""`, `manifest "" -o out`, `manifest root -o ""`.
  * En todos los casos, la CLI y las funciones atómicas de I/O rechazan las rutas vacías de forma controlada y retornan códigos de salida `!= 0` sin pánicos ni cuelgues.
  * **Verificado en:** `area5_empty_string_path_handling`.

* **Argumentos inesperados con flag global `--json`:**
  * Al pasar flags desconocidos junto con `--json` (ejemplo: `vnengine --json validate --nonexistent-flag`):
  * Clap intercepta el error en `Cli::parse()` antes de que `main()` tome el control.
  * Emite texto plano ANSI/ASCII a `stderr` y termina con código de salida `2`. No genera un sobre JSON, ya que Clap no implementa el formateo del envelope global de la aplicación.
  * **Verificado en:** `area5_json_flag_with_invalid_arguments_prints_raw_stderr`.

* **BUG FORZADO #6: Contaminación del flujo JSON en `vnengine --json authoring-validate`:**
  * **Comportamiento Anómalo:** Cuando se invoca `vnengine --json authoring-validate <script>`, la salida producida en `stdout` es:
    ```text
    authoring validation => issues=0 errors=0 warnings=0 infos=0
    {
      "ok": true,
      "code": "ok",
      "data": null,
      "error": null
    }
    ```
  * La salida es una mezcla de texto plano legible para humanos seguida inmediatamente por el sobre JSON.
  * **Impacto:** Cualquier herramienta automatizada, script de CI/CD o frontend que invoque el comando con `--json` e intente parsear `stdout` falla catastróficamente con error de parseo JSON (`serde_json::from_str` falla).
  * **Causa Raíz:** En `tools/cli/src/bin/vnengine/authoring.rs:188-193`, la función `validate_authoring_script` ejecuta `print_report(&report)` en `stdout` sin consultar si el usuario pidió `--json`. Por otro lado, en `tools/cli/src/bin/vnengine.rs:384`, `suppress_envelope` solo suprime el sobre para `Command::Authoring { .. }`, pero no para `Command::AuthoringValidate`. Por ende, ambos imprimen simultáneamente al mismo descriptor `stdout`.
  * **Verificado en:** `area5_bug_authoring_validate_json_stream_pollution`.

---

## Inventario Detallado de Bugs Forzados

### Bug CLI-01: Falso Éxito y Supresión Silenciosa en `theme validate`
* **Comportamiento:** Termina con código de salida `0` y salida vacía (0 bytes) ante temas con errores fatales de validación. En `--json`, emite `"ok": true, "code": "ok"`.
* **Archivo y Línea:** `tools/cli/src/bin/vnengine/commands.rs:68-75` y `tools/cli/src/bin/vnengine.rs:385-395`.
* **Causa Raíz:** `theme_validate_command` nunca evalúa si el reporte tiene errores antes de retornar `Ok(Some(...))`.
* **Propuesta de Corrección:**
```rust
// En tools/cli/src/bin/vnengine/commands.rs
pub(super) fn theme_validate_command(theme_path: &Path, json_output: bool) -> Result<Option<serde_json::Value>> {
    let raw = fs::read_to_string(theme_path).with_context(|| format!("read {}", theme_path.display()))?;
    let theme: UiTheme = serde_json::from_str(&raw).with_context(|| format!("parse theme {}", theme_path.display()))?;
    let report = validate_ui_theme(&theme);
    if !json_output {
        println!("theme validation => valid={} errors={} warnings={}", report.valid, report.errors.len(), report.warnings.len());
        for err in &report.errors {
            eprintln!("error: {err}");
        }
    }
    if !report.valid {
        anyhow::bail!("theme validation failed with {} error(s)", report.errors.len());
    }
    Ok(Some(serde_json::to_value(report)?))
}
```

---

### Bug CLI-02: Supresión Silenciosa de `route-tree` y `read-model` sin `--json`
* **Comportamiento:** Ambos comandos terminan con código 0 y 0 bytes de salida en consola cuando no se pasa el flag `--json`.
* **Archivo y Línea:** `tools/cli/src/bin/vnengine.rs:385-395` y `tools/cli/src/bin/vnengine/commands.rs:3-66`.
* **Causa Raíz:** `main()` solo imprime `data` si `json == true`. Los comandos retornan `Ok(Some(data))` pero no escriben nada en texto plano.
* **Propuesta de Corrección:**
```rust
// En tools/cli/src/bin/vnengine.rs:385
match run_command(cli, json) {
    Ok(data) => {
        if json && !suppress_envelope {
            print_json_envelope(CliEnvelope {
                ok: true,
                code: "ok".to_string(),
                data,
                error: None,
            });
        } else if let Some(val) = data {
            // Imprimir representación formateada si el usuario no solicitó envelope JSON
            println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
        }
        ExitCode::SUCCESS
    }
```

---

### Bug CLI-03: `trace` Falsamente Certifica Finalización Completa en `--steps 0` y Bucles Infinitos
* **Comportamiento:** El sobre de traza reporta `"partial": false` y `"stopped_reason": null` aun cuando la traza fue truncada por el límite de pasos.
* **Archivo y Línea:** `tools/cli/src/bin/vnengine/commands.rs:214-260`.
* **Causa Raíz:** El ciclo `for step in 0..steps` no actualiza `partial` si finaliza por alcanzar `steps` en lugar de por llegar al final de los eventos del script.
* **Propuesta de Corrección:**
```rust
// En tools/cli/src/bin/vnengine/commands.rs:253
    let reached_end = engine.state().position as usize >= engine.script().events.len();
    if !reached_end && !partial {
        partial = true;
        stopped_reason = Some(format!("execution stopped after requested step limit ({steps} steps) without reaching end of script"));
    }
    let envelope = TraceEnvelope {
        trace_format_version: 1,
        script_schema_version: SCRIPT_SCHEMA_VERSION.to_string(),
        partial,
        stopped_reason,
        trace,
    };
```

---

### Bug CLI-04: Contaminación de la Salida Estándar JSON en `authoring-validate`
* **Comportamiento:** `vnengine --json authoring-validate <script>` mezcla texto plano con el sobre JSON en `stdout`, rompiendo la compatibilidad con analizadores de JSON.
* **Archivo y Línea:** `tools/cli/src/bin/vnengine/authoring.rs:188-193` y `tools/cli/src/bin/vnengine.rs:384`.
* **Causa Raíz:** `validate_authoring_script` ejecuta `print_report` incondicionalmente, y `main()` no suprime el sobre para `Command::AuthoringValidate`.
* **Propuesta de Corrección:**
```rust
// En tools/cli/src/bin/vnengine.rs:384
let suppress_envelope = matches!(&cli.command, Command::Authoring { .. } | Command::AuthoringValidate { .. });

// O en tools/cli/src/bin/vnengine/dispatch.rs:
Command::AuthoringValidate { script, project_root, output } => {
    authoring::validate_authoring_script(&script, project_root.as_deref(), output.as_deref(), json_output)?;
    Ok(None)
}
```

---

### Bug CLI-05: Clasificación Errónea de `SaveError` como Error Genérico
* **Comportamiento:** `verify-save` ante partidas corruptas o claves inválidas retorna código 1 y sobre `"code": "error"` en lugar de `"code": "engine_error"` (código 2).
* **Archivo y Línea:** `tools/cli/src/bin/vnengine.rs:414-429`.
* **Causa Raíz:** `classify_error` no reconoce `visual_novel_engine::SaveError`.
* **Propuesta de Corrección:**
```rust
// En tools/cli/src/bin/vnengine.rs:414
fn classify_error(err: &anyhow::Error) -> (&'static str, u8) {
    if err.chain().any(|cause| {
        cause.downcast_ref::<visual_novel_engine::VnError>().is_some()
            || cause.downcast_ref::<visual_novel_engine::SaveError>().is_some()
    }) {
        ("engine_error", 2)
    } else if err.chain().any(|cause| cause.downcast_ref::<std::io::Error>().is_some()) {
        ("io_error", 3)
    } else {
        ("error", 1)
    }
}
```

---

### Bug CLI-06: Mensaje Espurio de Fallo de Limpieza de Staging en `package`
* **Comportamiento:** Si el plan o empaquetado falla antes de crear la carpeta de staging, el error al usuario se contamina con: `; additionally failed to remove staging '...': The system cannot find the path specified`.
* **Archivo y Línea:** `crates/core/src/bundle.rs:146-151`.
* **Causa Raíz:** `export_bundle_atomic` llama a `fs::remove_dir_all(&staging_root)` sin comprobar si el directorio llegó a crearse.
* **Propuesta de Corrección:**
```rust
// En crates/core/src/bundle.rs:146
        Err(err) => {
            if staging_root.exists() {
                if let Err(cleanup_err) = fs::remove_dir_all(&staging_root) {
                    return Err(invalid_bundle(format!(
                        "{err}; additionally failed to remove staging '{}': {cleanup_err}",
                        staging_root.display()
                    )));
                }
            }
            Err(err)
        }
```

---

## Conclusión

La auditoría de la herramienta `vnengine` revela que las rutas centrales de validación y compilación de scripts en formato JSON son robustas ante entradas corruptas (código de salida 2). No obstante, se forzaron y verificaron **seis bugs críticos de diseño y contrato** en la interfaz CLI:
1. Supresión silenciosa total y falso éxito (código 0) en `theme validate`.
2. Supresión total de salida en `route-tree` y `read-model` en modo texto estándar.
3. Falsificación de trazas completas (`partial: false`, `stopped_reason: null`) en scripts truncados o bucles infinitos con `--steps`.
4. Corrupción del stream JSON en `authoring-validate` por mezcla de texto plano y sobre JSON en `stdout`.
5. Omisión de `SaveError` en el clasificador de códigos de salida.
6. Fallo espurio de limpieza de staging en `package` por invocación incondicional de `remove_dir_all`.

Todas las vulnerabilidades y fallas de contrato fueron aisladas, reproducidas y demostradas mediante la suite `tools/cli/tests/force_cli_bugs.rs`.
