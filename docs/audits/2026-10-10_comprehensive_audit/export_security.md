# Auditoría de Seguridad: Exportación, Generación de Launchers, Integridad y AssetStore (`export_security`)

**Fecha:** 11 de Octubre de 2026  
**Auditor:** Senior Rust Security Auditor  
**Workspace:** `rustnovel` (`crates/core`, `crates/assets`, `tools/cli`)  
**Archivos Auditados:**
- `crates/core/src/bundle.rs`, `crates/core/src/bundle/**` (`materialize.rs`, `plan.rs`, `helpers.rs`, `assets.rs`, `reporting.rs`, `spec.rs`)
- `crates/core/src/authoring/validation/assets.rs` (`is_unsafe_asset_ref`)
- `crates/assets/src/**` (`store.rs`, `helpers.rs`, `model.rs`, `lib.rs`)
- `tools/cli/src/bin/vnengine/**` (`package.rs`, `commands.rs`)

---

## 1. Resumen Ejecutivo

Se ha completado una auditoría exhaustiva de seguridad centrada en la exportación de paquetes (bundles), generadores de scripts de lanzamiento (`launch.bat`, `launch.sh`), verificación criptográfica (HMAC-SHA256), manejo y validación de assets en `AssetStore`, y resiliencia transaccional (staging y rollback).

Durante la auditoría se identificaron y **confirmaron mediante 22 pruebas automatizadas** vulnerabilidades críticas y de severidad alta/media:
1. **Inyección de comandos y fragilidad en generadores de launchers (`write_launcher`)**: `write_launcher` interpola sin sanitización nombres de ejecutables dentro de scripts `.sh` y `.bat`. El cambio no commiteado en `bundle.rs` (`let launch_target = executable_rel.as_deref()`) parcheó un síntoma superficial pero dejó `write_launcher` indefenso y causó una regresión funcional en runtimes no estándar.
2. **Consumo no controlado de memoria (DoS / CWE-400) en `AssetStore`**: `load_bytes_shared` lee el archivo completo en memoria con `fs::read` antes de verificar el límite de tamaño `limits.max_bytes`.
3. **Bomba de descompresión (CWE-409) en decodificación de imágenes**: `decode_image_bytes` asigna memoria proporcional a `width * height * 4` antes de validar los límites de dimensiones.
4. **Evasión de modo `SecurityMode::Untrusted` en `AssetStore`**: Si `require_manifest == false`, el almacén no exige manifiesto y carga archivos arbitrarios sin verificación.
5. **Evasión de lista blanca de extensiones para imágenes**: Archivos sin extensión pueden resolverse y cargarse como imágenes en `resolve_image_path`.
6. **Bypasses en `is_unsafe_asset_ref`**: No detecta Alternate Data Streams (ADS en NTFS con `:` en posiciones intermedias), caracteres nulos (`\0`), ni secuencias `...`.
7. **Filtro defectuoso en `validate_export_plan`**: Ignora diagnósticos marcados con `blocking_release: true` si su severidad es `"warning"` (ej. incompatibilidad de arquitectura de runtime).
8. **Fuga de directorios staging (`.staging-*`)** en fallos durante la fase de publicación transaccional.
9. **Ausencia de verificación de integridad HMAC en tiempo de ejecución**: El motor genera firmas HMAC para el manifiesto, pero no existe ninguna función en el runtime ni comando CLI para verificar la autenticidad del paquete antes o durante el juego.

---

## 2. Bugs CONFIRMADOS

### BUG-EXP-01: Inyección de Comandos en Generador de Launchers (`write_launcher`) y Cambio Incompleto en `bundle.rs`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/core/src/bundle/materialize.rs:104-142`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle/materialize.rs#L104-L142) y [`crates/core/src/bundle.rs:353-358`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle.rs#L353-L358)
- **Descripción:**
  `write_launcher` genera scripts `.bat` para Windows y `.sh` para Linux/macOS. Interpola el parámetro `runtime_rel` sin ninguna sanitización:
  ```rust
  // Windows:
  format!("@echo off\r\nsetlocal\r\n\"%~dp0{runtime}\" ... %*\r\n")
  // Linux/macOS:
  format!("exec \"$DIR/{runtime}\" ... \"$@\"\n")
  ```
  - En Linux/macOS (`sh`), comillas dobles no evitan la sustitución de comandos `$(...)` ni la ejecución por comillas invertidas `` `...` ``.
  - En Windows (`cmd.exe`), comillas dobles no impiden la expansión de variables `%VAR%`, y comillas adicionales permiten inyectar comandos con `&` o `|`.
  
  **Análisis del cambio no commiteado en `bundle.rs`:**
  El cambio no commiteado eliminó el fallback que asignaba `launch_target = runtime_artifact.rel_path`:
  ```diff
  - let launch_target = executable_rel.as_deref().or_else(|| {
  -     runtime_artifact.as_ref().map(|runtime| runtime.rel_path.as_str())
  - });
  + let launch_target = executable_rel.as_deref();
  ```
  *Problemas derivados:*
  1. `write_launcher` sigue completamente expuesto: cualquier llamada directa o futura con un `runtime_rel` arbitrario reabre la inyección inmediatamente.
  2. Regresión funcional: Si el usuario exporta con un runtime personalizado válido que no se llame `game.exe` o no coincida con el parser PE/ELF estándar, el launcher se genera diciendo permanentemente `"Runtime artifact missing in bundle"` a pesar de que el binario fue copiado exitosamente en `runtime/`.
- **Reproducción:**
  Test `test_launcher_command_injection_characters_handled` y `test_launcher_linux_sh_injection_handled` en `crates/core/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Sanitizar estrictamente `runtime_rel` en `write_launcher`:
  ```rust
  if let Some(runtime) = runtime_rel {
      if !runtime.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '\\')) {
          return Err(invalid_bundle("runtime path contains unsafe characters"));
      }
      if runtime.contains("..") || runtime.contains('%') || runtime.contains('$') {
          return Err(invalid_bundle("runtime path contains illegal shell characters"));
      }
  }
  ```

---

### BUG-EXP-02: `AssetStore` Consumo Ilimitado de Memoria (CWE-400 / DoS) previo a la Validación de Tamaño
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/assets/src/store.rs:101-108`](file:///D:/Rust/audit/rustnovel/crates/assets/src/store.rs#L101-L108)
- **Descripción:**
  En `AssetStore::load_bytes_shared`:
  ```rust
  let bytes = fs::read(&full_path)?;
  let size = bytes.len() as u64;
  if size > self.limits.max_bytes {
      return Err(AssetError::TooLarge { size, max: self.limits.max_bytes });
  }
  ```
  El método lee el archivo completo en memoria con `fs::read` ANTES de comprobar el límite `self.limits.max_bytes`. Si se solicita un archivo de varios gigabytes, el proceso sufrirá un pánico por agotamiento de memoria (OOM), vulnerabilidad clásica de Denegación de Servicio (DoS).
- **Reproducción:**
  Test `test_asset_store_size_limit_rejection` en `crates/assets/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Inspeccionar la longitud de los metadatos antes de leer los bytes:
  ```rust
  let metadata = fs::metadata(&full_path)?;
  let size = metadata.len();
  if size > self.limits.max_bytes {
      return Err(AssetError::TooLarge { size, max: self.limits.max_bytes });
  }
  ```
  Y para prevenir TOCTOU, limitar la lectura con `Read::take(self.limits.max_bytes + 1)`.

---

### BUG-EXP-03: `AssetStore` Bypass de Seguridad en `SecurityMode::Untrusted` si `require_manifest == false`
- **Severidad:** Alta
- **Archivo:Línea:** [`crates/assets/src/store.rs:166-172`](file:///D:/Rust/audit/rustnovel/crates/assets/src/store.rs#L166-L172)
- **Descripción:**
  En `verify_manifest`:
  ```rust
  if self.mode == SecurityMode::Untrusted && self.require_manifest && self.manifest.is_none() {
      return Err(AssetError::ManifestMissing);
  }
  let Some(manifest) = &self.manifest else {
      return Ok(());
  };
  ```
  Si se inicializa un almacén con `SecurityMode::Untrusted` pero `require_manifest == false` y `manifest_path == None`:
  1. La condición de la línea 166 no se cumple porque `require_manifest` es `false`.
  2. La línea 170 retorna `Ok(())` porque `manifest` es `None`.
  Como resultado, un `AssetStore` en modo no confiable carga cualquier archivo de disco sin requerir ni verificar hashes SHA-256 ni tamaños.
- **Reproducción:**
  Test `test_asset_store_untrusted_mode_bypassed_when_require_manifest_false` en `crates/assets/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  En modo no confiable, el manifiesto debe ser estrictamente obligatorio:
  ```rust
  if self.mode == SecurityMode::Untrusted && (self.require_manifest || self.manifest.is_none()) {
      if self.manifest.is_none() {
          return Err(AssetError::ManifestMissing);
      }
  }
  ```

---

### BUG-EXP-04: Bomba de Descompresión en Decodificación de Imágenes (CWE-409)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/assets/src/store.rs:198-207`](file:///D:/Rust/audit/rustnovel/crates/assets/src/store.rs#L198-L207)
- **Descripción:**
  En `decode_image_bytes`:
  ```rust
  let image = image::load_from_memory(bytes)...;
  let rgba = image.to_rgba8();
  let (width, height) = (rgba.width(), rgba.height());
  if width > limits.max_width || height > limits.max_height { ... }
  ```
  `image.to_rgba8()` descomprime la imagen en memoria plana (`width * height * 4` bytes) antes de verificar si excede `max_width` o `max_height`. Una imagen PNG pequeña (ej. 50 KB) configurada a 30,000 x 30,000 píxeles forzará una asignación inmediata de 3.6 GB de RAM antes de que el límite la rechace.
- **Reproducción:**
  Test `test_decode_image_bytes_dimension_limit_checked` en `crates/assets/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Validar las dimensiones directamente en el contenedor `image` antes de llamar a `to_rgba8()`:
  ```rust
  let (width, height) = (image.width(), image.height());
  if width > limits.max_width || height > limits.max_height {
      return Err(AssetError::InvalidDimensions {
          width, height, max_width: limits.max_width, max_height: limits.max_height,
      });
  }
  let rgba = image.to_rgba8();
  ```

---

### BUG-EXP-05: `is_unsafe_asset_ref` Omite Verificación de ADS (Alternate Data Streams), Null Bytes y `...`
- **Severidad:** Media
- **Archivo:Línea:** [`crates/core/src/authoring/validation/assets.rs:120-137`](file:///D:/Rust/audit/rustnovel/crates/core/src/authoring/validation/assets.rs#L120-L137)
- **Descripción:**
  La función `is_unsafe_asset_ref(path)` tiene múltiples agujeros de validación:
  1. Solo busca dos puntos `:` en el índice 1: `path.chars().nth(1).is_some_and(|s| s == ':')`. Rutas NTFS ADS como `assets/image.png:secret` o `assets/bg.jpg::$DATA` no se consideran inseguras.
  2. No verifica bytes nulos (`\0`).
  3. Solo comprueba `part == ".."` en los componentes, omitiendo secuencias anómalas como `...` o `....`.
  4. Rutas vacías `""` retornan `false`.
- **Reproducción:**
  Test `test_is_unsafe_asset_ref_bypasses` en `crates/core/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Rechazar cualquier ruta que contenga `:`, caracteres de control (`\0`, `\r`, `\n`) o componentes con más de dos puntos seguidos.

---

### BUG-EXP-06: `AssetStore` Omite Validación de Extensión en Archivos sin Extensión (`resolve_image_path`)
- **Severidad:** Media
- **Archivo:Línea:** [`crates/assets/src/store.rs:128-133`](file:///D:/Rust/audit/rustnovel/crates/assets/src/store.rs#L128-L133) y [`crates/assets/src/helpers.rs:86-112`](file:///D:/Rust/audit/rustnovel/crates/assets/src/helpers.rs#L86-L112)
- **Descripción:**
  `resolve_image_path` solo verifica `allowed_image_extensions` si `rel.extension()` es `Some`. Si en el directorio de assets existe un archivo sin extensión llamado `payload`, candidate 0 es `"payload"`. Al carecer de extensión, omite el whitelist y `resolve_image_path` retorna `Ok("payload")`, permitiendo cargar archivos binarios o ejecutables como supuestas imágenes.
- **Reproducción:**
  Test `test_asset_store_resolve_image_extensionless_file_bypass` en `crates/assets/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Exigir que todo candidato resuelto deba finalizar obligatoriamente con una extensión permitida en `self.allowed_image_extensions`.

---

### BUG-EXP-07: `validate_export_plan` Ignora Diagnósticos con `blocking_release: true` si `severity == "warning"`
- **Severidad:** Media
- **Archivo:Línea:** [`crates/core/src/bundle.rs:59-67`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle.rs#L59-L67) y [`crates/core/src/bundle/plan.rs:170-171`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle/plan.rs#L170-L171)
- **Descripción:**
  En `validate_export_plan`, el filtro de errores solo busca `diagnostic.severity == "error"`.
  En `plan.rs`, el diagnóstico `export.runtime_artifact.target_mismatch` (cuando el ejecutable es de otra plataforma o arquitectura) tiene `severity: "warning"`, pero `blocking_release: true`.
  Por tanto, `validate_export_plan` devuelve `Ok(())`, permitiendo exportar bundles destinados a fallar en ejecución.
- **Reproducción:**
  Test `test_validate_export_plan_ignores_blocking_release_warnings` en `crates/core/tests/audit_export_security_tests.rs`.
- **Fix sugerido:**
  Filtrar por:
  ```rust
  let blocking_errors = plan.diagnostics.iter().filter(|d| d.severity == "error" || d.blocking_release).collect::<Vec<_>>();
  ```

---

### BUG-EXP-08: Fuga de Directorio Staging (`.staging-*`) en Fallo de `publish_staged_bundle`
- **Severidad:** Baja
- **Archivo:Línea:** [`crates/core/src/bundle.rs:142-154`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle.rs#L142-L154)
- **Descripción:**
  En `export_bundle_atomic`, si `publish_staged_bundle` falla (por ejemplo, colisión de nombre de rollback o permisos), el operador `?` retorna el error inmediatamente sin ejecutar `fs::remove_dir_all(&staging_root)`. El directorio temporal `.{output}.staging-{uuid}` queda abandonado en el sistema de archivos permanentemente.
- **Fix sugerido:**
  Asegurar la eliminación de `staging_root` ante cualquier fallo de `publish_staged_bundle`.

---

## 3. Sospechas

1. **Ausencia Total de Verificación de Integridad HMAC en Runtime (Feature Huérfana)**
   - **Evidencia:** [`crates/core/src/bundle.rs:434-458`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle.rs#L434-L458), [`crates/core/src/lib.rs`](file:///D:/Rust/audit/rustnovel/crates/core/src/lib.rs).
   - **Sospecha:** La exportación con `BundleIntegrity::HmacSha256` calcula y escribe `meta/bundle.hmac_sha256` y `meta/bundle_file_manifest.json`. Sin embargo, no existe ninguna función en el motor (`Engine`), ni en `vnengine_cli`, ni en `vn_player` que lea o verifique dicha firma antes de ejecutar el juego. Un atacante puede alterar los scripts o assets en el paquete distribuido y el juego se ejecutará sin ninguna alerta de integridad.
2. **`assets_manifest.json` no cuenta con firma criptográfica propia**
   - **Evidencia:** [`crates/core/src/bundle.rs:325-339`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle.rs#L325-L339) y `AssetStore` en [`crates/assets/src/store.rs:165-186`](file:///D:/Rust/audit/rustnovel/crates/assets/src/store.rs#L165-L186).
   - **Sospecha:** `AssetStore` valida los assets contra `meta/assets_manifest.json`, pero este archivo no está firmado. Si un atacante reemplaza un asset y modifica `assets_manifest.json` con el nuevo SHA-256 y tamaño, `AssetStore` aceptará el asset alterado sin advertencia.
3. **Duplicación de Archivos Binarios en el Paquete Final**
   - **Evidencia:** [`crates/core/src/bundle/materialize.rs:46-53`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle/materialize.rs#L46-L53) y [`92-99`](file:///D:/Rust/audit/rustnovel/crates/core/src/bundle/materialize.rs#L92-L99).
   - **Sospecha:** Cada exportación copia el binario a `output/runtime/<archivo>` y luego lo vuelve a copiar a `output/game.exe` (o `game`). En juegos comerciales esto duplica innecesariamente 50 a 100 MB de binarios en cada distribución.

---

## 4. Simplificaciones

1. **Unificar `sanitize_relative_path` y `sanitize_rel_path`**:
   Existe una implementación en `crates/core/src/bundle/helpers.rs` y otra en `crates/assets/src/helpers.rs`. La de `crates/assets` no comprueba rutas vacías (`""`), lo que derivó en el BUG-EXP-06. Ambas deben unificarse en una función canónica.
2. **Eliminar campos redundantes/muertos `warnings` y `errors` en `ExportPlan`**:
   En `crates/core/src/bundle/plan.rs:257-258`, los vectores `warnings` y `errors` se inicializan siempre vacíos (`Vec::new()`), mientras que los diagnósticos reales se guardan en `diagnostics`. Esto genera confusión en la API pública.
3. **Optimizar `publish_staged_bundle`**:
   Simplificar el flujo de staging utilizando un directorio temporal dentro del target y un solo movimiento atómico con fallback limpio.

---

## 5. Mejoras

1. **Comando CLI de Verificación**:
   Agregar a `vnengine_cli` un subcomando `vnengine verify-bundle --root <PATH> [--hmac-key <KEY>]` que valide todos los archivos del paquete contra `bundle_file_manifest.json` y verifique la firma HMAC.
2. **Hardening de `launch.bat` y `launch.sh`**:
   En lugar de generar rutas variables, hardcodear la invocación relativa a `"game.exe"` / `"game"` exclusivamente, o invocar directamente el ejecutable sin scripts intermedios cuando sea posible.
3. **Validación de Límites en Stream**:
   En `AssetStore::load_bytes_shared`, utilizar un lector con límite (`Read::take`) para abortar tan pronto se superen los bytes permitidos, protegiendo la memoria en tiempo real.

---

## 6. Tests Añadidos y Cómo Ejecutarlos

Se han creado dos suites completas de pruebas de seguridad:

1. **`crates/core/tests/audit_export_security_tests.rs`** (14 pruebas):
   - Path traversal en `entry_script` (`..`, absoluto, UNC, vacío).
   - Path traversal en `runtime_artifact`.
   - Evasiones confirmadas de `is_unsafe_asset_ref` (ADS, null bytes, triple dots, vacío).
   - Transaccionalidad de exportación (limpieza de staging y preservación de bundles existentes).
   - Inyección de comandos en `launch.bat` y `launch.sh`.
   - Falta de verificación de HMAC en el runtime y desprotección de `assets_manifest.json`.
   - Diagnósticos de bloqueo en `validate_export_plan`.

2. **`crates/assets/tests/audit_export_security_tests.rs`** (8 pruebas):
   - Traversal y lectura anómala de directorios por ruta vacía en `load_bytes`.
   - Agotamiento de memoria / límite de tamaño en `AssetStore`.
   - Bypass de `SecurityMode::Untrusted` sin manifiesto.
   - Bypass de extensiones de imagen en archivos sin extensión.
   - Decompression bomb / asignación previa a comprobación de dimensiones.
   - Detección de discrepancias de hash y tamaño en manifiestos.

### Instrucciones de Ejecución (PowerShell):
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";

# Ejecutar tests de visual_novel_engine (crates/core)
cargo test -p visual_novel_engine --test audit_export_security_tests

# Ejecutar tests de vnengine_assets (crates/assets)
cargo test -p vnengine_assets --test audit_export_security_tests
```

**Resultado de ejecución:**
- `visual_novel_engine`: **14 pasados, 0 fallados**.
- `vnengine_assets`: **8 pasados, 0 fallados**.
- `cargo clippy` en `--lib` / `--bin`: **0 advertencias / 0 errores**.
