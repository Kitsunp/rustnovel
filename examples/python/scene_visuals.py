"""Example that shows scene updates in the visual state."""

from __future__ import annotations

from visual_novel_engine import PyEngine

SCRIPT_JSON = """
{
  "script_schema_version": "1.0",
  "events": [
    {"type": "scene", "background": "bg/room.png", "music": "music/theme.ogg", "characters": [
      {"name": "Ava", "expression": "smile", "position": "center"}
    ]},
    {"type": "patch", "background": "bg/night.png", "add": [], "update": [
      {"name": "Ava", "expression": "serious", "position": null}
    ], "remove": []},
    {"type": "dialogue", "speaker": "Ava", "text": "Bienvenido"}
  ],
  "labels": {"start": 0}
}
"""


def main() -> None:
    engine = PyEngine(SCRIPT_JSON)
    print("current:", engine.current_event())
    print("visual:", engine.visual_state())
    print("step:", engine.step())
    print("visual:", engine.visual_state())
    print("step:", engine.step())
    print("visual:", engine.visual_state())


if __name__ == "__main__":
    main()
