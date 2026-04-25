"""Script container and normalization helpers for VN Python events."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from typing import Any, Dict, Iterable, List, Mapping, Optional, Tuple, Union

from .types import (
    SCRIPT_SCHEMA_VERSION,
    AudioAction,
    CharacterPatch,
    CharacterPlacement,
    Choice,
    ChoiceOption,
    Cond,
    CondFlag,
    CondVarCmp,
    Dialogue,
    Event,
    ExtCall,
    Jump,
    JumpIf,
    Patch,
    Scene,
    SetCharacterPosition,
    SetFlag,
    SetVar,
    Transition,
    _require_int,
)


@dataclass(frozen=True)
class Script:
    """Script container with stable JSON serialization."""

    events: List[Event] = field(default_factory=list)
    labels: Dict[str, int] = field(default_factory=dict)
    script_schema_version: str = SCRIPT_SCHEMA_VERSION

    def to_dict(self) -> Dict[str, Any]:
        ordered_labels = {key: self.labels[key] for key in sorted(self.labels)}
        return {
            "script_schema_version": self.script_schema_version,
            "events": [event.to_dict() for event in self.events],
            "labels": ordered_labels,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), separators=(",", ":"), sort_keys=True)

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "Script":
        found_version = data.get("script_schema_version")
        if found_version is None:
            found_version = SCRIPT_SCHEMA_VERSION
        if not _is_compatible_schema_version(str(found_version), SCRIPT_SCHEMA_VERSION):
            raise ValueError(
                "schema incompatible: found "
                f"{found_version}, expected {SCRIPT_SCHEMA_VERSION}"
            )
        events = [event_from_dict(item) for item in data.get("events", [])]
        labels = {
            str(key): _require_int(value, f"Script label '{key}'")
            for key, value in data.get("labels", {}).items()
        }
        return cls(
            events=events, labels=labels, script_schema_version=str(found_version)
        )

    @classmethod
    def from_json(cls, raw: str) -> "Script":
        return cls.from_dict(json.loads(raw))


def event_from_dict(data: Mapping[str, Any]) -> Event:
    event_type = data.get("type")
    decoder = _EVENT_DECODERS.get(str(event_type))
    if decoder is not None:
        return decoder.from_dict(data)
    raise ValueError(f"Unknown event type: {event_type}")


def cond_from_dict(data: Mapping[str, Any]) -> Cond:
    kind = data.get("kind")
    if kind == "flag":
        return CondFlag.from_dict(data)
    if kind == "var_cmp":
        return CondVarCmp.from_dict(data)
    raise ValueError(f"Unknown condition kind: {kind}")


def normalize_choice_options(
    options: Iterable[Union[ChoiceOption, Tuple[str, str]]],
) -> List[ChoiceOption]:
    normalized: List[ChoiceOption] = []
    for option in options:
        if isinstance(option, ChoiceOption):
            normalized.append(option)
        else:
            text, target = option
            normalized.append(ChoiceOption(text=text, target=target))
    return normalized


def normalize_characters(
    characters: Iterable[
        Union[CharacterPlacement, Tuple[str, Optional[str], Optional[str]]]
    ],
) -> List[CharacterPlacement]:
    normalized: List[CharacterPlacement] = []
    for character in characters:
        if isinstance(character, CharacterPlacement):
            normalized.append(character)
        else:
            name, expression, position = character
            normalized.append(
                CharacterPlacement(name=name, expression=expression, position=position)
            )
    return normalized


def normalize_character_patches(
    characters: Iterable[
        Union[CharacterPatch, Tuple[str, Optional[str], Optional[str]]]
    ],
) -> List[CharacterPatch]:
    normalized: List[CharacterPatch] = []
    for character in characters:
        if isinstance(character, CharacterPatch):
            normalized.append(character)
        else:
            name, expression, position = character
            normalized.append(
                CharacterPatch(name=name, expression=expression, position=position)
            )
    return normalized


def _is_compatible_schema_version(found: str, expected: str) -> bool:
    if found == expected:
        return True
    if "." not in found or "." not in expected:
        return False
    try:
        found_major = int(found.split(".", 1)[0])
        expected_major = int(expected.split(".", 1)[0])
    except ValueError:
        return False
    return found_major <= expected_major


_EVENT_DECODERS = {
    "dialogue": Dialogue,
    "choice": Choice,
    "scene": Scene,
    "jump": Jump,
    "set_flag": SetFlag,
    "set_var": SetVar,
    "jump_if": JumpIf,
    "patch": Patch,
    "ext_call": ExtCall,
    "audio_action": AudioAction,
    "transition": Transition,
    "set_character_position": SetCharacterPosition,
}
