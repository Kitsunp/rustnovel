use std::path::{Path, PathBuf};

use crate::editor::{AssetFieldTarget, AssetImportKind};

pub fn pick_asset_file(kind: AssetImportKind, project_root: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .set_title(kind.dialog_title())
        .add_filter(kind.label(), kind.allowed_extensions());
    if let Some(root) = project_root {
        dialog = dialog.set_directory(root);
    }
    dialog.pick_file()
}

pub fn asset_node_field_path(node_id: u32, target: AssetFieldTarget) -> String {
    match target {
        AssetFieldTarget::SceneBackground => format!("graph.nodes[{node_id}].background"),
        AssetFieldTarget::SceneMusic => format!("graph.nodes[{node_id}].music"),
        AssetFieldTarget::SceneCharacterExpression(idx) => {
            format!("graph.nodes[{node_id}].characters[{idx}].expression")
        }
        AssetFieldTarget::ScenePatchBackground => {
            format!("graph.nodes[{node_id}].patch.background")
        }
        AssetFieldTarget::ScenePatchMusic => format!("graph.nodes[{node_id}].patch.music"),
        AssetFieldTarget::ScenePatchAddCharacterExpression(idx) => {
            format!("graph.nodes[{node_id}].patch.add[{idx}].expression")
        }
        AssetFieldTarget::AudioActionAsset => format!("graph.nodes[{node_id}].audio.asset"),
    }
}

pub fn stringify_optional_asset(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "<none>".to_string())
}

pub fn normalized_character_name(name: &str, asset_path: &str) -> String {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    std::path::Path::new(asset_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitized_identifier)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Character".to_string())
}

fn sanitized_identifier(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

pub fn upsert_character_asset(
    characters: &mut Vec<visual_novel_engine::runtime::CharacterPlacementRaw>,
    name: &str,
    imported: &str,
    x: i32,
    y: i32,
) -> (usize, String, String) {
    if let Some((idx, character)) = characters.iter_mut().enumerate().find(|(_, character)| {
        character.name == name && character.expression.as_deref() == Some(imported)
    }) {
        let before = character_snapshot(character);
        character.x = Some(x);
        character.y = Some(y);
        character.scale = character.scale.or(Some(1.0));
        let after = character_snapshot(character);
        return (idx, before, after);
    }

    let idx = characters.len();
    characters.push(visual_novel_engine::runtime::CharacterPlacementRaw {
        name: name.to_string(),
        expression: Some(imported.to_string()),
        position: None,
        x: Some(x),
        y: Some(y),
        scale: Some(1.0),
    });
    let after = character_snapshot(&characters[idx]);
    (idx, "<none>".to_string(), after)
}

fn character_snapshot(character: &visual_novel_engine::runtime::CharacterPlacementRaw) -> String {
    format!(
        "{}|{}|x={}|y={}|scale={}",
        character.name,
        character.expression.as_deref().unwrap_or("<none>"),
        character
            .x
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string()),
        character
            .y
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string()),
        character
            .scale
            .map(|value| value.to_string())
            .unwrap_or_else(|| "<none>".to_string())
    )
}
