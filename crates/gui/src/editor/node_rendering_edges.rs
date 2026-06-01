use eframe::egui;
use visual_novel_engine::authoring::StoryNode;

use crate::editor::node_types::CHOICE_OPTION_CELL_WIDTH;

const DEFAULT_EDGE_COLOR: egui::Color32 = egui::Color32::from_rgb(100, 180, 100);
const ROUTE_COLORS: [(u8, u8, u8); 6] = [
    (112, 207, 128),
    (103, 181, 255),
    (245, 188, 86),
    (207, 132, 255),
    (91, 211, 204),
    (255, 137, 112),
];
const ROUTE_LABEL_MIN_ZOOM: f32 = 0.5;

struct RouteLabel<'a> {
    text: &'a str,
    rect: egui::Rect,
}

/// Draws a bezier connection curve between two points.
pub fn draw_bezier_connection(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2) {
    draw_bezier_connection_with_label(painter, from, to, DEFAULT_EDGE_COLOR, None, 1.0);
}

pub fn draw_story_connection(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    source: &StoryNode,
    from_port: usize,
    zoom: f32,
) {
    let label = route_label_for_source(source, from_port);
    let color = if label.is_some() {
        route_color(from_port)
    } else {
        DEFAULT_EDGE_COLOR
    };
    let label_rect = label
        .as_deref()
        .and_then(|label| route_label_rect_for_source(from, to, label, zoom, source, from_port));
    draw_bezier_connection_with_label_rect(
        painter,
        from,
        to,
        color,
        label.as_deref(),
        zoom,
        label_rect,
    );
}

pub fn draw_story_connection_projected(
    painter: &egui::Painter,
    from_graph: egui::Pos2,
    to_graph: egui::Pos2,
    src: &StoryNode,
    from_port: usize,
    zoom: f32,
    project: impl Fn(egui::Pos2) -> egui::Pos2,
) {
    let label = route_label_for_source(src, from_port);
    let color = if label.is_some() {
        route_color(from_port)
    } else {
        DEFAULT_EDGE_COLOR
    };
    let label_rect = label.as_deref().and_then(|label| {
        route_label_rect_for_source(from_graph, to_graph, label, zoom, src, from_port)
    });
    draw_bezier_connection_projected_with_label_rect(
        painter,
        from_graph,
        to_graph,
        color,
        zoom,
        label
            .as_deref()
            .zip(label_rect)
            .map(|(text, rect)| RouteLabel { text, rect }),
        project,
    );
}

pub fn connection_intersects_viewport(
    from: egui::Pos2,
    to: egui::Pos2,
    viewport: egui::Rect,
    zoom: f32,
) -> bool {
    connection_bounds(from, to, zoom).intersects(viewport)
}

pub fn connection_intersects_viewport_projected(
    from_graph: egui::Pos2,
    to_graph: egui::Pos2,
    viewport: egui::Rect,
    zoom: f32,
    project: impl Fn(egui::Pos2) -> egui::Pos2,
) -> bool {
    connection_bounds_projected(from_graph, to_graph, zoom, project).intersects(viewport)
}

pub fn connection_bounds(from: egui::Pos2, to: egui::Pos2, zoom: f32) -> egui::Rect {
    let (control1, control2) = bezier_control_points(from, to);
    egui::Rect::from_min_max(
        egui::pos2(
            from.x.min(to.x).min(control1.x).min(control2.x),
            from.y.min(to.y).min(control1.y).min(control2.y),
        ),
        egui::pos2(
            from.x.max(to.x).max(control1.x).max(control2.x),
            from.y.max(to.y).max(control1.y).max(control2.y),
        ),
    )
    .expand((48.0 * zoom.clamp(0.5, 1.5)).max(24.0))
}

pub fn connection_bounds_projected(
    from_graph: egui::Pos2,
    to_graph: egui::Pos2,
    zoom: f32,
    project: impl Fn(egui::Pos2) -> egui::Pos2,
) -> egui::Rect {
    let mut points = bezier_curve_points(from_graph, to_graph)
        .into_iter()
        .map(project);
    let Some(first) = points.next() else {
        return egui::Rect::NOTHING;
    };
    let mut min = first;
    let mut max = first;
    for point in points {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    egui::Rect::from_min_max(min, max).expand((48.0 * zoom.clamp(0.5, 1.5)).max(24.0))
}

pub fn route_color(port: usize) -> egui::Color32 {
    let (r, g, b) = ROUTE_COLORS[port % ROUTE_COLORS.len()];
    egui::Color32::from_rgb(r, g, b)
}

pub fn route_label_for_source(source: &StoryNode, from_port: usize) -> Option<String> {
    match source {
        StoryNode::Choice { options, .. } => options
            .get(from_port)
            .map(|option| format!("{}. {}", from_port + 1, option)),
        StoryNode::JumpIf { .. } => Some(if from_port == 0 { "True" } else { "False" }.to_string()),
        _ => None,
    }
}

pub fn draw_bezier_connection_with_label(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
    label: Option<&str>,
    zoom: f32,
) {
    let label_rect = label.and_then(|label| route_label_rect(from, to, label, zoom));
    draw_bezier_connection_with_label_rect(painter, from, to, color, label, zoom, label_rect);
}

fn draw_bezier_connection_with_label_rect(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
    label: Option<&str>,
    zoom: f32,
    label_rect: Option<egui::Rect>,
) {
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
        egui::Stroke::new((2.0 * zoom.clamp(0.75, 1.5)).clamp(1.5, 3.0), color),
    ));

    let arrow_size = 8.0 * zoom.clamp(0.8, 1.4);
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
        color,
        egui::Stroke::NONE,
    ));

    if let (Some(label), Some(rect)) = (label, label_rect) {
        draw_route_label_at_rect(painter, rect, label, color, zoom);
    }
}

fn draw_bezier_connection_projected_with_label_rect(
    painter: &egui::Painter,
    from_graph: egui::Pos2,
    to_graph: egui::Pos2,
    color: egui::Color32,
    zoom: f32,
    label: Option<RouteLabel<'_>>,
    project: impl Fn(egui::Pos2) -> egui::Pos2,
) {
    if (to_graph - from_graph).length_sq() <= f32::EPSILON {
        return;
    }
    let graph_points = bezier_curve_points(from_graph, to_graph);
    let points: Vec<egui::Pos2> = graph_points.iter().copied().map(&project).collect();
    painter.add(egui::Shape::line(
        points.clone(),
        egui::Stroke::new((2.0 * zoom.clamp(0.75, 1.5)).clamp(1.5, 3.0), color),
    ));

    let arrow_size = 8.0 * zoom.clamp(0.8, 1.4);
    let Some(to) = points.last().copied() else {
        return;
    };
    let mut arrow_dir = points
        .iter()
        .rev()
        .skip(1)
        .find_map(|previous| {
            let dir = to - *previous;
            (dir.length_sq() > f32::EPSILON).then_some(dir)
        })
        .unwrap_or(to - project(from_graph));
    if arrow_dir.length_sq() <= f32::EPSILON {
        return;
    }
    arrow_dir = arrow_dir.normalized();
    let arrow_left = to - arrow_dir * arrow_size + arrow_dir.rot90() * arrow_size * 0.5;
    let arrow_right = to - arrow_dir * arrow_size - arrow_dir.rot90() * arrow_size * 0.5;
    painter.add(egui::Shape::convex_polygon(
        vec![to, arrow_left, arrow_right],
        color,
        egui::Stroke::NONE,
    ));

    if let Some(label) = label {
        let rect = egui::Rect::from_center_size(project(label.rect.center()), label.rect.size());
        draw_route_label_at_rect(painter, rect, label.text, color, zoom);
    }
}

pub fn bezier_control_points(from: egui::Pos2, to: egui::Pos2) -> (egui::Pos2, egui::Pos2) {
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

pub fn bezier_curve_points(from: egui::Pos2, to: egui::Pos2) -> Vec<egui::Pos2> {
    let (control1, control2) = bezier_control_points(from, to);
    (0..=20)
        .map(|i| {
            let t = i as f32 / 20.0;
            cubic_bezier_point(from, control1, control2, to, t)
        })
        .collect()
}

pub fn bezier_point(from: egui::Pos2, to: egui::Pos2, t: f32) -> egui::Pos2 {
    let (control1, control2) = bezier_control_points(from, to);
    let t = t.clamp(0.0, 1.0);
    cubic_bezier_point(from, control1, control2, to, t)
}

fn cubic_bezier_point(
    from: egui::Pos2,
    control1: egui::Pos2,
    control2: egui::Pos2,
    to: egui::Pos2,
    t: f32,
) -> egui::Pos2 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    egui::pos2(
        mt3 * from.x + 3.0 * mt2 * t * control1.x + 3.0 * mt * t2 * control2.x + t3 * to.x,
        mt3 * from.y + 3.0 * mt2 * t * control1.y + 3.0 * mt * t2 * control2.y + t3 * to.y,
    )
}

fn draw_route_label_at_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    color: egui::Color32,
    zoom: f32,
) {
    let label = truncate_route_label(label, route_label_max_chars(zoom));
    let font_size = (11.0 * zoom).clamp(8.0, 12.0);
    painter.rect_filled(
        rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(18, 20, 28, 224),
    );
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, color));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(font_size),
        egui::Color32::WHITE,
    );
}

pub fn route_label_rect(
    from: egui::Pos2,
    to: egui::Pos2,
    label: &str,
    zoom: f32,
) -> Option<egui::Rect> {
    if zoom < ROUTE_LABEL_MIN_ZOOM {
        return None;
    }
    let size = route_label_size(label, zoom, 190.0);
    let delta = to - from;
    let horizontal_bias = delta.x.abs() >= delta.y.abs();
    let t = if horizontal_bias { 0.62 } else { 0.42 };
    let offset = if horizontal_bias {
        let direction = if delta.y >= 0.0 { -1.0 } else { 1.0 };
        egui::vec2(0.0, direction * 12.0 * zoom.clamp(0.5, 1.2))
    } else {
        egui::vec2(0.0, -10.0 * zoom.clamp(0.5, 1.2))
    };
    let center = bezier_point(from, to, t) + offset;
    Some(egui::Rect::from_center_size(center, size))
}

pub fn route_label_rect_for_source(
    from: egui::Pos2,
    to: egui::Pos2,
    label: &str,
    zoom: f32,
    source: &StoryNode,
    from_port: usize,
) -> Option<egui::Rect> {
    match source {
        StoryNode::Choice { options, .. } => {
            choice_route_label_rect(from, to, label, zoom, options.len(), from_port)
        }
        _ => route_label_rect(from, to, label, zoom),
    }
}

fn choice_route_label_rect(
    from: egui::Pos2,
    to: egui::Pos2,
    label: &str,
    zoom: f32,
    route_count: usize,
    from_port: usize,
) -> Option<egui::Rect> {
    if zoom < ROUTE_LABEL_MIN_ZOOM {
        return None;
    }
    let route_count = route_count.max(1);
    let route_index = from_port.min(route_count - 1);
    let delta = to - from;
    let horizontal_bias = delta.x.abs() >= delta.y.abs();
    let size = route_label_size(
        label,
        zoom,
        if horizontal_bias {
            170.0
        } else {
            CHOICE_OPTION_CELL_WIDTH - 10.0
        },
    );
    let gap = 8.0 * zoom.clamp(0.5, 1.2);
    let center = if horizontal_bias {
        let x_dir = if delta.x < 0.0 { -1.0 } else { 1.0 };
        let middle = (route_count - 1) as f32 * 0.5;
        let lane_offset = (route_index as f32 - middle) * (size.y + 6.0);
        egui::pos2(from.x + x_dir * (size.x * 0.5 + gap), from.y + lane_offset)
    } else {
        let y_dir = if delta.y < 0.0 { -1.0 } else { 1.0 };
        egui::pos2(from.x, from.y + y_dir * (size.y * 0.5 + gap))
    };
    Some(egui::Rect::from_center_size(center, size))
}

fn route_label_size(label: &str, zoom: f32, max_width: f32) -> egui::Vec2 {
    let label = truncate_route_label(label, route_label_max_chars(zoom));
    let font_size = (11.0 * zoom).clamp(8.0, 12.0);
    let width = (label.chars().count() as f32 * font_size * 0.58 + 14.0).clamp(32.0, max_width);
    let height = (font_size + 8.0).clamp(16.0, 22.0);
    egui::vec2(width, height)
}

fn route_label_max_chars(zoom: f32) -> usize {
    if zoom < 0.75 {
        14
    } else {
        28
    }
}

fn truncate_route_label(label: &str, max_chars: usize) -> String {
    let mut chars = label.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
