# Visual Novel Engine (Rust)

Complete visual novel engine based on events. It loads a JSON script, advances through events, handles choices, saves/loads game state, and displays the story with a native graphical interface.

## Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start (Rust)](#quick-start-rust)
- [Graphical Interface (GUI)](#graphical-interface-gui)
- [Script Format](#script-format)
- [Save System](#save-system)
- [Development Tools](#development-tools)
- [Python Bindings](#python-bindings)
- [Code Layout](#code-layout)

## Features

- **Logic Engine**: Dialogue, scene, choice, jump, and flag events.
- **Branching & variables**: `jump_if` conditions and integer variables via `set_var`.
- **Visual State**: Maintains accumulated background, music, and characters.
- **Native GUI**: Full viewer built with `eframe` (egui).
- **Persistence**: Binary saves with `script_id` (SHA-256) and integrity checks.
- **Dialogue History**: Scrollable backlog of the last 200 messages.
- **Debug Inspector**: Real-time tool to modify flags and jump to labels.
- **Python Bindings**: Use the engine from Python via `pyo3`.
- **AssetStore**: Asset loading with path sanitization, limits, and optional manifest.

## Installation

### Core only (no GUI)

```toml
[dependencies]
visual_novel_engine = { path = "crates/core" }
```

### Automatic Install (Windows)

Run the included script to build Rust and install Python bindings:

```powershell
.\install.ps1
```

### Manual Build

1. **Rust (Core + GUI)**: `cargo build --release`
2. **Python Bindings**:
   ```bash
   pip install maturin
   maturin build --manifest-path crates/py/Cargo.toml --release
   pip install target/wheels/*.whl --force-reinstall
   ```

### With graphical interface

```toml
[dependencies]
visual_novel_gui = { path = "crates/gui" }
```

## Quick Start (Rust)

### Logic only (no window)

```rust
use visual_novel_engine::{Engine, Script, SecurityPolicy, ResourceLimiter};

let script_json = r#"
{
  "script_schema_version": "1.0",
  "events": [
    {"type": "dialogue", "speaker": "Ava", "text": "Hello"},
    {"type": "choice", "prompt": "Go?", "options": [
      {"text": "Yes", "target": "end"},
      {"text": "No", "target": "start"}
    ]},
    {"type": "dialogue", "speaker": "Ava", "text": "The end"}
  ],
  "labels": {"start": 0, "end": 2}
}
"#;

let script = Script::from_json(script_json)?;
let mut engine = Engine::new(script, SecurityPolicy::default(), ResourceLimiter::default())?;

println!("Current event: {:?}", engine.current_event()?);
engine.step()?;
engine.choose(0)?; // Pick the first option
```

### With graphical interface

```rust
use visual_novel_gui::{run_app, VnConfig};

let script_json = include_str!("my_story.json");

let config = VnConfig {
    title: "My Visual Novel".to_string(),
    width: Some(1280.0),
    height: Some(720.0),
    ..Default::default()
};

run_app(script_json.to_string(), Some(config))?;
```

## Graphical Interface (GUI)

The GUI provides a complete visual novel experience:

- **Scene Rendering**: Displays backgrounds, characters, and music info.
- **Dialogue Box**: Presents text and choices interactively.
- **Settings Menu** (`ESC`): Adjust UI scale, fullscreen, and VSync.
- **History** (UI button): Review past dialogue lines.
- **Save/Load**: Native file dialogs from the settings menu.

## Script Format

A script is JSON with:

- `script_schema_version`: JSON schema version for the script.
- `events`: list of events.
- `labels`: map of labels to indices (`start` is required).

```json
{"type": "dialogue", "speaker": "Ava", "text": "Hello"}
{"type": "choice", "prompt": "Go?", "options": [{"text": "Yes", "target": "end"}]}
{"type": "scene", "background": "bg/room.png", "music": "music/theme.ogg", "characters": [{"name": "Ava", "expression": "smile", "position": "center"}]}
{"type": "jump", "target": "intro"}
{"type": "set_flag", "key": "visited", "value": true}
{"type": "set_var", "key": "counter", "value": 3}
{"type": "jump_if", "cond": {"kind": "var_cmp", "key": "counter", "op": "gt", "value": 1}, "target": "high"}
{"type": "patch", "background": "bg/night.png", "add": [{"name": "Ava", "expression": "smile", "position": "left"}], "update": [], "remove": []}
```

## Save System

The engine includes secure persistence:

- **Script identity**: Each save stores the `script_id` (SHA-256 of the compiled binary).
- **Validation on Load**: If the script changed, the save is rejected to prevent corruption.
- **Binary format**: Saves use a canonical binary format with versioning and checksum.

## Development Tools

### Inspector (`F12`)

Debug window for developers:

- View and modify **flags** in real time.
- Jump to any **label** in the script.
- Monitor **FPS** and history memory usage.

### CLI (`vnengine`)

Primary commands for QA and tooling:

```bash
vnengine validate script.json
vnengine compile script.json -o script.vnsc
vnengine trace script.json --steps 50 -o trace.yaml
vnengine verify-save save.vns --script script.vnsc
vnengine manifest assets/ -o manifest.json
vnengine import-renpy path/to/renpy-project -o imported_project --entry-label start
```

## Python Bindings

### Installation

```bash
pip install visual_novel_engine --find-links=target/wheels
```

> Note: Ensure you ran `install.ps1` or built manually with `maturin` first.

### Basic usage (logic only)

```python
from visual_novel_engine import PyEngine

engine = PyEngine(script_json)
print(engine.current_event())
engine.step()
engine.choose(0)
```

### With graphical interface

```python
import visual_novel_engine as vn

config = vn.VnConfig(width=1280.0, height=720.0)
vn.run_visual_novel(script_json, config)
```

## Code Layout

- `crates/core/`: Engine core (logic, compilation, state).
- `crates/gui/`: Graphical interface with eframe.
- `crates/py/`: Python bindings.
- `examples/`: Usage examples in Rust and Python.

## Security modes

The engine supports two modes:

- **Trusted** (default): scripts/assets are trusted.
- **Untrusted**: validates paths, sizes, and asset hashes (optional manifest).
