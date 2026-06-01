use super::asset_refs::{
    collect_character_asset_refs, collect_scene_patch_asset_refs, AssetRefCollector,
};
use super::capabilities::{host_required_cap, runtime_real_cap};
use super::fields::*;
use super::*;

const EVENT_SPECS: [EventSpec; 12] = [
    EventKind::Dialogue.spec(),
    EventKind::Choice.spec(),
    EventKind::Scene.spec(),
    EventKind::Jump.spec(),
    EventKind::SetFlag.spec(),
    EventKind::SetVar.spec(),
    EventKind::JumpIf.spec(),
    EventKind::Patch.spec(),
    EventKind::ExtCall.spec(),
    EventKind::AudioAction.spec(),
    EventKind::Transition.spec(),
    EventKind::SetCharacterPosition.spec(),
];

impl EventKind {
    pub const ALL: &'static [Self] = &[
        Self::Dialogue,
        Self::Choice,
        Self::Scene,
        Self::Jump,
        Self::SetFlag,
        Self::SetVar,
        Self::JumpIf,
        Self::Patch,
        Self::ExtCall,
        Self::AudioAction,
        Self::Transition,
        Self::SetCharacterPosition,
    ];

    pub const STABLE_NAMES: &'static [&'static str] = &[
        "dialogue",
        "choice",
        "scene",
        "jump",
        "set_flag",
        "set_var",
        "jump_if",
        "patch",
        "ext_call",
        "audio_action",
        "transition",
        "set_character_position",
    ];

    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Dialogue => "dialogue",
            Self::Choice => "choice",
            Self::Scene => "scene",
            Self::Jump => "jump",
            Self::SetFlag => "set_flag",
            Self::SetVar => "set_var",
            Self::JumpIf => "jump_if",
            Self::Patch => "patch",
            Self::ExtCall => "ext_call",
            Self::AudioAction => "audio_action",
            Self::Transition => "transition",
            Self::SetCharacterPosition => "set_character_position",
        }
    }

    pub const fn contract_name(self) -> &'static str {
        match self {
            Self::Dialogue => "Dialogue",
            Self::Choice => "Choice",
            Self::Scene => "Scene",
            Self::Jump => "Jump",
            Self::SetFlag => "SetFlag",
            Self::SetVar => "SetVariable",
            Self::JumpIf => "JumpIf",
            Self::Patch => "ScenePatch",
            Self::ExtCall => "ExtCall",
            Self::AudioAction => "AudioAction",
            Self::Transition => "Transition",
            Self::SetCharacterPosition => "SetCharacterPosition",
        }
    }

    pub const fn spec(self) -> EventSpec {
        EventSpec {
            kind: self,
            stable_name: self.stable_name(),
            contract_name: self.contract_name(),
            raw_schema: self.raw_schema(),
            compiled_schema: self.compiled_schema(),
            flow: self.flow(),
            trace_kind: self.stable_name(),
            raw_field_paths: self.raw_field_paths(),
            compiled_field_paths: self.compiled_field_paths(),
            asset_ref_field_paths: self.asset_ref_field_paths(),
            capabilities: self.capabilities(),
        }
    }

    pub const fn capabilities(self) -> EventCapabilities {
        match self {
            Self::Dialogue => runtime_real_cap(false, true, false, false, true),
            Self::Choice => runtime_real_cap(false, true, true, false, true),
            Self::Scene => runtime_real_cap(true, true, false, true, true),
            Self::Jump => runtime_real_cap(false, false, false, false, true),
            Self::SetFlag => runtime_real_cap(false, false, false, false, true),
            Self::SetVar => runtime_real_cap(false, false, false, false, true),
            Self::JumpIf => runtime_real_cap(false, false, false, false, true),
            Self::Patch => runtime_real_cap(true, true, false, true, true),
            Self::ExtCall => host_required_cap(),
            Self::AudioAction => runtime_real_cap(true, false, false, true, true),
            Self::Transition => runtime_real_cap(true, true, false, false, false),
            Self::SetCharacterPosition => runtime_real_cap(true, true, false, false, false),
        }
    }

    const fn raw_schema(self) -> &'static str {
        match self {
            Self::Dialogue => "EventRaw::Dialogue(DialogueRaw)",
            Self::Choice => "EventRaw::Choice(ChoiceRaw)",
            Self::Scene => "EventRaw::Scene(SceneUpdateRaw)",
            Self::Jump => "EventRaw::Jump { target }",
            Self::SetFlag => "EventRaw::SetFlag { key, value }",
            Self::SetVar => "EventRaw::SetVar { key, value }",
            Self::JumpIf => "EventRaw::JumpIf { cond, target }",
            Self::Patch => "EventRaw::Patch(ScenePatchRaw)",
            Self::ExtCall => "EventRaw::ExtCall { command, args }",
            Self::AudioAction => "EventRaw::AudioAction(AudioActionRaw)",
            Self::Transition => "EventRaw::Transition(SceneTransitionRaw)",
            Self::SetCharacterPosition => "EventRaw::SetCharacterPosition(SetCharacterPositionRaw)",
        }
    }

    const fn compiled_schema(self) -> &'static str {
        match self {
            Self::Dialogue => "EventCompiled::Dialogue(DialogueCompiled)",
            Self::Choice => "EventCompiled::Choice(ChoiceCompiled)",
            Self::Scene => "EventCompiled::Scene(SceneUpdateCompiled)",
            Self::Jump => "EventCompiled::Jump { target_ip }",
            Self::SetFlag => "EventCompiled::SetFlag { flag_id, value }",
            Self::SetVar => "EventCompiled::SetVar { var_id, value }",
            Self::JumpIf => "EventCompiled::JumpIf { cond, target_ip }",
            Self::Patch => "EventCompiled::Patch(ScenePatchCompiled)",
            Self::ExtCall => "EventCompiled::ExtCall { command, args }",
            Self::AudioAction => "EventCompiled::AudioAction(AudioActionCompiled)",
            Self::Transition => "EventCompiled::Transition(SceneTransitionCompiled)",
            Self::SetCharacterPosition => {
                "EventCompiled::SetCharacterPosition(SetCharacterPositionCompiled)"
            }
        }
    }

    const fn flow(self) -> EventFlow {
        match self {
            Self::Jump => EventFlow::SingleTarget,
            Self::JumpIf => EventFlow::ConditionalTarget,
            Self::Choice => EventFlow::ChoiceTargets,
            _ => EventFlow::Linear,
        }
    }

    const fn raw_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_RAW_FIELDS,
            Self::Jump => JUMP_RAW_FIELDS,
            Self::SetFlag | Self::SetVar => STATE_RAW_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::Patch => PATCH_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::AudioAction => AUDIO_RAW_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::SetCharacterPosition => CHARACTER_POSITION_RAW_FIELDS,
        }
    }

    const fn compiled_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Dialogue => DIALOGUE_RAW_FIELDS,
            Self::Choice => CHOICE_RAW_FIELDS,
            Self::Scene => SCENE_RAW_FIELDS,
            Self::Jump => COMPILED_TARGET_FIELDS,
            Self::SetFlag | Self::SetVar => COMPILED_STATE_FIELDS,
            Self::JumpIf => JUMP_IF_RAW_FIELDS,
            Self::Patch => PATCH_RAW_FIELDS,
            Self::ExtCall => EXT_CALL_RAW_FIELDS,
            Self::AudioAction => COMPILED_AUDIO_FIELDS,
            Self::Transition => TRANSITION_RAW_FIELDS,
            Self::SetCharacterPosition => CHARACTER_POSITION_RAW_FIELDS,
        }
    }

    pub(super) const fn asset_ref_field_paths(self) -> &'static [&'static str] {
        match self {
            Self::Scene => SCENE_ASSET_FIELDS,
            Self::Patch => PATCH_ASSET_FIELDS,
            Self::AudioAction => AUDIO_ASSET_FIELDS,
            _ => EMPTY_FIELDS,
        }
    }
}

pub fn event_specs() -> &'static [EventSpec] {
    &EVENT_SPECS
}

pub fn event_spec(kind: EventKind) -> &'static EventSpec {
    &EVENT_SPECS[event_kind_index(kind)]
}

pub fn event_behavior(kind: EventKind) -> StaticEventBehavior {
    StaticEventBehavior::new(kind)
}

pub fn event_behavior_for_raw(event: &EventRaw) -> StaticEventBehavior {
    event_behavior(event_kind_for_raw(event))
}

pub fn event_behavior_for_compiled(event: &EventCompiled) -> StaticEventBehavior {
    event_behavior(event_kind_for_compiled(event))
}

pub fn event_spec_for_raw(event: &EventRaw) -> &'static EventSpec {
    event_spec(event_kind_for_raw(event))
}

pub fn event_spec_for_compiled(event: &EventCompiled) -> &'static EventSpec {
    event_spec(event_kind_for_compiled(event))
}

pub fn event_asset_refs_for_raw(event: &EventRaw) -> Vec<String> {
    let mut refs = AssetRefCollector::default();
    match event {
        EventRaw::Scene(scene) => {
            refs.push_optional(&scene.background);
            refs.push_optional(&scene.music);
            collect_character_asset_refs(&scene.characters, &mut refs);
        }
        EventRaw::Patch(patch) => collect_scene_patch_asset_refs(patch, &mut refs),
        EventRaw::AudioAction(action) => refs.push_optional(&action.asset),
        _ => {}
    }
    refs.into_vec()
}

pub fn event_kind_for_raw(event: &EventRaw) -> EventKind {
    match event {
        EventRaw::Dialogue(_) => EventKind::Dialogue,
        EventRaw::Choice(_) => EventKind::Choice,
        EventRaw::Scene(_) => EventKind::Scene,
        EventRaw::Jump { .. } => EventKind::Jump,
        EventRaw::SetFlag { .. } => EventKind::SetFlag,
        EventRaw::SetVar { .. } => EventKind::SetVar,
        EventRaw::JumpIf { .. } => EventKind::JumpIf,
        EventRaw::Patch(_) => EventKind::Patch,
        EventRaw::ExtCall { .. } => EventKind::ExtCall,
        EventRaw::AudioAction(_) => EventKind::AudioAction,
        EventRaw::Transition(_) => EventKind::Transition,
        EventRaw::SetCharacterPosition(_) => EventKind::SetCharacterPosition,
    }
}

pub fn event_kind_for_compiled(event: &EventCompiled) -> EventKind {
    match event {
        EventCompiled::Dialogue(_) => EventKind::Dialogue,
        EventCompiled::Choice(_) => EventKind::Choice,
        EventCompiled::Scene(_) => EventKind::Scene,
        EventCompiled::Jump { .. } => EventKind::Jump,
        EventCompiled::SetFlag { .. } => EventKind::SetFlag,
        EventCompiled::SetVar { .. } => EventKind::SetVar,
        EventCompiled::JumpIf { .. } => EventKind::JumpIf,
        EventCompiled::Patch(_) => EventKind::Patch,
        EventCompiled::ExtCall { .. } => EventKind::ExtCall,
        EventCompiled::AudioAction(_) => EventKind::AudioAction,
        EventCompiled::Transition(_) => EventKind::Transition,
        EventCompiled::SetCharacterPosition(_) => EventKind::SetCharacterPosition,
    }
}

const fn event_kind_index(kind: EventKind) -> usize {
    match kind {
        EventKind::Dialogue => 0,
        EventKind::Choice => 1,
        EventKind::Scene => 2,
        EventKind::Jump => 3,
        EventKind::SetFlag => 4,
        EventKind::SetVar => 5,
        EventKind::JumpIf => 6,
        EventKind::Patch => 7,
        EventKind::ExtCall => 8,
        EventKind::AudioAction => 9,
        EventKind::Transition => 10,
        EventKind::SetCharacterPosition => 11,
    }
}
