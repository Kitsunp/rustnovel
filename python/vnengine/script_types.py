"""Script container and normalization helpers for VN Python events."""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from typing import Any, Callable, Dict, Iterable, List, Mapping, Optional, Tuple, Union

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

SchemaPolicy = str
STRICT_CURRENT: SchemaPolicy = "strict_current"
LEGACY_READ_ONLY: SchemaPolicy = "legacy_read_only"
MIGRATING: SchemaPolicy = "migrating"


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
    def from_dict(
        cls, data: Mapping[str, Any], schema_policy: SchemaPolicy = STRICT_CURRENT
    ) -> "Script":
        found_version = _validate_schema_version(
            data.get("script_schema_version"), schema_policy
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
    def from_json(
        cls, raw: str, schema_policy: SchemaPolicy = STRICT_CURRENT
    ) -> "Script":
        return cls.from_dict(json.loads(raw), schema_policy=schema_policy)

    @classmethod
    def from_legacy_dict(cls, data: Mapping[str, Any]) -> "Script":
        return cls.from_dict(data, schema_policy=LEGACY_READ_ONLY)

    @classmethod
    def from_legacy_json(cls, raw: str) -> "Script":
        return cls.from_json(raw, schema_policy=LEGACY_READ_ONLY)


def event_from_dict(data: Mapping[str, Any]) -> Event:
    event_type = data.get("type")
    decoder = _EVENT_DECODERS.get(str(event_type))
    if decoder is not None:
        return decoder(data)
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


def _validate_schema_version(raw: Any, policy: SchemaPolicy) -> str:
    if raw is None:
        if policy in {LEGACY_READ_ONLY, MIGRATING}:
            return SCRIPT_SCHEMA_VERSION
        raise ValueError(f"missing script_schema_version under {policy}")
    if not isinstance(raw, str):
        raise ValueError("script_schema_version must be a string")
    if raw == SCRIPT_SCHEMA_VERSION:
        return raw
    if policy in {LEGACY_READ_ONLY, MIGRATING} and _is_legacy_schema_version(raw):
        return raw
    raise ValueError(
        "schema incompatible: found "
        f"{raw}, expected {SCRIPT_SCHEMA_VERSION} under {policy}"
    )


def _is_legacy_schema_version(found: str) -> bool:
    if "." not in found or "." not in SCRIPT_SCHEMA_VERSION:
        return False
    try:
        found_major = int(found.split(".", 1)[0])
        expected_major = int(SCRIPT_SCHEMA_VERSION.split(".", 1)[0])
    except ValueError:
        return False
    return found_major <= expected_major


_EVENT_DECODERS: Dict[str, Callable[[Mapping[str, Any]], Event]] = {
    "dialogue": Dialogue.from_dict,
    "choice": Choice.from_dict,
    "scene": Scene.from_dict,
    "jump": Jump.from_dict,
    "set_flag": SetFlag.from_dict,
    "set_var": SetVar.from_dict,
    "jump_if": JumpIf.from_dict,
    "patch": Patch.from_dict,
    "ext_call": ExtCall.from_dict,
    "audio_action": AudioAction.from_dict,
    "transition": Transition.from_dict,
    "set_character_position": SetCharacterPosition.from_dict,
}
