"""Typed, stable serialization helpers for VN scripts."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Tuple, Union

SCRIPT_SCHEMA_VERSION = "1.0"
SUPPORTED_EVENT_TYPES: Tuple[str, ...] = (
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
)


def _require_bool(value: Any, field_name: str) -> bool:
    if not isinstance(value, bool):
        raise ValueError(f"{field_name} must be bool, got {type(value).__name__}")
    return value


def _require_int(value: Any, field_name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ValueError(f"{field_name} must be int, got {type(value).__name__}")
    return value


def _require_float(value: Any, field_name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{field_name} must be float, got {type(value).__name__}")
    return float(value)


@dataclass(frozen=True)
class Dialogue:
    """Dialogue event.

    Args:
        speaker: Character name.
        text: Dialogue text.
    """

    speaker: str
    text: str

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "dialogue", "speaker": self.speaker, "text": self.text}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Dialogue":
        return cls(speaker=str(data["speaker"]), text=str(data["text"]))


@dataclass(frozen=True)
class ChoiceOption:
    """Choice option entry."""

    text: str
    target: str

    def to_dict(self) -> Dict[str, Any]:
        return {"text": self.text, "target": self.target}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "ChoiceOption":
        return cls(text=str(data["text"]), target=str(data["target"]))


@dataclass(frozen=True)
class Choice:
    """Choice event containing a prompt and options."""

    prompt: str
    options: List[ChoiceOption] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "choice",
            "prompt": self.prompt,
            "options": [option.to_dict() for option in self.options],
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Choice":
        options = [ChoiceOption.from_dict(item) for item in data.get("options", [])]
        return cls(prompt=str(data["prompt"]), options=options)


@dataclass(frozen=True)
class CharacterPlacement:
    """Character placement in a scene update."""

    name: str
    expression: Optional[str] = None
    position: Optional[str] = None
    x: Optional[int] = None
    y: Optional[int] = None
    scale: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "expression": self.expression,
            "position": self.position,
            "x": self.x,
            "y": self.y,
            "scale": self.scale,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "CharacterPlacement":
        expression = data.get("expression")
        position = data.get("position")
        return cls(
            name=str(data["name"]),
            expression=str(expression) if expression is not None else None,
            position=str(position) if position is not None else None,
            x=_require_int(data["x"], "CharacterPlacement 'x'")
            if data.get("x") is not None
            else None,
            y=_require_int(data["y"], "CharacterPlacement 'y'")
            if data.get("y") is not None
            else None,
            scale=_require_float(data["scale"], "CharacterPlacement 'scale'")
            if data.get("scale") is not None
            else None,
        )


@dataclass(frozen=True)
class Scene:
    """Scene update event."""

    background: Optional[str] = None
    music: Optional[str] = None
    characters: List[CharacterPlacement] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "scene",
            "background": self.background,
            "music": self.music,
            "characters": [character.to_dict() for character in self.characters],
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Scene":
        characters = [
            CharacterPlacement.from_dict(item) for item in data.get("characters", [])
        ]
        return cls(
            background=data.get("background"),
            music=data.get("music"),
            characters=characters,
        )


@dataclass(frozen=True)
class CharacterPatch:
    """Character patch entry."""

    name: str
    expression: Optional[str] = None
    position: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "expression": self.expression,
            "position": self.position,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "CharacterPatch":
        expression = data.get("expression")
        position = data.get("position")
        return cls(
            name=str(data["name"]),
            expression=str(expression) if expression is not None else None,
            position=str(position) if position is not None else None,
        )


@dataclass(frozen=True)
class Patch:
    """Scene patch event with add/update/remove operations."""

    background: Optional[str] = None
    music: Optional[str] = None
    add: List[CharacterPlacement] = field(default_factory=list)
    update: List[CharacterPatch] = field(default_factory=list)
    remove: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "patch",
            "background": self.background,
            "music": self.music,
            "add": [character.to_dict() for character in self.add],
            "update": [character.to_dict() for character in self.update],
            "remove": list(self.remove),
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Patch":
        add = [CharacterPlacement.from_dict(item) for item in data.get("add", [])]
        update = [CharacterPatch.from_dict(item) for item in data.get("update", [])]
        remove = [str(item) for item in data.get("remove", [])]
        return cls(
            background=data.get("background"),
            music=data.get("music"),
            add=add,
            update=update,
            remove=remove,
        )


@dataclass(frozen=True)
class AudioAction:
    """Audio action event."""

    channel: str
    action: str
    asset: Optional[str] = None
    volume: Optional[float] = None
    fade_duration_ms: Optional[int] = None
    loop_playback: Optional[bool] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "audio_action",
            "channel": self.channel,
            "action": self.action,
            "asset": self.asset,
            "volume": self.volume,
            "fade_duration_ms": self.fade_duration_ms,
            "loop_playback": self.loop_playback,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "AudioAction":
        return cls(
            channel=str(data["channel"]),
            action=str(data["action"]),
            asset=str(data["asset"]) if data.get("asset") is not None else None,
            volume=_require_float(data["volume"], "AudioAction 'volume'")
            if data.get("volume") is not None
            else None,
            fade_duration_ms=(
                _require_int(data["fade_duration_ms"], "AudioAction 'fade_duration_ms'")
                if data.get("fade_duration_ms") is not None
                else None
            ),
            loop_playback=(
                _require_bool(data["loop_playback"], "AudioAction 'loop_playback'")
                if data.get("loop_playback") is not None
                else None
            ),
        )


@dataclass(frozen=True)
class Transition:
    """Scene transition event."""

    kind: str
    duration_ms: int
    color: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "transition",
            "kind": self.kind,
            "duration_ms": self.duration_ms,
            "color": self.color,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Transition":
        return cls(
            kind=str(data["kind"]),
            duration_ms=_require_int(data["duration_ms"], "Transition 'duration_ms'"),
            color=str(data["color"]) if data.get("color") is not None else None,
        )


@dataclass(frozen=True)
class SetCharacterPosition:
    """Absolute character position event."""

    name: str
    x: int
    y: int
    scale: Optional[float] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "type": "set_character_position",
            "name": self.name,
            "x": self.x,
            "y": self.y,
            "scale": self.scale,
        }

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "SetCharacterPosition":
        return cls(
            name=str(data["name"]),
            x=_require_int(data["x"], "SetCharacterPosition 'x'"),
            y=_require_int(data["y"], "SetCharacterPosition 'y'"),
            scale=_require_float(data["scale"], "SetCharacterPosition 'scale'")
            if data.get("scale") is not None
            else None,
        )


@dataclass(frozen=True)
class ExtCall:
    """External call event."""

    command: str
    args: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "ext_call", "command": self.command, "args": list(self.args)}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "ExtCall":
        return cls(
            command=str(data["command"]),
            args=[str(item) for item in data.get("args", [])],
        )


@dataclass(frozen=True)
class Jump:
    """Jump event."""

    target: str

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "jump", "target": self.target}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Jump":
        return cls(target=str(data["target"]))


@dataclass(frozen=True)
class SetFlag:
    """Set-flag event."""

    key: str
    value: bool

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "set_flag", "key": self.key, "value": self.value}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "SetFlag":
        value = data["value"]
        if not isinstance(value, bool):
            raise ValueError(
                f"SetFlag 'value' must be bool, got {type(value).__name__}"
            )
        return cls(key=str(data["key"]), value=value)


@dataclass(frozen=True)
class SetVar:
    """Set-var event."""

    key: str
    value: int

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "set_var", "key": self.key, "value": self.value}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "SetVar":
        value = data["value"]
        return cls(key=str(data["key"]), value=_require_int(value, "SetVar 'value'"))


@dataclass(frozen=True)
class CondFlag:
    key: str
    is_set: bool

    def to_dict(self) -> Dict[str, Any]:
        return {"kind": "flag", "key": self.key, "is_set": self.is_set}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "CondFlag":
        value = data["is_set"]
        if not isinstance(value, bool):
            raise ValueError(
                f"CondFlag 'is_set' must be bool, got {type(value).__name__}"
            )
        return cls(key=str(data["key"]), is_set=value)


@dataclass(frozen=True)
class CondVarCmp:
    key: str
    op: str
    value: int

    def to_dict(self) -> Dict[str, Any]:
        return {"kind": "var_cmp", "key": self.key, "op": self.op, "value": self.value}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "CondVarCmp":
        value = data["value"]
        return cls(
            key=str(data["key"]),
            op=str(data["op"]),
            value=_require_int(value, "CondVarCmp 'value'"),
        )


Cond = Union[CondFlag, CondVarCmp]


@dataclass(frozen=True)
class JumpIf:
    """Conditional jump event."""

    cond: Cond
    target: str

    def to_dict(self) -> Dict[str, Any]:
        return {"type": "jump_if", "cond": self.cond.to_dict(), "target": self.target}

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "JumpIf":
        cond = cond_from_dict(data.get("cond", {}))
        return cls(cond=cond, target=str(data["target"]))


Event = Union[
    Dialogue,
    Choice,
    Scene,
    Jump,
    SetFlag,
    SetVar,
    JumpIf,
    Patch,
    AudioAction,
    Transition,
    SetCharacterPosition,
    ExtCall,
]


from .script_types import (  # noqa: E402  (loaded after event classes exist)
    Script,
    cond_from_dict,
    event_from_dict,
    normalize_character_patches,
    normalize_characters,
    normalize_choice_options,
)
