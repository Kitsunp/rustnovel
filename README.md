# Visual Novel Engine (Rust)

Motor completo para novelas visuales basado en eventos. Permite interpretar un guion en JSON, avanzar por los eventos, tomar decisiones, guardar/cargar partidas y visualizar la historia con una interfaz gráfica nativa.

## Contenido

- [Características](#características)
- [Instalación](#instalación)
- [Uso rápido (Rust)](#uso-rápido-rust)
- [Interfaz Gráfica (GUI)](#interfaz-gráfica-gui)
- [Formato del guion](#formato-del-guion)
- [Sistema de Guardado](#sistema-de-guardado)
- [Herramientas de Desarrollo](#herramientas-de-desarrollo)
- [Bindings de Python](#bindings-de-python)
- [Estructura del código](#estructura-del-código)

## Características

- **Motor Lógico**: Eventos de diálogo, escena, elecciones, saltos y banderas.
- **Branching y variables**: Condiciones (`jump_if`) y variables enteras (`set_var`) con comparadores.
- **Estado Visual**: Mantiene fondo, música y personajes acumulados.
- **Interfaz Gráfica Nativa**: Visualizador completo con `eframe` (egui).
- **Persistencia**: Guardados binarios con `script_id` (SHA-256) y verificación de integridad.
- **Historial de Diálogo**: Backlog navegable de los últimos 200 mensajes.
- **Inspector de Depuración**: Herramienta en tiempo real para modificar banderas y saltar etiquetas.
- **Bindings Python**: Usa el motor desde Python con `pyo3`.
- **AssetStore**: Carga de assets con saneamiento de rutas, límites y manifest opcional.

## Instalación

### Solo el núcleo (sin GUI)

```toml
[dependencies]
visual_novel_engine = { path = "crates/core" }
```

### Instalación Automática (Windows)

Ejecuta el script incluido para compilar Rust e instalar los bindings de Python:

```powershell
.\install.ps1
```

### Compilación Manual

1. **Rust (Core + GUI)**: `cargo build --release`
2. **Python Bindings**:
   ```bash
   pip install maturin
   maturin build --manifest-path crates/py/Cargo.toml --release
   pip install target/wheels/*.whl --force-reinstall
   ```

### Con interfaz gráfica

```toml
[dependencies]
visual_novel_gui = { path = "crates/gui" }
```

## Prerrequisitos y Configuración Local

Para compilar y ejecutar el proyecto correctamente en tu entorno local, asegúrate de instalar:

### Windows

1.  **Rust**: Usando `rustup`.
2.  **C++ Build Tools**: A través de Visual Studio Installer (necesario para el enlazado).
3.  **Drivers de GPU**: Asegúrate de tener drivers compatibles con **Vulkan**, **DirectX 12** o **DirectX 11**.
    - Si no tienes GPU dedicada, el motor usará el fallback de Software automáticamente.
4.  **Python 3.10+**: Necesario si planeas compilar o probar los bindings de Python (`crates/py`).

### Testing

El proyecto incluye una suite de pruebas completa:

```bash
# Ejecutar verificación de compilación (Rápido)
cargo check --workspace --tests

# Ejecutar todos los tests (Unitarios + Integración + Snapshots)
# Nota: Puede fallar en entornos sin librerías gráficas o de python enlazadas.
cargo test --workspace
```

## Uso rápido (Rust)

### Solo lógica (sin ventana)

```rust
use visual_novel_engine::{Engine, Script, SecurityPolicy, ResourceLimiter};

let script_json = r#"
{
  "script_schema_version": "1.0",
  "events": [
    {"type": "dialogue", "speaker": "Ava", "text": "Hola"},
    {"type": "choice", "prompt": "¿Ir?", "options": [
      {"text": "Sí", "target": "end"},
      {"text": "No", "target": "start"}
    ]},
    {"type": "dialogue", "speaker": "Ava", "text": "Fin"}
  ],
  "labels": {"start": 0, "end": 2}
}
"#;

let script = Script::from_json(script_json)?;
let mut engine = Engine::new(script, SecurityPolicy::default(), ResourceLimiter::default())?;

println!("Evento actual: {:?}", engine.current_event()?);
engine.step()?;
engine.choose(0)?; // Elige la primera opción
```

### Con interfaz gráfica

```rust
use visual_novel_gui::{run_app, VnConfig};

let script_json = include_str!("mi_historia.json");

let config = VnConfig {
    title: "Mi Novela Visual".to_string(),
    width: Some(1280.0),
    height: Some(720.0),
    ..Default::default()
};

run_app(script_json.to_string(), Some(config))?;
```

## Interfaz Gráfica (GUI)

La GUI proporciona una experiencia completa de novela visual:

- **Renderizado de Escenas**: Muestra fondos, personajes y música.
- **Caja de Diálogo**: Presenta el texto y opciones de forma interactiva.
- **Menú de Configuración** (`ESC`): Ajusta escala de UI, pantalla completa y VSync.
- **Historial** (botón en UI): Revisa los últimos diálogos leídos.
- **Guardar/Cargar**: Desde el menú de configuración, usa diálogos de archivo nativos.

## Formato del guion

Un guion es un JSON con:

- `script_schema_version`: versión del esquema JSON del guion.
- `events`: lista de eventos.
- `labels`: mapa de etiquetas a índices (`start` es obligatorio).

```json
{"type": "dialogue", "speaker": "Ava", "text": "Hola"}
{"type": "choice", "prompt": "¿Ir?", "options": [{"text": "Sí", "target": "end"}]}
{"type": "scene", "background": "bg/room.png", "music": "music/theme.ogg", "characters": [{"name": "Ava", "expression": "smile", "position": "center"}]}
{"type": "jump", "target": "intro"}
{"type": "set_flag", "key": "visited", "value": true}
{"type": "set_var", "key": "counter", "value": 3}
{"type": "jump_if", "cond": {"kind": "var_cmp", "key": "counter", "op": "gt", "value": 1}, "target": "high"}
{"type": "patch", "background": "bg/night.png", "add": [{"name": "Ava", "expression": "smile", "position": "left"}], "update": [], "remove": []}
```

## Sistema de Guardado

El motor incluye persistencia segura:

- **Identidad de Script**: Cada save guarda el `script_id` (SHA-256 del binario compilado).
- **Validación al Cargar**: Si el guion cambió, el save se rechaza para evitar corrupción.
- **Formato binario**: Los saves usan un formato binario canónico con versión y checksum.

## Herramientas de Desarrollo

### Inspector (`F12`)

Ventana de depuración para desarrolladores:

- Ver y modificar **banderas** en tiempo real.
- Saltar a cualquier **etiqueta** del guion.
- Monitorear **FPS** y uso de memoria del historial.

### CLI (`vnengine`)

Comandos principales para QA y herramientas internas:

```bash
vnengine validate script.json
vnengine compile script.json -o script.vnsc
vnengine trace script.json --steps 50 -o trace.yaml
vnengine verify-save save.vns --script script.vnsc
vnengine manifest assets/ -o manifest.json
vnengine import-renpy path/to/renpy-project -o imported_project --entry-label start
```

## Bindings de Python

### Instalación

```bash
pip install visual_novel_engine --find-links=target/wheels
```

> Nota: Asegúrate de haber ejecutado `install.ps1` o construido con `maturin` primero.

### Uso básico (solo lógica)

```python
from visual_novel_engine import PyEngine

engine = PyEngine(script_json)
print(engine.current_event())
engine.step()
engine.choose(0)
```

### Con interfaz gráfica

```python
import visual_novel_engine as vn

config = vn.VnConfig(width=1280.0, height=720.0)
vn.run_visual_novel(script_json, config)
```

## Estructura del código

- `crates/core/`: Núcleo del motor (lógica, compilación, estado).
- `crates/gui/`: Interfaz gráfica con eframe.
- `crates/py/`: Bindings de Python.
- `examples/`: Ejemplos de uso en Rust y Python.

## Seguridad y modos de ejecución

El motor soporta dos modos:

- **Trusted** (default): scripts/assets confiables.
- **Untrusted**: valida rutas, tamaños y hashes de assets (manifest opcional).

---

## Sistema de Animaciones (Timeline)

El motor incluye un sistema de línea de tiempo para animaciones deterministas.

### Uso en Rust

```rust
use visual_novel_engine::{Timeline, Track, Keyframe, Easing, EntityId, PropertyType};

// Crear timeline a 60 ticks/segundo
let mut timeline = Timeline::new(60);

// Crear track para animar posición X
let mut track = Track::new(EntityId::new(1), PropertyType::PositionX);
track.add_keyframe(Keyframe::new(0, 0, Easing::Linear))?;
track.add_keyframe(Keyframe::new(60, 100, Easing::EaseOut))?;
timeline.add_track(track)?;

// Evaluar en cualquier tiempo
timeline.seek(30);
let values = timeline.evaluate(); // [(EntityId(1), PositionX, 50)]
```

### Uso en Python

```python
import visual_novel_engine as vn

timeline = vn.Timeline(ticks_per_second=60)
track = vn.Track(entity_id=1, property="position_x")
track.add_keyframe(vn.Keyframe(0, 0, "linear"))
track.add_keyframe(vn.Keyframe(60, 100, "ease_out"))
timeline.add_track(track)

timeline.seek(30)
values = timeline.evaluate()
```

**Funciones de Easing**: `linear`, `ease_in`, `ease_out`, `ease_in_out`, `step`

**Ejemplo completo**: Ver `examples/python/timeline_demo.py`

---

## Grafo de Historia (Story Graph)

Genera un grafo dirigido desde el script compilado para visualizar el flujo narrativo.

### Uso en Rust

```rust
use visual_novel_engine::{ScriptRaw, StoryGraph};

let script = ScriptRaw::from_json(script_json)?;
let compiled = script.compile()?;
let graph = StoryGraph::from_script(&compiled);

// Estadísticas
let stats = graph.stats();
println!("Nodos: {}, Inalcanzables: {}", stats.total_nodes, stats.unreachable_nodes);

// Detectar nodos muertos
let dead = graph.unreachable_nodes();

// Exportar a DOT (Graphviz)
let dot = graph.to_dot();
std::fs::write("graph.dot", dot)?;
// Generar PNG: dot -Tpng graph.dot -o graph.png
```

### Uso en Python

```python
import visual_novel_engine as vn

graph = vn.StoryGraph.from_json(script_json)

# Estadísticas
stats = graph.stats()
print(f"Nodos: {stats.total_nodes}, Inalcanzables: {stats.unreachable_nodes}")

# Detectar código muerto
for node_id in graph.unreachable_nodes():
    print(f"⚠️ Nodo {node_id} inalcanzable")

# Exportar para visualización
with open("graph.dot", "w") as f:
    f.write(graph.to_dot())
```

**Ejemplo completo**: Ver `examples/python/story_graph_demo.py`

---

## Sistema de Entidades (Entity System)

Sistema ligero para gestionar entidades visuales (imágenes, personajes, texto).

### Componentes Principales

- **EntityId**: Identificador único (u32)
- **Transform**: Posición, z-order, escala, opacidad (punto fijo)
- **EntityKind**: Tipo de entidad (`Image`, `Text`, `Character`, `Video`, `Audio`)
- **SceneState**: Colección de entidades con iteración determinista

### Uso en Rust

```rust
use visual_novel_engine::{SceneState, EntityKind, ImageData, Transform};

let mut scene = SceneState::new();

// Spawn entidades
let bg_id = scene.spawn(EntityKind::Image(ImageData {
    path: "bg/room.png".into(),
    tint: None,
}))?;

// Modificar transform
if let Some(entity) = scene.get_mut(bg_id) {
    entity.transform.z_order = -100;
}

// Iterar en orden determinista (z-order, luego id)
for entity in scene.iter_sorted() {
    println!("{}: {:?}", entity.id, entity.kind);
}
```

---

## Ejemplos

### Rust

```bash
# Timeline demo
cargo run -p visual_novel_engine --example timeline_demo

# Story graph demo
cargo run -p visual_novel_engine --example story_graph_demo
```

### Python

```bash
# Requiere: pip install visual_novel_engine
python examples/python/timeline_demo.py
python examples/python/story_graph_demo.py
```

---

## 🎨 Editor Visual / Visual Editor

### Español

El motor incluye un editor visual nativo para crear y editar historias de forma interactiva.

#### Ejecutar el Editor

```bash
# Compilar y ejecutar el editor
cargo run -p visual_novel_gui --bin vn_editor

# O compilar primero y luego ejecutar
cargo build -p visual_novel_gui --bin vn_editor --release
./target/release/vn_editor
```

#### Paneles del Editor

| Panel         | Descripción                                                  |
| ------------- | ------------------------------------------------------------ |
| **Timeline**  | Controles de reproducción, scrubbing, lista de tracks        |
| **Graph**     | Vista de nodos de la historia con detección de inalcanzables |
| **Inspector** | Propiedades del nodo/entidad seleccionada                    |
| **Viewport**  | Vista previa de la escena con entidades                      |

#### Uso

1. **File → Open Script**: Carga un archivo JSON de script
2. **View**: Muestra/oculta los paneles (Timeline, Graph, Inspector)
3. **Mode**: Cambia entre modo Player (juego) y Editor

#### Script de Ejemplo

Para probar el editor con un ejemplo completo, usa el script incluido:

```bash
# Abre el editor y carga el archivo de ejemplo
# File → Open Script → examples/scripts/demo_story.json
```

El script `demo_story.json` incluye:

- Escenas con fondos y personajes
- Diálogos con múltiples personajes
- Sistema de elecciones (3 rutas)
- Variables y banderas

---

### English

The engine includes a native visual editor for interactive story creation and editing.

#### Running the Editor

```bash
# Compile and run the editor
cargo run -p visual_novel_gui --bin vn_editor

# Or compile first, then run
cargo build -p visual_novel_gui --bin vn_editor --release
./target/release/vn_editor
```

#### Editor Panels

| Panel         | Description                                |
| ------------- | ------------------------------------------ |
| **Timeline**  | Playback controls, scrubbing, track list   |
| **Graph**     | Story node view with unreachable detection |
| **Inspector** | Selected node/entity properties            |
| **Viewport**  | Scene preview with entities                |

#### Usage

1. **File → Open Script**: Load a JSON script file
2. **View**: Show/hide panels (Timeline, Graph, Inspector)
3. **Mode**: Switch between Player (game) and Editor mode

#### Example Script

To test the editor with a complete example, use the included script:

```bash
# Open the editor and load the example file
# File → Open Script → examples/scripts/demo_story.json
```

The `demo_story.json` script includes:

- Scenes with backgrounds and characters
- Dialogues with multiple characters
- Choice system (3 branching paths)
- Variables and flags
