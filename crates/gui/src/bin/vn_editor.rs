//! Visual Novel Editor - A visual authoring tool for visual novels.
//!
//! This binary launches the editor workbench for creating and editing
//! visual novel scripts with timeline, graph, and viewport panels.

fn main() {
    let _ = tracing_subscriber::fmt::try_init();
    match visual_novel_gui::run_editor() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error running editor: {e}");
            std::process::exit(1);
        }
    }
}
