//! Ejemplo completo de uso del motor con interfaz gráfica.
//!
//! Ejecutar con: `cargo run --example gui_demo`

use visual_novel_gui::{run_app, VnConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Guion de ejemplo con múltiples eventos
    let script_json = r#"
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
        {"type": "dialogue", "speaker": "Sistema", "text": "El motor soporta guardado y carga de partidas con verificación de integridad."},
        {"type": "dialogue", "speaker": "Sistema", "text": "También incluye un historial de diálogos navegable."},
        {"type": "jump", "target": "end"},
        {"type": "dialogue", "speaker": "Sistema", "text": "¡Gracias por probar el motor!"},
        {"type": "set_flag", "key": "demo_completada", "value": true}
      ],
      "labels": {"start": 0, "info": 5, "end": 8}
    }
    "#;

    // Configuración de la ventana
    let config = VnConfig {
        title: "Demo - Visual Novel Engine".to_string(),
        width: Some(1280.0),
        height: Some(720.0),
        fullscreen: false,
        scale_factor: None, // Auto-detectar
        ..Default::default()
    };

    println!("Iniciando demo...");
    println!("Controles:");
    println!("  ESC  - Menú de configuración (guardar/cargar)");
    println!("  F12  - Inspector de depuración");

    run_app(script_json.to_string(), Some(config))?;

    println!("Demo finalizada.");
    Ok(())
}
