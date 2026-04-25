"""
Headless Python runtime demo.

The Python extension no longer launches the Rust/egui desktop GUI. Use this
example to validate the same script contract from Python, then open the Rust GUI
binary for interactive authoring and preview.

Run with:
    python examples/python/gui_demo.py
"""

from vnengine import run_script_headless


script_json = """
{
  "script_schema_version": "1.0",
  "events": [
    {"type": "scene", "background": "assets/bg_intro.png", "music": "assets/theme.ogg", "characters": []},
    {"type": "dialogue", "speaker": "Sistema", "text": "Bienvenido a la demo headless."},
    {"type": "dialogue", "speaker": "Sistema", "text": "Python valida runtime, audio y flujo sin lanzar GUI."},
    {"type": "choice", "prompt": "Que deseas hacer?", "options": [
      {"text": "Ver mas informacion", "target": "info"},
      {"text": "Terminar demo", "target": "end"}
    ]},
    {"type": "dialogue", "speaker": "Sistema", "text": "El GUI interactivo se ejecuta desde el binario Rust."},
    {"type": "jump", "target": "end"},
    {"type": "dialogue", "speaker": "Sistema", "text": "Demo finalizada."}
  ],
  "labels": {"start": 0, "info": 4, "end": 6}
}
"""


def main() -> None:
    events = run_script_headless(script_json, chooser=lambda _event: 0)
    print("Python headless runtime demo")
    print("=" * 32)
    for index, event in enumerate(events, start=1):
        kind = event.get("type", "unknown")
        speaker = event.get("speaker")
        text = event.get("text") or event.get("prompt") or ""
        prefix = f"{index:02d}. {kind}"
        if speaker:
            prefix += f" [{speaker}]"
        print(f"{prefix}: {text}")


if __name__ == "__main__":
    main()
