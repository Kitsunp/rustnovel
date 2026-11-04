"""Stable Python interface for the Visual Novel Engine."""

from .app import EngineApp, run_script_headless
from .builder import ScriptBuilder
from .engine import Engine
from .localization import LocalizationCatalog, collect_script_localization_keys
from .types import (
    SCRIPT_SCHEMA_VERSION,
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
    Script,
    SetCharacterPosition,
    SetFlag,
    SetVar,
    Transition,
)

__all__ = [
    "SCRIPT_SCHEMA_VERSION",
    "AudioAction",
    "CharacterPatch",
    "CharacterPlacement",
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
    "LocalizationCatalog",
    "Patch",
    "Scene",
    "Script",
    "ScriptBuilder",
    "SetCharacterPosition",
    "SetFlag",
    "SetVar",
    "Transition",
    "collect_script_localization_keys",
    "run_script_headless",
]
