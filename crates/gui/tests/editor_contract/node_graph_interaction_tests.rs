use super::*;

fn pos(x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(x, y)
}

#[path = "node_graph_interaction_tests/branching.rs"]
mod branching;
#[path = "node_graph_interaction_tests/connections.rs"]
mod connections;
#[path = "node_graph_interaction_tests/selection.rs"]
mod selection;
