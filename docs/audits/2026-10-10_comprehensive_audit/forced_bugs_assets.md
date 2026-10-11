# Reporte de Auditoría: Forzado de Bugs en vnengine_assets

**Área auditada:** `crates/assets` (`store.rs`, `cache.rs`, `helpers.rs`, `catalog.rs`, `model.rs`, `platform.rs`)  
**Suite de pruebas de verificación:** `crates/assets/tests/force_assets_bugs.rs`  
**Comando de ejecución:**
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p vnengine_assets --test force_assets_bugs -- --test-threads=1
```
**Resultado de la suite:** 19 pruebas ejecutadas, 19 pasadas (100% verificado, incluyendo pánico forzado con `should_panic`).

---

## Respuestas Específicas a las Preguntas de Auditoría

### 1. Path Traversal Sofisticado

* **Prefijos de dispositivos Windows (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`):**
  * **Vulnerabilidad:** En `helpers.rs` (`sanitize_rel_path`), el filtro solo descarta `Component::ParentDir | RootDir | Prefix(_)`. En Windows, los nombres de dispositivos como `"CON"`, `"AUX"`, `"NUL"`, `"COM1"` (e incluso `"CON.png"`) son tratados por `std::path::Path` como `Component::Normal`.
  * **Consecuencia:** `sanitize_rel_path(Path::new("CON"))` retorna `Ok(PathBuf::from("CON"))`.
  * Si el motor intenta abrir o leer `"CON"`, Windows bloquea el hilo indefinidamente en espera de entrada de teclado por consola (Denegación de Servicio / Hang). Abrir `"AUX"` o `"COM1"` intenta comunicarse con puertos de hardware seriales locales.
  * **Verificado en:** `bug_path_traversal_windows_device_names_not_sanitized`.

* **Rutas UNC (`\\localhost\c$\...`):**
  * `normalize_asset_request` convierte `\\` a `/`.
  * En Windows, `Path::new("//localhost/c$/...").components()` produce `Component::Prefix(UNC)`.
  * `sanitize_rel_path` detecta `Component::Prefix` y retorna `Err(AssetError::Traversal)`.
  * **Bloqueado correctamente.** Verificado en `test_path_traversal_unc_paths`.

* **Rutas con `.` y `..` mezclados (`..\\..\\`, `foo/../../bar`):**
  * Toda presencia de `ParentDir` (`..`) es detectada y rechazada con `Err(AssetError::Traversal)`.
  * **Bloqueado correctamente.** Verificado en `test_path_traversal_mixed_dots_and_backslashes`.

* **Ruta de directorio actual o vacía (`.` o `""`):**
  * **Comportamiento Anómalo:** `sanitize_rel_path(Path::new("."))` omite `CurDir` y retorna `Ok(PathBuf::from(""))`.
  * Al invocar `store.load_bytes(".")`, `canonicalize_within_root(&self.root, "")` resuelve al directorio raíz del juego. Seguidamente, `fs::read(&full_path)` intenta leer el directorio raíz como archivo binario, fallando con un error del sistema operativo (`AssetError::Io(Access is denied)` en Windows o `IsADirectory` en Unix) en lugar de un error de validación de ruta.
  * **Verificado en:** `bug_path_traversal_empty_or_curdir_path_escapes_to_root_directory`.

* **Puntos y espacios finales en Windows (`"file.png."`, `"file.png "`):**
  * Windows Win32 trunca automáticamente puntos y espacios al final de nombres de archivo. `sanitize_rel_path` no los normaliza ni rechaza.
  * `fs::read(".../portrait.png.")` abre el archivo real `portrait.png`, pero la clave del caché se almacena como `"portrait.png."`. Esto provoca duplicación de memoria en el caché y fragmentación de claves de seguridad.
  * **Verificado en:** `bug_path_traversal_trailing_dot_and_space_normalization_bypass`.

* **NTFS Alternate Data Streams (ADS):**
  * Rutas con sintaxis ADS (ej. `"assets/portrait.png::$DATA"`) pasan `sanitize_rel_path` como `Component::Normal` porque los dos puntos `:` en esa posición no se consideran prefijo de unidad de disco.
  * **Verificado en:** `bug_path_traversal_ntfs_alternate_data_streams_not_blocked`.

---

### 2. Falsos Formatos de Imagen

* **Pruebas realizadas:**
  1. Archivo de 0 bytes con extensión `.png`.
  2. 512 bytes de ceros (`0x00`) con extensión `.png`.
  3. Bytes pseudoaleatorios (`0xDE, 0xAD, 0xBE, 0xEF, ...`) con extensión `.png`.
  4. Cabecera PNG truncada (solo firma mágica `\x89PNG\r\n\x1a\n` sin chunks).
  5. Cabecera JPEG corrupta.
* **Resultado:**
  * En todos los casos, la función `decode_image_bytes` intercepta el fallo de decodificación de la biblioteca `image` y retorna limpiamente:
    ```rust
    Err(AssetError::Decode { path, reason })
    ```
  * **No se producen pánicos**, divisiones por cero ni cuelgues en decodificación de datos corruptos.
  * **Verificado en:** `test_fake_image_formats_clean_error_handling`.

---

### 3. Decompression Bomb Check

* **¿Hay verificación de dimensiones antes de intentar decodificar imágenes enormes en memoria?**
  * **NO. Existe una vulnerabilidad crítica de orden de operaciones (Pixel / Decompression Bomb).**
  * En `crates/assets/src/store.rs`, líneas 189–213 (`decode_image_bytes`):
    ```rust
    pub fn decode_image_bytes(path: &str, bytes: &[u8], limits: &AssetLimits) -> Result<LoadedImage, AssetError> {
        let image = image::load_from_memory(bytes).map_err(...)?; // <-- 1. Decodifica TODO
        let rgba = image.to_rgba8();                              // <-- 2. Aloca W * H * 4 bytes en RAM
        let (width, height) = (rgba.width(), rgba.height());
        if width > limits.max_width || height > limits.max_height { // <-- 3. VERIFICA DESPUÉS DE ALOCAR
            return Err(AssetError::InvalidDimensions { ... });
        }
        ...
    }
    ```
  * **Impacto:** Si un desarrollador configura `limits.max_width = 1920, limits.max_height = 1080`, y recibe una imagen comprimida en PNG de 10,000 $\times$ 10,000 píxeles (que en PNG ocupa escasos ~20 KB en disco):
    * `load_from_memory` y `to_rgba8()` intentan alocar **400 MB de memoria RAM**.
    * Para una imagen de 30,000 $\times$ 30,000 píxeles, intentará alocar **3.6 GB de RAM**, provocando un cuelgue inmediato por agotamiento de memoria (`OutOfMemory` abort) **antes** de que se comprueben los límites configurados.
  * **Defecto idéntico en lectura de disco (`load_bytes_shared`, `store.rs:101-107`):**
    `let bytes = fs::read(&full_path)?;` carga archivos completos de cualquier tamaño (ej. 2 GB) a la memoria heap antes de verificar `if size > self.limits.max_bytes`.
  * **Verificado en:** `bug_decompression_bomb_decodes_and_allocates_before_checking_limits` y `bug_post_read_size_check_wastes_memory_before_checking_limit`.

---

### 4. Cache de Bytes (LRU y Budget)

* **Código muerto / Error jamás retornado:**
  * En `crates/assets/src/model.rs:79` se define:
    ```rust
    #[error("asset exceeds cache budget: {bytes} bytes (budget {budget})")]
    BudgetExceeded { bytes: usize, budget: usize },
    ```
  * Este error **nunca es emitido** por `ByteCache` ni por `AssetStore`.
* **Comportamiento si un archivo excede `budget_bytes`:**
  * En `ByteCache::insert` (`cache.rs:39-41`):
    ```rust
    let bytes = data.len();
    if bytes > self.max_bytes {
        return;
    }
    ```
  * `insert` descarta silenciosamente el almacenamiento en caché del asset sin retornar error.
  * Al solicitar el archivo subsecuentemente, `store.load_bytes()` sufre un cache miss constante, releyendo el archivo de disco y volviendo a alocar un nuevo `Arc<[u8]>` en cada petición (Thrashing de I/O y memoria).
* **Corrupción de Caché por Datos Obsoletos (Stale Cache Bug):**
  * La guarda `if bytes > self.max_bytes { return; }` se ejecuta **antes** de `if let Some(old) = self.entries.remove(&key)`.
  * Si un asset ya estaba en caché (ej. 20 bytes), y se actualiza en disco a un tamaño superior al presupuesto del caché (ej. 100 bytes con budget de 50):
  * `insert` retorna anticipadamente en la línea 40.
  * La entrada antigua de 20 bytes **permanece en el caché** y se sigue sirviendo de forma obsoleta indefinidamente.
  * **Verificado en:** `bug_cache_stale_data_retained_when_updated_asset_exceeds_budget`.
* **Fuga de Memoria con Presupuesto Cero (`budget_bytes = 0`):**
  * Si el llamador configura `with_cache_budget(0)` para deshabilitar el caché:
  * Al cargar archivos vacíos de 0 bytes, `bytes (0) > max_bytes (0)` es falso y `current_bytes + bytes > max_bytes` es falso.
  * Las entradas se insertan en `self.entries` y **nunca se expulsan**, acumulando memoria sin cota.
  * **Verificado en:** `bug_cache_zero_byte_files_leak_into_zero_budget_cache`.

---

### 5. Manifest (Hashes, Rutas Inexistentes, Duplicados)

* **Evasión de Seguridad / Violación de Contrato Crítica (`require_manifest`):**
  * En `AssetStore::verify_manifest` (`store.rs:166-172`):
    ```rust
    fn verify_manifest(&self, asset_key: &str, size: u64, bytes: &[u8]) -> Result<(), AssetError> {
        if self.mode == SecurityMode::Untrusted && self.require_manifest && self.manifest.is_none() {
            return Err(AssetError::ManifestMissing);
        }
        let Some(manifest) = &self.manifest else {
            return Ok(());
        };
        ...
    }
    ```
  * Si `self.mode == SecurityMode::Trusted` pero el desarrollador configuró `require_manifest = true` y `manifest_path = None`:
  * La condición `self.mode == SecurityMode::Untrusted` es falsa.
  * La línea 170 retorna `Ok(())`.
  * **Se permite la carga de todos los archivos sin verificar manifiesto alguno**, ignorando por completo el parámetro explícito `require_manifest = true`.
  * **Verificado en:** `bug_manifest_require_manifest_bypassed_in_trusted_mode`.

* **Hashes SHA-256 alterados:**
  * Si los bytes de un archivo en disco no coinciden con el hash registrado en el manifiesto, `verify_manifest` falla de forma determinista con `AssetError::ManifestHashMismatch(asset_key)`.
  * Si el tamaño no coincide, falla con `AssetError::ManifestSizeMismatch(asset_key)`.
  * **Verificado en:** `test_manifest_tampered_hash_fails`.

* **Rutas que no existen en disco:**
  * Si el manifiesto declara un archivo inexistente en disco, `canonicalize_within_root` falla con `std::io::ErrorKind::NotFound` mapeado a `AssetError::Io(...)`.
  * **Verificado en:** `test_manifest_nonexistent_file_on_disk`.

* **Entradas duplicadas tras normalización:**
  * Rutas como `"assets/bg/./pic.png"` y `"assets/bg/pic.png"` son detectadas y rechazadas con `AssetError::ManifestDuplicateEntry`.
  * Sin embargo, en Windows NTFS (sistema de archivos case-insensitive), claves como `"bg/Pic.png"` y `"bg/pic.png"` se registran como dos entradas independientes en el `BTreeMap` del manifiesto pese a representar el mismo archivo físico.
  * **Verificado en:** `test_manifest_duplicate_normalized_entries`.

* **Clave vacía en manifiesto:**
  * La clave `""` no es rechazada en la deserialización ni sanitización del manifiesto, permitiendo registrar una entrada que intenta leer el directorio raíz.
  * **Verificado en:** `bug_manifest_allows_empty_asset_key`.

---

## Hallazgos Críticos Adicionales y Pánicos Forzados

---

### Bug 1: Pánico por Desbordamiento Aritmético en `budget_report` (CRÍTICO)

* **Mensaje exacto de pánico:**
  ```text
  thread 'panic_budget_report_arithmetic_overflow' panicked at crates\assets\src\catalog.rs:96:67:
  attempt to add with overflow
  ```
* **Ubicación:** `crates/assets/src/catalog.rs`, línea 96.
  ```rust
  pub fn budget_report(&self, budget: PlatformBudget) -> BudgetReport {
      let total_bytes = self.entries.values().map(|entry| entry.size).sum();
      ...
  ```
* **Causa Raíz:**
  `AssetFingerprintCatalog` deriva `serde::Deserialize`. Si se carga un catálogo deserializado con entradas cuyos tamaños sumen más de `u64::MAX`, o si se construye un catálogo con valores extremos, `Iterator::sum::<u64>()` utiliza suma con comprobación de desbordamiento en modo debug/test, provocando un pánico no controlado en tiempo de ejecución.
* **Propuesta de Arreglo Limpio:**
  Utilizar suma saturada en el iterador:
  ```rust
  let total_bytes = self.entries.values().map(|entry| entry.size).fold(0u64, |acc, sz| acc.saturating_add(sz));
  ```

---

### Bug 2: Contradicción de Contrato de Plataforma — Recomendación de `.webp` No Soportado

* **Comportamiento / Contradicción:**
  * En `crates/assets/src/platform.rs`, líneas 33 y 41:
    * `PlatformTarget::Mobile` configura `image_extension: "webp"`.
    * `PlatformTarget::Web` configura `image_extension: "webp"`.
  * `catalog.transcode_recommendations()` sugiere transcodificar todas las imágenes a `.webp` para estas plataformas.
  * **Sin embargo:**
    * `SUPPORTED_IMAGE_EXTENSIONS` en `model.rs:82` es rígidamente: `["png", "jpg", "jpeg"]`.
    * `Cargo.toml` de `vnengine_assets` define:
      ```toml
      image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
      ```
      (¡el feature `"webp"` no está ni siquiera habilitado!).
  * **Consecuencia:** Cuando un desarrollador sigue las recomendaciones oficiales emitidas por el motor y transcodifica sus assets a `.webp`, `AssetStore::load_image` rechaza de inmediato todas las imágenes con `AssetError::UnsupportedExtension`.
* **Verificado en:** `bug_platform_transcode_preset_recommends_unsupported_webp`.
* **Propuesta de Arreglo Limpio:**
  1. Habilitar el feature `"webp"` en `Cargo.toml` para `image`: `features = ["png", "jpeg", "webp"]`.
  2. Incluir `"webp"` en `SUPPORTED_IMAGE_EXTENSIONS` en `crates/assets/src/model.rs`.

---

### Bug 3: Vulnerabilidad de Decompression Bomb en `decode_image_bytes` (CRÍTICO)

* **Ubicación:** `crates/assets/src/store.rs`, líneas 189–213.
* **Causa Raíz:**
  No se inspeccionan las cabeceras ni las dimensiones de la imagen antes de invocar la decodificación completa y la conversión a RGBA8.
* **Propuesta de Arreglo Limpio:**
  Utilizar `image::ImageReader` con `into_dimensions()` para inspeccionar las dimensiones en tiempo $O(1)$ sin alocar el búfer de píxeles:
  ```rust
  pub fn decode_image_bytes(
      path: &str,
      bytes: &[u8],
      limits: &AssetLimits,
  ) -> Result<LoadedImage, AssetError> {
      use std::io::Cursor;
      let reader = image::ImageReader::new(Cursor::new(bytes))
          .with_guessed_format()
          .map_err(|err| AssetError::Decode {
              path: path.to_string(),
              reason: err.to_string(),
          })?;
      
      let (width, height) = reader.into_dimensions().map_err(|err| AssetError::Decode {
          path: path.to_string(),
          reason: err.to_string(),
      })?;

      if width > limits.max_width || height > limits.max_height {
          return Err(AssetError::InvalidDimensions {
              width,
              height,
              max_width: limits.max_width,
              max_height: limits.max_height,
          });
      }

      let image = image::load_from_memory(bytes).map_err(|err| AssetError::Decode {
          path: path.to_string(),
          reason: err.to_string(),
      })?;
      let rgba = image.to_rgba8();
      Ok(LoadedImage {
          name: path.to_string(),
          size: [width as usize, height as usize],
          pixels: rgba.into_raw(),
      })
  }
  ```

---

### Bug 4: Lectura Completa de Archivos antes de Comprobar Límites de Tamaño

* **Ubicación:** `crates/assets/src/store.rs`, líneas 101–108.
* **Causa Raíz:**
  `fs::read(&full_path)` carga la totalidad del archivo en la memoria RAM antes de comprobar `if size > self.limits.max_bytes`.
* **Propuesta de Arreglo Limpio:**
  Consultar los metadatos del archivo en disco antes de leer el contenido:
  ```rust
  let metadata = fs::metadata(&full_path)?;
  let size = metadata.len();
  if size > self.limits.max_bytes {
      return Err(AssetError::TooLarge {
          size,
          max: self.limits.max_bytes,
      });
  }
  let bytes = fs::read(&full_path)?;
  ```

---

### Bug 5: Omitir Filtrado de Nombres de Dispositivo Reservados de Windows

* **Ubicación:** `crates/assets/src/helpers.rs`, líneas 10–21 (`sanitize_rel_path`).
* **Causa Raíz:**
  No se contemplan los nombres de dispositivos DOS reservados en Windows (`CON`, `PRN`, `AUX`, `NUL`, `COM1`..`COM9`, `LPT1`..`LPT9`).
* **Propuesta de Arreglo Limpio:**
  ```rust
  const RESERVED_DEVICE_NAMES: &[&str] = &[
      "CON", "PRN", "AUX", "NUL",
      "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
      "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
  ];

  pub fn sanitize_rel_path(rel: &Path) -> Result<PathBuf, AssetError> {
      use std::path::Component::*;
      let mut out = PathBuf::new();
      for component in rel.components() {
          match component {
              CurDir => {}
              Normal(part) => {
                  let part_str = part.to_string_lossy();
                  let stem = part_str.split('.').next().unwrap_or("").to_ascii_uppercase();
                  if RESERVED_DEVICE_NAMES.contains(&stem.as_str()) || part_str.contains(':') {
                      return Err(AssetError::Traversal);
                  }
                  out.push(part);
              }
              ParentDir | RootDir | Prefix(_) => return Err(AssetError::Traversal),
          }
      }
      if out.as_os_str().is_empty() {
          return Err(AssetError::Traversal);
      }
      Ok(out)
  }
  ```

---

## Resumen Ejecutivo de la Auditoría

| Identificador | Severidad | Categoría | Archivo y Línea | Impacto |
| :--- | :--- | :--- | :--- | :--- |
| **Bug 1** | **Alta** | Pánico / Overflow | `catalog.rs:96` | Pánico no controlado al sumar tamaños de assets en `budget_report`. |
| **Bug 2** | **Media** | Contradicción Contrato | `platform.rs:33,41` vs `model.rs:82` | Motor recomienda transcodificar a `.webp` pero `AssetStore` lo rechaza por falta de soporte. |
| **Bug 3** | **Crítica** | DoS / Decompression Bomb | `store.rs:194-206` | Decodificación y alocación en RAM completa de imágenes antes de validar dimensiones. |
| **Bug 4** | **Alta** | DoS / Consumo de RAM | `store.rs:101-108` | Carga de archivos arbitrariamente gigantescos en memoria antes de evaluar `limits.max_bytes`. |
| **Bug 5** | **Alta** | DoS / Path Traversal | `helpers.rs:10-21` | Permite nombres de dispositivos Windows (`CON`, `PRN`, `AUX`, `NUL`), provocando cuelgues de hilo. |
| **Bug 6** | **Alta** | Evasión de Seguridad | `store.rs:166` | Parámetro `require_manifest = true` es ignorado si el modo es `Trusted`. |
| **Bug 7** | **Media** | Corrupción de Caché | `cache.rs:39-41` | Caché retiene datos viejos y obsoletos si el archivo actualizado supera el presupuesto. |
| **Bug 8** | **Baja** | Fuga de Recursos | `cache.rs:39-60` | Caché con presupuesto 0 acumula archivos de 0 bytes indefinidamente. |
| **Bug 9** | **Baja** | Código Muerto | `model.rs:79` | `AssetError::BudgetExceeded` está definido pero jamás se emite. |
