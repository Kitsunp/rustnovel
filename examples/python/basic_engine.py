"""
Ejemplo básico de uso del motor sin interfaz gráfica.

Demuestra cómo:
- Crear un engine desde un script JSON
- Avanzar por eventos
- Manejar elecciones
- Consultar el estado visual

Ejecutar con: python examples/python/basic_engine.py

Requisitos:
    maturin develop --features python
"""

from __future__ import annotations

from visual_novel_engine import PyEngine

SCRIPT_JSON = """
{
    "script_schema_version": "1.0",
    "events": [
    {"type": "scene", "background": "bg/sala.png", "music": null, "characters": [
      {"name": "Ava", "expression": "neutral", "position": "center"}
    ]},
    {"type": "dialogue", "speaker": "Ava", "text": "Hola, bienvenido."},
    {"type": "set_flag", "key": "saludo_visto", "value": true},
    {"type": "set_var", "key": "contador", "value": 0},
    {"type": "jump_if", "cond": {"kind": "var_cmp", "key": "contador", "op": "gt", "value": 2}, "target": "amable"},
    {"type": "choice", "prompt": "¿Qué respondes?", "options": [
      {"text": "Hola, ¿cómo estás?", "target": "amable"},
      {"text": "No tengo tiempo.", "target": "end"}
    ]},
    {"type": "dialogue", "speaker": "Ava", "text": "¡Qué amable! Estoy bien, gracias."},
    {"type": "jump", "target": "end"},
    {"type": "dialogue", "speaker": "Ava", "text": "Entiendo. Hasta luego."}
  ],
  "labels": {"start": 0, "amable": 6, "end": 8}
}
"""


def main() -> None:
    print("=== Demo del Motor de Novelas Visuales ===\n")

    # Crear el motor
    engine = PyEngine(SCRIPT_JSON)

    # Mostrar evento inicial (Scene)
    print("1. Evento inicial:")
    print(f"   {engine.current_event()}")

    # Mostrar estado visual
    print("\n2. Estado visual:")
    visual = engine.visual_state()
    print(f"   Fondo: {visual.get('background')}")
    print(f"   Personajes: {visual.get('characters')}")

    # Avanzar al diálogo
    engine.step()
    print("\n3. Después de step():")
    print(f"   {engine.current_event()}")

    # Avanzar (set_flag)
    engine.step()
    print("\n4. Después de set_flag:")
    print(f"   {engine.current_event()}")

    # Avanzar al choice
    engine.step()
    choice = engine.current_event()
    print("\n5. Elección:")
    print(f"   Prompt: {choice.get('prompt')}")
    for i, opt in enumerate(choice.get("options", [])):
        print(f"   [{i}] {opt.get('text')}")

    # Elegir opción 0 (amable)
    engine.choose(0)
    print("\n6. Después de elegir opción 0:")
    print(f"   {engine.current_event()}")

    print("\n=== Demo completada ===")


if __name__ == "__main__":
    main()
