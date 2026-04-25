"""
Ejemplo completo de uso del motor con interfaz gráfica desde Python.

Ejecutar con: python examples/python/gui_demo.py

Requisitos:
    maturin develop --features python
"""

import visual_novel_engine as vn

# Guion de ejemplo con múltiples eventos
script_json = """
{
    "script_schema_version": "1.0",
    "events": [
    {"type": "scene", "background": "assets/bg_intro.png", "music": "assets/theme.ogg", "characters": []},
    {"type": "dialogue", "speaker": "Sistema", "text": "Bienvenido a la demo del motor de novelas visuales."},
    {"type": "dialogue", "speaker": "Sistema", "text": "Puedes usar ESC para abrir el menú de configuración."},
    {"type": "dialogue", "speaker": "Sistema", "text": "F12 abre el inspector de depuración."},
    {"type": "choice", "prompt": "¿Qué deseas hacer?", "options": [
      {"text": "Ver más información", "target": "info"},
      {"text": "Terminar demo", "target": "end"}
    ]},
    {"type": "dialogue", "speaker": "Sistema", "text": "El motor soporta guardado y carga de partidas."},
    {"type": "jump", "target": "end"},
    {"type": "dialogue", "speaker": "Sistema", "text": "¡Gracias por probar el motor!"}
  ],
  "labels": {"start": 0, "info": 5, "end": 7}
}
"""


def main():
    # Configuración de la ventana
    config = vn.VnConfig(
        title="Demo - Visual Novel Engine (Python)",
        width=1280.0,
        height=720.0,
        fullscreen=False,
    )

    print("Iniciando demo...")
    print("Controles:")
    print("  ESC  - Menú de configuración (guardar/cargar)")
    print("  F12  - Inspector de depuración")

    try:
        vn.run_visual_novel(script_json, config)
        print("Demo finalizada.")
    except Exception as exc:
        print(f"Error: {exc}")


if __name__ == "__main__":
    main()
