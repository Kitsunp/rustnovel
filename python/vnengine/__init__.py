"""Stable Python interface for the Visual Novel Engine."""

from .app import EngineApp
from .builder import ScriptBuilder
from .engine import Engine
from .localization import LocalizationCatalog, collect_script_localization_keys
from .types import (
    AudioAction,
    CharacterPatch,
    CharacterPlacement,
    Choice,
    ChoiceOption,
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
    Script,
    SCRIPT_SCHEMA_VERSION,
    SetFlag,
    SetVar,
    Transition,
)

__all__ = [
    "CharacterPatch",
    "CharacterPlacement",
    "AudioAction",
    "Choice",
    "ChoiceOption",
    "CondFlag",
    "CondVarCmp",
    "Dialogue",
    "Engine",
    "EngineApp",
    "Event",
    "ExtCall",
    "Jump",
    "JumpIf",
    "Patch",
    "Scene",
    "SetCharacterPosition",
    "Script",
    "SCRIPT_SCHEMA_VERSION",
    "ScriptBuilder",
    "LocalizationCatalog",
    "collect_script_localization_keys",
    "SetFlag",
    "SetVar",
    "Transition",
]
