# Reporte de Auditoría: Forzado de Bugs en Máquina de Estados y Persistencia del Motor

**Área auditada:** `engine_state_and_save`  
**Archivos fuente analizados:**  
- `crates/core/src/engine/runtime.rs`
- `crates/core/src/state.rs`
- `crates/core/src/storage.rs`
- `crates/core/src/storage/helpers.rs`
- `crates/core/src/security.rs`
- `crates/core/src/event_behavior/runtime.rs`
- `crates/core/src/event_behavior/context.rs`

**Suite de pruebas de verificación adversaria:** `crates/core/tests/force_engine_bugs.rs`  
**Comando de ejecución:**
```powershell
$env:RUSTUP_HOME='D:\Rust\rustup'; $env:CARGO_HOME='D:\Rust\cargo'; $env:PYO3_USE_ABI3_FORWARD_COMPATIBILITY='1'; $env:PATH="D:\Rust\cargo\bin;D:\Rust\mingw64\bin;C:\Users\pelju\AppData\Local\Python\pythoncore-3.14-64;$env:PATH";
cargo test -p visual_novel_engine --test force_engine_bugs -- --test-threads=1
```
**Resultado de la ejecución:** 19 pruebas ejecutadas, 19 pasadas (100% de reproducción y verificación).

---

## 1. Respuestas a las Preguntas de Auditoría

### 1. Desbordamiento de enteros en variables
* **¿Qué sucede al usar `set_var` o comparaciones con `i32::MAX`, `i32::MIN` o `i32::MAX + 1`?**
  * **Valores extremos en runtime:** `EngineState::set_var` almacena valores `i32` sin modificar (`i32::MAX`, `i32::MIN`, etc.). La evaluación de `CondCompiled::VarCmp` con operadores `CmpOp` (`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`) en [crates/core/src/event_behavior/context.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/event_behavior/context.rs#L140-L150) utiliza comparaciones nativas de Rust entre enteros con signo, por lo que **no hay pánico por overflow** ni desbordamiento aritmético en tiempo de ejecución para comparaciones.
  * **Deserialización de `i32::MAX + 1` (ej. 2147483648):** En `ScriptRaw::from_json`, el intento de deserializar un entero mayor a `i32::MAX` en `EventRaw::SetVar { value: i32 }` es interceptado limpiamente por `serde_json`, devolviendo un `Err(VnError::Serialization)`.
  * **Ren'Py Import:** En [crates/core/src/renpy_import/syntax.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/renpy_import/syntax.rs#L177), `rhs.parse::<i32>()` retorna `Err` ante valores fuera de rango, lo que degrada la instrucción de forma segura a una llamada externa (`ext_call`) sin pánico.
  * **VULNERABILIDAD CRÍTICA EN `var_id`:** Mientras que `SecurityPolicy::validate_compiled` valida estrictamente los flags (`flag_id < script.flag_count`), **NO valida los identificadores de variables (`var_id`)**. `ScriptCompiled` ni siquiera cuenta con un campo `var_count`. Un script con un `var_id` arbitrario (ej. `20_000` o `u32::MAX`) elude la validación por completo. Al ejecutarse en `step()`, [crates/core/src/state.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/state.rs#L67) invoca `self.vars.resize(idx + 1, 0)`, provocando asignación de memoria masiva no controlada (OOM) o pánico por `attempt to add with overflow` en plataformas de 32-bit (`idx + 1`).

### 2. Bucle infinito de saltos
* **Si un script tiene labels saltándose en loop sin eventos de diálogo, ¿se congela `step()` o tiene un límite de pasos?**
  * `Engine::step()` ejecuta **exactamente un evento compilado por invocación**. Al encontrar un `EventCompiled::Jump`, actualiza `state.position = target_ip` y retorna inmediatamente `Ok((audio, change))`.
  * **Ausencia de límite de pasos:** `Engine` **NO tiene ningún contador de pasos, ni presupuesto de ejecución, ni detección de ciclos**. En la prueba adversaria (`test_2_infinite_jump_loop_has_no_step_limit`), se ejecutaron 1,000 llamadas a `step()` consecutivas entre dos etiquetas cíclicas (`loop_a -> jump loop_b -> jump loop_a`) sin que el motor emitiera ningún error ni detención.
  * **Consecuencia para hosts y runners:** Si un adaptador o bucle de juego ejecuta `while !engine.is_current_dialogue_read() { engine.step()?; }`, **el hilo de ejecución se congelará en un bucle infinito al 100% de CPU**.

### 3. Save data corruption
* **¿Qué ocurre al cargar partidas con bytes truncados, campos mutados, script_id manipulado o números mágicos corruptos?**
  * **Bytes truncados (< 14 bytes binario simple o < 42 bytes autenticado):** Devuelve limpiamente `Err(SaveError::TooSmall)`. No ocurre ningún pánico.
  * **Magic bytes corruptos:** Devuelve limpiamente `Err(SaveError::InvalidMagic)`.
  * **Versión incompatible:** Devuelve limpiamente `Err(SaveError::IncompatibleVersion { found, expected })`.
  * **Cuerpo mutado o longitud alterada:** Devuelve limpiamente `Err(SaveError::LengthMismatch)`, `Err(SaveError::ChecksumMismatch)` o `Err(SaveError::AuthenticationFailed)`.
  * **Validación de `script_id`:** `SaveData::validate_script_id(&expected)` devuelve limpiamente `Err(SaveError::ScriptMismatch)`.
  * **FALLA EN `Engine::set_state`:** `Engine::set_state` recibe directamente `EngineState` y **no verifica la identidad del script (`ScriptId`)**. Una partida de un juego/script completamente diferente puede inyectarse en el motor sin ningún error mientras `state.position <= script.events.len()`.
  * **BUG OFF-BY-ONE EN POSICIÓN:** `Engine::set_state` valida `if state.position as usize > self.script.events.len()`. Dado que usa `>` en lugar de `>=`, **acepta `position == script.events.len()`**, colocando al motor en un estado donde `current_event()` y `step()` fallan inmediatamente con `EndOfScript`.
  * **FRAGILIDAD EN `SaveSlotStore::list_slots`:** Si un único archivo JSON en `meta/` está corrupto, `list_slots()` aborta la llamada completa devolviendo `Err(SaveStoreError::Save(...))`, impidiendo que el usuario vea o cargue cualquiera de sus otras partidas válidas.

### 4. Choices inválidas
* **¿Qué pasa si se llama a `choose(999)` o `choose` cuando el evento actual NO es una elección?**
  * **Evento no es Choice:** Retorna limpiamente el error tipado `Err(VnError::InvalidChoice)`. No causa pánico.
  * **Índice fuera de límites (`choose(999)`):** Retorna limpiamente `Err(VnError::InvalidChoice)` y preserva la posición del motor en el evento de elección.
  * **`choose` al final del script:** Retorna `Err(VnError::EndOfScript)`.
  * **BUG CRÍTICO DE ESTANCAMIENTO EN `step()`:** Si el evento actual es un `Choice` y el llamador invoca `engine.step()`, el motor retorna `Ok((vec![], change))`, pero **NO incrementa la posición** ([crates/core/src/event_behavior/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/event_behavior/runtime.rs#L156): `EventCompiled::Choice(_) => Ok(())`). Si el runner invoca `step()` esperando avanzar, se queda congelado en el mismo choice indefinidamente sin recibir ningún error.
  * **BUG DE MUTACIÓN PREMATURA EN `choose()`:** En [crates/core/src/engine/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L188-L195), `record_choice_decision` se invoca **antes** de `self.jump_to_ip`. Si el salto falla, la decisión ya quedó registrada permanentemente en `self.choice_history`, corrompiendo el historial con una elección fantasma.

### 5. Historial de diálogo
* **¿Qué pasa si se fuerzan más de 200 eventos rápidamente? ¿Consume memoria o descarta los más viejos?**
  * **Descarte de diálogo:** En `EngineState::record_dialogue`, al alcanzar el límite constante `HISTORY_LIMIT = 200`, se ejecuta `self.history.pop_front()` antes de insertar el nuevo diálogo. En la prueba con 250 eventos consecutivos, el buffer se mantuvo exactamente en 200 elementos y los primeros 50 eventos fueron descartados en orden FIFO.
  * **CONSUMO DE MEMORIA EN `read_dialogue_ips` Y `route_visited_ips`:** A diferencia de `state.history`, las colecciones `read_dialogue_ips` y `route_visited_ips` de `Engine` son `BTreeSet<u32>` **sin ningún límite de capacidad**. Al recorrer 250 eventos, almacenan los 250 IPs. En sesiones largas con miles de eventos, estas colecciones crecen de manera estrictamente monótona.
  * **BUG DE INVARIANTE EN CARGA:** Si se deserializa un `EngineState` cuya lista `history` contiene más de 200 entradas (ej. 210 entradas provenientes de una partida manipulada), `record_dialogue` solo realiza un `pop_front()` por inserción, manteniendo el tamaño en 210 permanentemente sin podar el exceso hacia 200.

---

## 2. Detalle de Bugs Forzados y Hallazgos

### Bug 1: Ausencia de Validación de Límites en `var_id` (Severidad: ALTA / DoS / Crash)

* **Ubicación:**  
  - [crates/core/src/security.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/security.rs#L272-L277)
  - [crates/core/src/state.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/state.rs#L64-L70)
* **Comportamiento inesperado:**  
  `SecurityPolicy::validate_compiled` valida `flag_id < script.flag_count`, pero ignora completamente `EventCompiled::SetVar`. Además, `ScriptCompiled` no almacena un `var_count`.
* **Causa raíz:**  
  `set_var` en `EngineState` ejecuta:
  ```rust
  let idx = id as usize;
  if idx >= self.vars.len() {
      self.vars.resize(idx + 1, 0);
  }
  self.vars[idx] = value;
  ```
  Un script con `EventCompiled::SetVar { var_id: 100_000_000, value: 1 }` elude toda validación previa y, al llamar a `step()`, fuerza una asignación de cientos de megabytes o gigabytes en memoria. En arquitecturas de 32 bits, `u32::MAX as usize + 1` genera pánico aritmético en modo debug.
* **Propuesta de corrección:**  
  1. Agregar `var_count: u32` a `ScriptCompiled`.
  2. En `SecurityPolicy::validate_compiled`, verificar:
     ```rust
     EventCompiled::SetVar { var_id, .. } if *var_id >= script.var_count => {
         return Err(VnError::InvalidScript(format!(
             "var id {} outside compiled range",
             var_id
         )));
     }
     ```
  3. En `EngineState::set_var`, limitar `idx` a un máximo estricto (ej. 16,384) antes de llamar a `resize`.

---

### Bug 2: Estancamiento Silencioso de `Engine::step()` en Eventos `Choice` (Severidad: MEDIA / Lógica)

* **Ubicación:**  
  - [crates/core/src/event_behavior/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/event_behavior/runtime.rs#L156)
  - [crates/core/src/engine/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L161-L171)
* **Comportamiento inesperado:**  
  La documentación de `Engine::step()` indica: *"Advances the engine by applying the current event"*. Sin embargo, al llegar a un evento `Choice`, `engine.step()` devuelve `Ok(...)` pero **no avanza la posición**.
* **Causa raíz:**  
  En `execute_event_compiled`:
  ```rust
  EventCompiled::Choice(_) => Ok(()),
  ```
  No avanza la posición ni retorna un error que indique que la ejecución está detenida a la espera de una decisión interactiva. Un llamador que invoque `step()` en un bucle normal se quedará estancado infinitamente procesando la misma elección.
* **Propuesta de corrección:**  
  `Engine::step()` debe retornar un error tipado cuando el evento actual es una elección, exigiendo el uso de `choose`:
  ```rust
  EventCompiled::Choice(_) => Err(VnError::InvalidChoice),
  ```
  O definir una variante explícita `VnError::ChoicePending`.

---

### Bug 3: Falla de Aislamiento de Script en `Engine::set_state` (Severidad: MEDIA / Corrupción de Datos)

* **Ubicación:**  
  - [crates/core/src/engine/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L450-L480)
* **Comportamiento inesperado:**  
  `Engine::set_state` permite inyectar el estado de cualquier otro script sin validar la correspondencia criptográfica del script de origen.
* **Causa raíz:**  
  `SaveData` almacena `script_id: ScriptId`, pero `set_state` acepta únicamente `EngineState`. Si la capa superior no llama explícitamente a `save.validate_script_id(&expected)`, variables, flags y punteros de instrucción de un juego A se aplican a un juego B, causando comportamiento errático o saltos a eventos incoherentes.
* **Propuesta de corrección:**  
  Agregar un método seguro en `Engine`:
  ```rust
  pub fn load_save_data(&mut self, save: &SaveData) -> VnResult<()> {
      let current_script_id = compute_script_id(&self.script.to_binary()?);
      save.validate_script_id(&current_script_id)
          .map_err(|e| VnError::InvalidScript(e.to_string()))?;
      self.set_state(save.state.clone())
  }
  ```

---

### Bug 4: Error Off-By-One en Límite de Posición en `set_state` (Severidad: BAJA / Lógica)

* **Ubicación:**  
  - [crates/core/src/engine/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L451)
* **Comportamiento inesperado:**  
  `engine.set_state` acepta estados cuya posición es exactamente igual a `script.events.len()`, a pesar de que dicho índice está fuera del rango de eventos válidos (`0..events.len()`).
* **Causa raíz:**  
  La condición actual es:
  ```rust
  if state.position as usize > self.script.events.len() {
  ```
  El operador `>` debería ser `>=`.
* **Propuesta de corrección:**  
  ```rust
  if state.position as usize >= self.script.events.len() {
      return Err(VnError::InvalidScript(format!(
          "state position '{}' outside script",
          state.position
      )));
  }
  ```

---

### Bug 5: Mutación Prematura de `choice_history` en `Engine::choose` (Severidad: MEDIA / Integridad)

* **Ubicación:**  
  - [crates/core/src/engine/runtime.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/engine/runtime.rs#L188-L195)
* **Comportamiento inesperado:**  
  Si la operación de salto a la opción elegida (`self.jump_to_ip`) falla por cualquier motivo, la decisión ya fue insertada en `self.choice_history`.
* **Causa raíz:**  
  ```rust
  self.record_choice_decision(
      self.state.position,
      choice.prompt.as_ref(),
      option_index,
      option.text.as_ref(),
      option.target_ip,
  );
  self.jump_to_ip(option.target_ip)?; // <-- Si esto falla, el historial queda contaminado
  ```
* **Propuesta de corrección:**  
  Efectuar el salto primero y solo registrar la decisión una vez confirmado el éxito:
  ```rust
  let current_ip = self.state.position;
  self.jump_to_ip(option.target_ip)?;
  self.record_choice_decision(
      current_ip,
      choice.prompt.as_ref(),
      option_index,
      option.text.as_ref(),
      option.target_ip,
  );
  ```

---

### Bug 6: Invariante Rota en `record_dialogue` ante Historiales Sobredimensionados (Severidad: BAJA)

* **Ubicación:**  
  - [crates/core/src/state.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/state.rs#L78-L84)
* **Comportamiento inesperado:**  
  Si un estado se deserializa con más de 200 diálogos (ej. 210), las llamadas subsecuentes a `record_dialogue` mantienen el tamaño en 210 y nunca restauran el límite contractuado de 200.
* **Causa raíz:**  
  `if self.history.len() >= HISTORY_LIMIT { self.history.pop_front(); }` solo elimina un elemento por llamada.
* **Propuesta de corrección:**  
  Reemplazar el `if` por un bucle `while`:
  ```rust
  while self.history.len() >= HISTORY_LIMIT {
      self.history.pop_front();
  }
  self.history.push_back(dialogue.clone());
  ```

---

### Bug 7: Falla en Cascada de `SaveSlotStore::list_slots` por Metadatos Corruptos (Severidad: MEDIA / UX)

* **Ubicación:**  
  - [crates/core/src/storage.rs](file:///D:/Rust/audit/rustnovel/crates/core/src/storage.rs#L360-L362)
* **Comportamiento inesperado:**  
  La presencia de un solo archivo JSON inválido o truncado en `meta/` hace que `list_slots()` falle completamente para todo el directorio de guardado.
* **Causa raíz:**  
  `list_slots` propaga el error de deserialización de inmediato mediante el operador `?`:
  ```rust
  let metadata: SaveSlotMetadata = serde_json::from_slice(&bytes)
      .map_err(|err| SaveError::Serialization(err.to_string()))?;
  ```
* **Propuesta de corrección:**  
  Ignorar o registrar como advertencia los metadatos defectuosos para permitir listar las demás ranuras válidas:
  ```rust
  let Ok(metadata) = serde_json::from_slice::<SaveSlotMetadata>(&bytes) else {
      continue;
  };
  ```
