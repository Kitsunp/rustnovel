use super::asset_refs::{
    collect_character_asset_refs, collect_scene_patch_asset_refs, AssetRefCollector,
};
use super::capabilities::{fallback_cap, preview_only_cap, subgraph_cap};
use super::fields::*;
use super::*;

const NODE_SPECS: [NodeSpec; 16] = [
    NodeKind::Start.spec(),
    NodeKind::End.spec(),
    NodeKind::Dialogue.spec(),
    NodeKind::Choice.spec(),
    NodeKind::Scene.spec(),
    NodeKind::Jump.spec(),
    NodeKind::SetVariable.spec(),
    NodeKind::SetFlag.spec(),
    NodeKind::ScenePatch.spec(),
    NodeKind::JumpIf.spec(),
    NodeKind::AudioAction.spec(),
    NodeKind::Transition.spec(),
    NodeKind::CharacterPlacement.spec(),
    NodeKind::ExtCall.spec(),
    NodeKind::SubgraphCall.spec(),
    NodeKind::GenericEvent.spec(),
];

impl NodeKind {
    pub const ALL: &'static [Self] = &[
        Self::Start,
        Self::End,
        Self::Dialogue,
        Self::Choice,
        Self::Scene,
        Self::Jump,
        Self::SetVariable,
        Self::SetFlag,
        Self::ScenePatch,
        Self::JumpIf,
        Self::AudioAction,
        Self::Transition,
        Self::CharacterPlacement,
        Self::ExtCall,
        Self::SubgraphCall,
        Self::GenericEvent,
    ];

    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Dialogue => "dialogue",
            Self::Choice => "choice",
            Self::Scene => "scene",
            Self::Jump => "jump",
            Self::SetVariable => "set_variable",
            Self::SetFlag => "set_flag",
            Self::ScenePatch => "scene_patch",
            Self::JumpIf => "jump_if",
            Self::AudioAction => "audio_action",
            Self::Transition => "transition",
            Self::CharacterPlacement => "character_placement",
            Self::ExtCall => "ext_call",
            Self::SubgraphCall => "subgraph_call",
            Self::GenericEvent => "generic_event",
        }
    }

    pub const fn spec(self) -> NodeSpec {
        NodeSpec {
            kind: self,
            stable_name: self.stable_name(),
            display_name: self.display_name(),
            contract_name: self.contract_name(),
            event_kind: self.event_kind(),
            ports: self.ports(),
            editor_schema: InspectorSchema {
                field_paths: self.field_paths(),
            },
            asset_ref_field_paths: self.asset_ref_field_paths(),
            capabilities: self.capabilities(),
        }
    }

    const fn display_name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::Dialogue => "Dialogue",
            Self::Choice => "Choice",
            Self::Scene => "Scene",
            Self::Jump => "Jump",
            Self::SetVariable => "Set Var",
            Self::SetFlag => "Set Flag",
            Self::ScenePatch => "Scene Patch",
            Self::JumpIf => "Branch (If)",
            Self::AudioAction => "Audio",
            Self::Transition => "Transition",
            Self::CharacterPlacement => "Placement",
            Self::ExtCall => "ExtCall",
            Self::SubgraphCall => "Subgraph Call",
            Self::GenericEvent => "Generic Event",
        }
    }

    const fn contract_name(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::SetVariable => "SetVariable",
            Self::ScenePatch => "ScenePatch",
            Self::CharacterPlacement => "SetCharacterPosition",
            Self::SubgraphCall => "SubgraphCall",
            Self::GenericEvent => "Generic/EventRaw",
            _ => match self.event_kind() {
                Some(kind) => kind.contract_name(),
                None => self.display_name(),
            },
        }
    }

    const fn event_kind(self) -> Option<EventKind> {
        match self {
            Self::Dialogue => Some(EventKind::Dialogue),
            Self::Choice => Some(EventKind::Choice),
            Self::Scene => Some(EventKind::Scene),
            Self::Jump => Some(EventKind::Jump),
            Self::SetVariable => Some(EventKind::SetVar),
            Self::SetFlag => Some(EventKind::SetFlag),
            Self::ScenePatch => Some(EventKind::Patch),
            Self::JumpIf => Some(EventKind::JumpIf),
            Self::AudioAction => Some(EventKind::AudioAction),
            Self::Transition => Some(EventKind::Transition),
            Self::CharacterPlacement => Some(EventKind::SetCharacterPosition),
            Self::ExtCall => Some(EventKind::ExtCall),
            Self::Start | Self::End | Self::SubgraphCall | Self::GenericEvent => None,
        }
    }

    const fn ports(self) -> PortSpec {
        match self {
            Self::Start => PortSpec {
                accepts_incoming: false,
                output: PortOutput::Linear,
            },
            Self::End => PortSpec {
                accepts_incoming: true,
                output: PortOutput::None,
            },
            Self::Choice => PortSpec {
                accepts_incoming: true,
                output: PortOutput::ChoiceOptions,
            },
            Self::Jump => PortSpec {
                accepts_incoming: true,
                output: PortOutput::SingleTarget,
            },
            Self::JumpIf => PortSpec {
                accepts_incoming: true,
                output: PortOutput::ConditionalTrueFalse,
            },
            Self::SubgraphCall => PortSpec {
                accepts_incoming: true,
                output: PortOutput::SubgraphExit,
            },
            _ => PortSpec {
                accepts_incoming: true,
                output: PortOutput::Linear,
            },
        }
    }

    const fn field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Start => START_FIELDS,
            Self::End => END_FIELDS,
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_NODE_FIELDS,
            Self::Jump => JUMP_RAW_FIELDS,
            Self::SetVariable | Self::SetFlag => STATE_RAW_FIELDS,
            Self::ScenePatch => PATCH_RAW_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::AudioAction => AUDIO_RAW_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::CharacterPlacement => CHARACTER_POSITION_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::SubgraphCall => SUBGRAPH_FIELDS,
            Self::GenericEvent => GENERIC_FIELDS,
        }
    }

    const fn asset_ref_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Scene => SCENE_ASSET_FIELDS,
            Self::ScenePatch => PATCH_ASSET_FIELDS,
            Self::AudioAction => AUDIO_ASSET_FIELDS,
            _ => match self.event_kind() {
                Some(kind) => kind.asset_ref_field_paths(),
                None => EMPTY_FIELDS,
            },
        }
    }

    const fn capabilities(self) -> EventCapabilities {
        match self {
            Self::Start | Self::End => preview_only_cap(false),
            Self::SubgraphCall => subgraph_cap(),
            Self::GenericEvent => fallback_cap(),
            _ => match self.event_kind() {
                Some(kind) => kind.capabilities(),
                None => fallback_cap(),
            },
        }
    }
}

pub fn node_specs() -> &'static [NodeSpec] {
    &NODE_SPECS
}

pub fn node_spec(kind: NodeKind) -> &'static NodeSpec {
    &NODE_SPECS[node_kind_index(kind)]
}

pub fn node_behavior(kind: NodeKind) -> StaticNodeBehavior {
    StaticNodeBehavior::new(kind)
}

pub fn node_behavior_for_authoring_node(node: &StoryNode) -> StaticNodeBehavior {
    node_behavior(node_kind_for_authoring_node(node))
}

pub fn node_spec_for_authoring_node(node: &StoryNode) -> &'static NodeSpec {
    node_spec(node_kind_for_authoring_node(node))
}

pub fn node_asset_refs_for_authoring_node(node: &StoryNode) -> Vec<String> {
    let mut refs = AssetRefCollector::default();
    match node {
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => {
            refs.push_optional(background);
            refs.push_optional(music);
            collect_character_asset_refs(characters, &mut refs);
        }
        StoryNode::ScenePatch(patch) => collect_scene_patch_asset_refs(patch, &mut refs),
        StoryNode::AudioAction { asset, .. } => refs.push_optional(asset),
        StoryNode::Generic(event) => refs.extend(event_asset_refs_for_raw(event)),
        _ => {}
    }
    refs.into_vec()
}

pub fn node_from_event_raw(event: &EventRaw) -> StoryNode {
    match event {
        EventRaw::Dialogue(dialogue) => StoryNode::Dialogue {
            speaker: dialogue.speaker.clone(),
            text: dialogue.text.clone(),
        },
        EventRaw::Choice(choice) => StoryNode::Choice {
            prompt: choice.prompt.clone(),
            options: choice
                .options
                .iter()
                .map(|option| option.text.clone())
                .collect(),
        },
        EventRaw::Scene(scene) => StoryNode::Scene {
            profile: None,
            background: scene.background.clone(),
            music: scene.music.clone(),
            characters: scene.characters.clone(),
        },
        EventRaw::Jump { target } => StoryNode::Jump {
            target: target.clone(),
        },
        EventRaw::SetFlag { key, value } => StoryNode::SetFlag {
            key: key.clone(),
            value: *value,
        },
        EventRaw::SetVar { key, value } => StoryNode::SetVariable {
            key: key.clone(),
            value: *value,
        },
        EventRaw::JumpIf { cond, target } => StoryNode::JumpIf {
            target: target.clone(),
            cond: cond.clone(),
        },
        EventRaw::Patch(patch) => StoryNode::ScenePatch(patch.clone()),
        EventRaw::AudioAction(action) => StoryNode::AudioAction {
            channel: action.channel.clone(),
            action: action.action.clone(),
            asset: action.asset.clone(),
            volume: action.volume,
            fade_duration_ms: action.fade_duration_ms,
            loop_playback: action.loop_playback,
        },
        EventRaw::Transition(transition) => StoryNode::Transition {
            kind: transition.kind.clone(),
            duration_ms: transition.duration_ms,
            color: transition.color.clone(),
        },
        EventRaw::SetCharacterPosition(pos) => StoryNode::CharacterPlacement {
            name: pos.name.clone(),
            x: pos.x,
            y: pos.y,
            scale: pos.scale,
        },
        EventRaw::ExtCall { .. } => StoryNode::Generic(event.clone()),
    }
}

pub fn node_to_event_raw_without_export_context(
    node: &StoryNode,
) -> Result<EventRaw, NodeToEventError> {
    Ok(match node {
        StoryNode::Dialogue { speaker, text } => EventRaw::Dialogue(DialogueRaw {
            speaker: speaker.clone(),
            text: text.clone(),
        }),
        StoryNode::Scene {
            background,
            music,
            characters,
            ..
        } => EventRaw::Scene(SceneUpdateRaw {
            background: background.clone(),
            music: music.clone(),
            characters: characters.clone(),
        }),
        StoryNode::SetVariable { key, value } => EventRaw::SetVar {
            key: key.clone(),
            value: *value,
        },
        StoryNode::SetFlag { key, value } => EventRaw::SetFlag {
            key: key.clone(),
            value: *value,
        },
        StoryNode::ScenePatch(patch) => EventRaw::Patch(patch.clone()),
        StoryNode::AudioAction {
            channel,
            action,
            asset,
            volume,
            fade_duration_ms,
            loop_playback,
        } => EventRaw::AudioAction(AudioActionRaw {
            channel: channel.clone(),
            action: action.clone(),
            asset: asset.clone(),
            volume: *volume,
            fade_duration_ms: *fade_duration_ms,
            loop_playback: *loop_playback,
        }),
        StoryNode::Transition {
            kind,
            duration_ms,
            color,
        } => EventRaw::Transition(SceneTransitionRaw {
            kind: kind.clone(),
            duration_ms: *duration_ms,
            color: color.clone(),
        }),
        StoryNode::CharacterPlacement { name, x, y, scale } => {
            EventRaw::SetCharacterPosition(SetCharacterPositionRaw {
                name: name.clone(),
                x: *x,
                y: *y,
                scale: *scale,
            })
        }
        StoryNode::Generic(event) => event.clone(),
        StoryNode::Choice { .. }
        | StoryNode::Jump { .. }
        | StoryNode::JumpIf { .. }
        | StoryNode::SubgraphCall { .. } => return Err(NodeToEventError::RequiresExportContext),
        StoryNode::Start | StoryNode::End => return Err(NodeToEventError::PreviewOnlyMarker),
    })
}

pub fn node_kind_for_event_raw(event: &EventRaw) -> NodeKind {
    match event {
        EventRaw::Dialogue(_) => NodeKind::Dialogue,
        EventRaw::Choice(_) => NodeKind::Choice,
        EventRaw::Scene(_) => NodeKind::Scene,
        EventRaw::Jump { .. } => NodeKind::Jump,
        EventRaw::SetFlag { .. } => NodeKind::SetFlag,
        EventRaw::SetVar { .. } => NodeKind::SetVariable,
        EventRaw::JumpIf { .. } => NodeKind::JumpIf,
        EventRaw::Patch(_) => NodeKind::ScenePatch,
        EventRaw::ExtCall { .. } => NodeKind::ExtCall,
        EventRaw::AudioAction(_) => NodeKind::AudioAction,
        EventRaw::Transition(_) => NodeKind::Transition,
        EventRaw::SetCharacterPosition(_) => NodeKind::CharacterPlacement,
    }
}

pub fn node_kind_for_authoring_node(node: &StoryNode) -> NodeKind {
    match node {
        StoryNode::Dialogue { .. } => NodeKind::Dialogue,
        StoryNode::Choice { .. } => NodeKind::Choice,
        StoryNode::Scene { .. } => NodeKind::Scene,
        StoryNode::Jump { .. } => NodeKind::Jump,
        StoryNode::SetVariable { .. } => NodeKind::SetVariable,
        StoryNode::SetFlag { .. } => NodeKind::SetFlag,
        StoryNode::ScenePatch(_) => NodeKind::ScenePatch,
        StoryNode::JumpIf { .. } => NodeKind::JumpIf,
        StoryNode::Start => NodeKind::Start,
        StoryNode::End => NodeKind::End,
        StoryNode::AudioAction { .. } => NodeKind::AudioAction,
        StoryNode::Transition { .. } => NodeKind::Transition,
        StoryNode::CharacterPlacement { .. } => NodeKind::CharacterPlacement,
        StoryNode::SubgraphCall { .. } => NodeKind::SubgraphCall,
        StoryNode::Generic(EventRaw::ExtCall { .. }) => NodeKind::ExtCall,
        StoryNode::Generic(EventRaw::SetFlag { .. }) => NodeKind::SetFlag,
        StoryNode::Generic(_) => NodeKind::GenericEvent,
    }
}

const fn node_kind_index(kind: NodeKind) -> usize {
    match kind {
        NodeKind::Start => 0,
        NodeKind::End => 1,
        NodeKind::Dialogue => 2,
        NodeKind::Choice => 3,
        NodeKind::Scene => 4,
        NodeKind::Jump => 5,
        NodeKind::SetVariable => 6,
        NodeKind::SetFlag => 7,
        NodeKind::ScenePatch => 8,
        NodeKind::JumpIf => 9,
        NodeKind::AudioAction => 10,
        NodeKind::Transition => 11,
        NodeKind::CharacterPlacement => 12,
        NodeKind::ExtCall => 13,
        NodeKind::SubgraphCall => 14,
        NodeKind::GenericEvent => 15,
    }
}
