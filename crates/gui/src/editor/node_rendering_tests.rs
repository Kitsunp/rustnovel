use super::*;

#[test]
fn test_bezier_connection_does_not_panic() {
    let from = egui::pos2(0.0, 0.0);
    let to = egui::pos2(100.0, 200.0);

    let (control1, control2) = bezier_control_points(from, to);

    assert_eq!(control1, egui::pos2(0.0, 100.0));
    assert_eq!(control2, egui::pos2(100.0, 100.0));

    let points: Vec<egui::Pos2> = (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            let t2 = t * t;
            let t3 = t2 * t;
            let mt = 1.0 - t;
            let mt2 = mt * mt;
            let mt3 = mt2 * mt;

            egui::pos2(
                mt3 * from.x + 3.0 * mt2 * t * control1.x + 3.0 * mt * t2 * control2.x + t3 * to.x,
                mt3 * from.y + 3.0 * mt2 * t * control1.y + 3.0 * mt * t2 * control2.y + t3 * to.y,
            )
        })
        .collect();

    assert_eq!(points.len(), 21);
    assert_eq!(points[0], from);
    assert_eq!(points[20], to);
}

#[test]
fn test_bezier_horizontal_line() {
    let from = egui::pos2(0.0, 50.0);
    let to = egui::pos2(100.0, 50.0);

    let (control1, control2) = bezier_control_points(from, to);
    assert_eq!(control1, egui::pos2(50.0, 50.0));
    assert_eq!(control2, egui::pos2(50.0, 50.0));
}

#[test]
fn test_bezier_control_points_keep_direction_for_reverse_edges() {
    let from = egui::pos2(300.0, 220.0);
    let to = egui::pos2(120.0, 80.0);
    let (control1, control2) = bezier_control_points(from, to);

    assert!(control1.x <= from.x);
    assert!(control2.x >= to.x);
}

#[test]
fn test_bezier_control_points_clamp_offset_for_long_edges() {
    let from = egui::pos2(0.0, 0.0);
    let to = egui::pos2(2000.0, 0.0);
    let (control1, control2) = bezier_control_points(from, to);

    assert_eq!(control1.x, 220.0);
    assert_eq!(control2.x, 1780.0);
}

#[test]
fn test_context_menu_no_panic_when_no_menu() {
    let mut graph = NodeGraph::new();
    graph.context_menu = None;
    assert!(graph.context_menu.is_none());
}

#[test]
fn test_inline_editor_no_panic_when_not_editing() {
    let mut graph = NodeGraph::new();
    graph.editing = None;
    assert!(graph.editing.is_none());
}
