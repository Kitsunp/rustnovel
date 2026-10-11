# Reporte de Auditoría: Forzado de Bugs en Timeline y Animación

**Área auditada:** `crates/core/src/timeline` (`timeline.rs`, `timeline/types.rs`)  
**Suite de pruebas de verificación:** `crates/core/tests/force_timeline_bugs.rs`  
**Comando de ejecución:**
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test force_timeline_bugs -- --test-threads=1
```
**Resultado de la suite:** 13 pruebas ejecutadas, 13 pasadas (reproducción y forzado 100% verificado).

---

## Respuestas Específicas a las Preguntas de Auditoría

### 1. Keyframes en el mismo tick
* **¿Qué pasa al agregar dos keyframes con el mismo tick mediante la API?**
  * `Track::add_keyframe` verifica `self.keyframes.iter().any(|k| k.time == kf.time)` y retorna un error explícito: `Err(TimelineError::DuplicateKeyframeTime { time })`. No se sobreescriben ni causan pánico inmediato en la API pública de inserción.
* **Vulnerabilidad por Deserialización (Serde):**
  * `Track` deriva `Deserialize` sin validación. Si un archivo JSON/binario contiene keyframes con ticks duplicados (ej. tick 10 con valor 0 y tick 10 con valor 100), la deserialización es exitosa. Al evaluar `track.evaluate(10)`, la condición `time <= keyframes[0].time` se activa de inmediato retornando el valor del primer keyframe (0). El segundo keyframe queda completamente sombreado e inaccesible.

### 2. Interpolación con ticks iguales
* **¿Hay división por cero `(current - start) / (end - start)` si inicio y fin caen en el mismo tick?**
  * **No hay división por cero**. En `Track::interpolate`, existe una guarda explícita:
    ```rust
    let duration = k1.time - k0.time;
    if duration == 0 {
        return k1.value;
    }
    ```
    Además, en `Track::evaluate`, el bucle de búsqueda de segmento requiere `time >= k0.time && time < k1.time`. Si `k0.time == k1.time`, la condición es matemáticamente imposible (`time >= X && time < X`), por lo que nunca se invoca `interpolate` con ticks idénticos.
  * Por último, en `Fixed::div`, existe la guarda `if rhs.0 == 0 { return Self::ZERO; }`.
* **Comportamiento anómalo con keyframes desordenados:**
  * Si se deserializan keyframes con `k0.time > k1.time`, `evaluate` no encuentra el segmento y salta al fallback `Some(last.value)`, omitiendo toda interpolación e ignorando los valores intermedios.

### 3. Easing numérico extremo
* **¿Hay overflow aritmético o NaN al evaluar con valores extremos (`i32::MAX`, negativos extremos)?**
  * **NaN:** No se genera `NaN` porque el sistema utiliza aritmética entera de punto fijo (`Fixed(i32)`) y no flotantes.
  * **Pánico por Overflow Aritmético:** Las funciones de easing por sí solas no entran en pánico porque `Fixed::Add`/`Sub` usan `saturating_add`/`saturating_sub`, y `Fixed::Mul` opera internamente en `i64` antes de truncar con `as i32`.
  * **Distorsión Severa por Truncamiento de Bits:**
    * `Easing::EaseIn.apply(Fixed(i32::MAX))` produce `-65536` (`-1.0` en punto fijo) en lugar de un número positivo enorme.
    * `Easing::EaseIn.apply(Fixed(i32::MIN))` produce `0`.
    * `Easing::EaseInOut.apply(Fixed(i32::MAX))` produce `-4` (un valor negativo).
    * `Easing::EaseInOut.apply(Fixed::from_int(10))` produce un valor negativo (`-161.0` en punto fijo) debido al desbordamiento en la multiplicación de `i32`.

### 4. Seeking
* **Seeking a ticks negativos / ticks extremos:**
  * `Timeline::seek(time: u32)` toma `u32`, por lo que un tick negativo ingresado como `(-1i32) as u32` se interpreta como `u32::MAX` (`4_294_967_295`).
  * `Timeline::advance(delta_ticks: u32)` utiliza `saturating_add`, manteniéndose saturado en `u32::MAX` sin entrar en pánico.
  * `Timeline::seek` más allá de `timeline.duration()` no está acotado ni validado; `evaluate()` retorna el valor del último keyframe para cualquier tiempo posterior a la duración.
* **Vulnerabilidad de `ticks_per_second`:**
  * `Timeline::new(0)` permite inicializar la línea de tiempo con 0 ticks por segundo. Cualquier cálculo downstream que convierta duración en ticks a segundos (`timeline.duration() / timeline.ticks_per_second`) causa pánico por división por cero en tiempo de ejecución.

### 5. Orden determinista de pistas
* **¿La evaluación produce siempre el mismo orden determinista?**
  * **No**. `Timeline::evaluate()` devuelve un `Vec<(EntityId, PropertyType, i32)>` en el orden de inserción de las pistas en `self.tracks`.
  * Si dos instancias agregan las pistas en orden distinto (ej. `[Entity(1), Entity(2)]` vs `[Entity(2), Entity(1)]`), las listas resultantes de `evaluate()` son diferentes.
  * Además, `Timeline::add_track` **permite pistas duplicadas** para la misma entidad y propiedad (`EntityId`, `PropertyType`). Si se agregan dos pistas para `(Entity(42), PositionX)`, `evaluate()` emitirá dos valores contradictorios en el mismo tick.

---

## Hallazgos Críticos y Bugs Forzados

---

### Bug 1: Pánico por Overflow Aritmético en `Fixed::lerp` (CRÍTICO)

* **Comportamiento / Mensaje exacto de pánico:**
  ```text
  thread '...' panicked at crates\core\src\timeline\types.rs:46:15:
  attempt to add with overflow
  ```
* **Ubicación:** `crates/core/src/timeline/types.rs`, línea 46.
  ```rust
  #[inline]
  pub fn lerp(a: Fixed, b: Fixed, t: Fixed) -> Fixed {
      let diff = b.0 as i64 - a.0 as i64;
      let scaled = (diff * t.0 as i64) >> Self::FRAC_BITS;
      Fixed(a.0 + scaled as i32) // <-- PÁNICO AQUÍ
  }
  ```
* **Causa Raíz:**
  El cálculo `scaled` se realiza en `i64`. Sin embargo, `scaled as i32` se castea a 32 bits **antes** de sumarse a `a.0`. Cuando `scaled >= 2^31` (lo cual ocurre simplemente cuando `diff * t >= 2^47`, alcanzable con `|b - a| > 32767` en formato Q16.16), `scaled as i32` se desborda y se convierte en un número negativo enorme. Al sumarlo con `a.0` (también negativo), la suma excede `i32::MIN` (`-2,147,483,648`), provocando un pánico inmediato por desbordamiento de suma con signo en modo debug/test.
* **Caso de Reproducción Forzado (`test_fixed_lerp_addition_overflow_panic`):**
  Un track con dos keyframes: `Keyframe::new(0, -30517, Easing::Linear)` y `Keyframe::new(100, 30517, Easing::Linear)`.
  Al evaluar a tick 60 ($t = 0.6$):
  * $a = -1,999,962,112$
  * $b = 1,999,962,112$
  * `diff` = $3,999,924,224$
  * `scaled` = $2,399,917,912$
  * `scaled as i32` = $-1,895,049,384$
  * $a.0 + \text{scaled}$ = $-3,895,011,496 < \text{i32::MIN}$ $\rightarrow$ **PÁNICO**.
* **Propuesta de Arreglo Limpio:**
  Realizar la suma en el espacio de 64 bits antes del casteo, o saturar en el rango `i32`:
  ```rust
  #[inline]
  pub fn lerp(a: Fixed, b: Fixed, t: Fixed) -> Fixed {
      let diff = b.0 as i64 - a.0 as i64;
      let scaled = (diff * t.0 as i64) >> Self::FRAC_BITS;
      let result = (a.0 as i64 + scaled).clamp(i32::MIN as i64, i32::MAX as i64);
      Fixed(result as i32)
  }
  ```

---

### Bug 2: Corrupción Silenciosa de Datos en `Fixed::from_int` para Valores Mayores a 32,767 (CRÍTICO)

* **Comportamiento:**
  `Keyframe` define su valor como `i32` (`pub value: i32`), lo que sugiere un rango de $-2^{31}$ a $2^{31}-1$. Sin embargo, en `Track::interpolate`:
  ```rust
  let v0 = Fixed::from_int(k0.value);
  let v1 = Fixed::from_int(k1.value);
  Fixed::lerp(v0, v1, eased_t).to_int()
  ```
  En `Fixed::from_int(n)`:
  ```rust
  pub const fn from_int(n: i32) -> Self {
      Self(n << Self::FRAC_BITS)
  }
  ```
  La operación `n << 16` descarta los 16 bits superiores de `n`:
  * `Fixed::from_int(65536)` resulta en `Fixed(0)`.
  * `Fixed::from_int(100_000)` resulta en un número negativo (`-31,072`).
  * `Fixed::from_int(i32::MAX)` resulta en `-1`.
* **Impacto:**
  Si una animación interpola entre 0 y 65,536 (por ejemplo coordenadas de mapa grande, subpíxeles o valores de volumen/puntuación):
  * A tick 50 (50%), `evaluate(50)` evalúa a **0** en lugar de **32,768**.
  * A tick 100, la condición de fin de pista devuelve **65,536**.
  * Se genera un salto abrupto e interpolación errónea durante toda la duración de la animación.
* **Ubicación:** `crates/core/src/timeline/types.rs:14` y `crates/core/src/timeline.rs:141-143`.
* **Causa Raíz:**
  No es necesario convertir los valores enteros de los keyframes al formato Q16.16. El factor `eased_t` ya está en formato Q16.16 ($[0, 65536]$).
* **Propuesta de Arreglo Limpio:**
  Calcular la interpolación de valores enteros directamente en `Track::interpolate` sin pasar los valores por `Fixed::from_int`:
  ```rust
  let diff = k1.value as i64 - k0.value as i64;
  let offset = (diff * (eased_t.raw() as i64)) >> Fixed::FRAC_BITS;
  let result = k0.value as i64 + offset;
  result.clamp(i32::MIN as i64, i32::MAX as i64) as i32
  ```

---

### Bug 3: Pistas Duplicadas en `Timeline::add_track` Generan Evaluaciones Contradictorias (MAYOR)

* **Comportamiento:**
  `Timeline::add_track` no valida si ya existe una pista para la tupla `(target: EntityId, property: PropertyType)`. Permite agregar múltiples pistas para el mismo objetivo y propiedad.
* **Impacto:**
  * `timeline.evaluate()` emite múltiples tuplas `(EntityId, PropertyType, i32)` para el mismo target y propiedad con valores potencialmente en conflicto.
  * `timeline.get_or_create_track` solo puede acceder a la primera pista encontrada, dejando a las subsiguientes como pistas huérfanas imposibles de modificar.
* **Ubicación:** `crates/core/src/timeline.rs:198-204`.
* **Causa Raíz:**
  `add_track` solo verifica el límite presupuestario `self.tracks.len() >= MAX_TRACKS` pero omite la unicidad de clave natural `(target, property)`.
* **Propuesta de Arreglo Limpio:**
  Agregar validación o reemplazo en `add_track`:
  ```rust
  pub fn add_track(&mut self, track: Track) -> Result<(), TimelineError> {
      if self.tracks.len() >= MAX_TRACKS {
          return Err(TimelineError::TrackLimitExceeded);
      }
      if self.tracks.iter().any(|t| t.target == track.target && t.property == track.property) {
          return Err(TimelineError::DuplicateTrack {
              target: track.target,
              property: track.property,
          });
      }
      self.tracks.push(track);
      Ok(())
  }
  ```

---

### Bug 4: Orden No Determinista de Evaluación en `Timeline::evaluate` (MEDIO)

* **Comportamiento:**
  El vector retornado por `Timeline::evaluate()` respeta el orden de inserción arbitrario de `self.tracks`.
* **Impacto:**
  Si diferentes sistemas o exportadores de scripts agregan pistas en órdenes distintos (por ejemplo, iterando un `HashMap`), las emisiones de propiedades cambian de orden entre ejecuciones, comprometiendo la reproducibilidad estricta.
* **Ubicación:** `crates/core/src/timeline.rs:263-272`.
* **Propuesta de Arreglo Limpio:**
  Mantener las pistas ordenadas internamente por `(target.id(), property as u8)` o asegurar un orden canónico al insertar/evaluar.

---

### Bug 5: Deserialización de `Track` Viola Invariantes sin Validación (MEDIO)

* **Comportamiento:**
  `Track` deriva `serde::Deserialize` directamente sobre `keyframes: Vec<Keyframe>`. Si los datos serializados contienen keyframes no ordenados por tiempo o con tiempos duplicados, se cargan sin error.
* **Impacto:**
  Viola el contrato: *"Invariant: Keyframes in a track are always sorted by time ($t_i < t_{i+1}$)"*. La evaluación posterior produce resultados inconsistentes o keyframes inaccesibles.
* **Ubicación:** `crates/core/src/timeline.rs:31-39`.
* **Propuesta de Arreglo Limpio:**
  Implementar `#[serde(try_from = "RawTrack")]` o deserialización manual que verifique que los keyframes estén ordenados estrictamente ($t_i < t_{i+1}$) y que su longitud no exceda `MAX_KEYFRAMES_PER_TRACK`.

---

## Resumen de la Suite de Pruebas `force_timeline_bugs.rs`

La suite creada en `crates/core/tests/force_timeline_bugs.rs` contiene 13 pruebas unitarias dirigidas:
1. `test_same_tick_add_keyframe_error`: Verifica error en la API pública para ticks duplicados.
2. `test_same_tick_deserialization_bypasses_invariant`: Fuerza keyframes con ticks duplicados vía deserialización y demuestra sombreado.
3. `test_equal_ticks_interpolation_guard`: Verifica comportamiento de guardas ante ticks iguales.
4. `test_inverted_ticks_deserialization_causes_loss_of_interpolation`: Fuerza keyframes desordenados y demuestra omisión de interpolación.
5. `test_fixed_lerp_addition_overflow_panic`: **Fuerza pánico real** por desbordamiento de suma en `Fixed::lerp`.
6. `test_fixed_lerp_positive_overflow_panic`: **Fuerza pánico real** en la dirección inversa en `Fixed::lerp`.
7. `test_fixed_from_int_silent_data_corruption`: **Fuerza corrupción de datos** para keyframes con valores $> 32,767$.
8. `test_extreme_easing_evaluations`: Evalúa funciones de easing ante valores extremos y demuestra distorsión por truncamiento de 32 bits.
9. `test_seek_extreme_and_overflow_advance`: Evalúa seek a `u32::MAX` y avances extremos.
10. `test_timeline_zero_ticks_per_second`: Demuestra falta de validación de `ticks_per_second == 0`.
11. `test_seek_past_duration_unclamped`: Verifica que el seek no se acota a la duración.
12. `test_duplicate_tracks_allowed_conflicting_evaluation`: **Fuerza violación de invariante** permitiendo pistas duplicadas para la misma entidad.
13. `test_non_deterministic_track_evaluation_order`: **Fuerza no-determinismo** en el orden de evaluación de pistas.
