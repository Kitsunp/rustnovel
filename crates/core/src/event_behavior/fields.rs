pub(super) const EMPTY_FIELDS: &[&str] = &[];
pub(super) const DIALOGUE_RAW_FIELDS: &[&str] = &["speaker", "text"];
pub(super) const CHOICE_RAW_FIELDS: &[&str] = &["prompt", "options[].text", "options[].target"];
pub(super) const SCENE_RAW_FIELDS: &[&str] = &[
    "background",
    "music",
    "characters[].name",
    "characters[].expression",
    "characters[].position",
    "characters[].x",
    "characters[].y",
    "characters[].scale",
];
pub(super) const JUMP_RAW_FIELDS: &[&str] = &["target"];
pub(super) const STATE_RAW_FIELDS: &[&str] = &["key", "value"];
pub(super) const JUMP_IF_RAW_FIELDS: &[&str] = &["cond", "target"];
pub(super) const PATCH_RAW_FIELDS: &[&str] = &[
    "background",
    "music",
    "add[].name",
    "add[].expression",
    "add[].position",
    "update[].name",
    "update[].expression",
    "update[].position",
    "remove[]",
];
pub(super) const EXT_CALL_RAW_FIELDS: &[&str] = &["command", "args[]"];
pub(super) const AUDIO_RAW_FIELDS: &[&str] = &[
    "channel",
    "action",
    "asset",
    "volume",
    "fade_duration_ms",
    "loop_playback",
];
pub(super) const TRANSITION_RAW_FIELDS: &[&str] = &["kind", "duration_ms", "color"];
pub(super) const CHARACTER_POSITION_RAW_FIELDS: &[&str] = &["name", "x", "y", "scale"];
pub(super) const SCENE_ASSET_FIELDS: &[&str] = &["background", "music", "characters[].expression"];
pub(super) const PATCH_ASSET_FIELDS: &[&str] = &[
    "background",
    "music",
    "add[].expression",
    "update[].expression",
];
pub(super) const AUDIO_ASSET_FIELDS: &[&str] = &["asset"];
pub(super) const COMPILED_TARGET_FIELDS: &[&str] = &["target_ip"];
pub(super) const COMPILED_STATE_FIELDS: &[&str] = &["flag_id", "var_id", "value"];
pub(super) const COMPILED_AUDIO_FIELDS: &[&str] = &[
    "channel",
    "action",
    "asset",
    "volume",
    "fade_duration_ms",
    "loop_playback",
];

pub(super) const START_FIELDS: &[&str] = &[];
pub(super) const END_FIELDS: &[&str] = &[];
pub(super) const SCENE_NODE_FIELDS: &[&str] = &[
    "profile",
    "background",
    "music",
    "characters[].name",
    "characters[].expression",
    "characters[].position",
    "characters[].x",
    "characters[].y",
    "characters[].scale",
];
pub(super) const SUBGRAPH_FIELDS: &[&str] = &["fragment_id", "entry_port", "exit_port"];
pub(super) const GENERIC_FIELDS: &[&str] = &["event"];
