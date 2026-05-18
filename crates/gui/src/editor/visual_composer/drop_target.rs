use crate::editor::{AssetFieldTarget, StoryNode};

pub(crate) struct DraggedAsset<'a> {
    pub(crate) kind: &'a str,
    pub(crate) name: &'a str,
    pub(crate) path: &'a str,
}

impl<'a> DraggedAsset<'a> {
    pub(crate) fn parse(payload: &'a str) -> Option<Self> {
        let payload = payload.strip_prefix("asset://")?;
        let mut lines = payload.lines();
        let header = lines.next()?;
        let path_override = lines.next();
        let (kind, name) = header.split_once('/')?;
        Some(Self {
            kind,
            name,
            path: path_override.unwrap_or(name),
        })
    }
}

pub(crate) fn assignment_for_dropped_asset(
    kind: &str,
    asset_path: &str,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
) -> Option<(u32, AssetFieldTarget, String)> {
    let node_id = selected_node_id?;
    let node = selected_node?;
    let target = match (kind, node) {
        ("bg", StoryNode::Scene { .. }) => AssetFieldTarget::SceneBackground,
        ("bg", StoryNode::ScenePatch(_)) => AssetFieldTarget::ScenePatchBackground,
        ("audio", StoryNode::Scene { .. }) => AssetFieldTarget::SceneMusic,
        ("audio", StoryNode::ScenePatch(_)) => AssetFieldTarget::ScenePatchMusic,
        ("audio", StoryNode::AudioAction { .. }) => AssetFieldTarget::AudioActionAsset,
        _ => return None,
    };
    Some((node_id, target, asset_path.to_string()))
}

pub(crate) fn character_drop_target_node(
    kind: &str,
    selected_node_id: Option<u32>,
    selected_node: Option<&StoryNode>,
) -> Option<u32> {
    let node_id = selected_node_id?;
    let node = selected_node?;
    (kind == "char" && matches!(node, StoryNode::Scene { .. } | StoryNode::ScenePatch(_)))
        .then_some(node_id)
}
