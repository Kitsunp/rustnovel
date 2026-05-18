use eframe::egui;

/// Draws a bezier connection curve between two points.
pub fn draw_bezier_connection(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2) {
    let delta = to - from;
    if delta.length_sq() <= f32::EPSILON {
        return;
    }

    let (control1, control2) = bezier_control_points(from, to);

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

    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 180, 100)),
    ));

    let arrow_size = 8.0;
    let mut arrow_dir = to - control2;
    if arrow_dir.length_sq() <= f32::EPSILON {
        arrow_dir = delta;
    }
    if arrow_dir.length_sq() <= f32::EPSILON {
        return;
    }
    let dir = arrow_dir.normalized();
    let arrow_left = to - dir * arrow_size + dir.rot90() * arrow_size * 0.5;
    let arrow_right = to - dir * arrow_size - dir.rot90() * arrow_size * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![to, arrow_left, arrow_right],
        egui::Color32::from_rgb(100, 180, 100),
        egui::Stroke::NONE,
    ));
}

pub(crate) fn bezier_control_points(from: egui::Pos2, to: egui::Pos2) -> (egui::Pos2, egui::Pos2) {
    let delta = to - from;
    if delta.length_sq() <= f32::EPSILON {
        return (from, to);
    }

    let horizontal_bias = delta.x.abs() >= delta.y.abs();
    if horizontal_bias {
        let direction = delta.x.signum();
        let magnitude = (delta.x.abs() * 0.5).clamp(36.0, 220.0);
        let control_offset = magnitude * direction;
        (
            from + egui::vec2(control_offset, 0.0),
            to - egui::vec2(control_offset, 0.0),
        )
    } else {
        let direction = delta.y.signum();
        let magnitude = (delta.y.abs() * 0.5).clamp(36.0, 220.0);
        let control_offset = magnitude * direction;
        (
            from + egui::vec2(0.0, control_offset),
            to - egui::vec2(0.0, control_offset),
        )
    }
}
