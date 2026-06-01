use super::*;

pub(super) fn visual_render_commands(visual: &VisualState) -> Vec<RenderCommand> {
    let mut commands = vec![RenderCommand::Clear {
        color: "stage.background".to_string(),
    }];
    if let Some(background) = &visual.background {
        commands.push(RenderCommand::Image {
            asset: background.to_string(),
            rect: LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            fit: ImageFit::Cover,
            z: -100,
        });
    }
    let count = visual.characters.len().max(1);
    for (index, character) in visual.characters.iter().enumerate() {
        let x = character
            .x
            .map(|value| value as f32)
            .unwrap_or_else(|| character_slot_x(index, count));
        let y = character.y.map(|value| value as f32).unwrap_or(140.0);
        let scale = character.scale.unwrap_or(1.0).clamp(0.25, 4.0);
        let width = 260.0 * scale;
        let height = 420.0 * scale;
        let asset = character
            .expression
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| character.name.to_string());
        commands.push(RenderCommand::Image {
            asset,
            rect: LayoutRect {
                x: x - width * 0.5,
                y,
                width,
                height,
            },
            fit: ImageFit::Contain,
            z: index as i32,
        });
    }
    commands
}

fn character_slot_x(index: usize, count: usize) -> f32 {
    if count == 1 {
        return 640.0;
    }
    let left = 360.0;
    let right = 920.0;
    left + (right - left) * (index as f32 / (count.saturating_sub(1)) as f32)
}
