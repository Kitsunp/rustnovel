"""Engine wrapper with stable signatures."""

from __future__ import annotations

import json
from typing import Any, Dict, Mapping, Optional, Union

from .native import call_native_method, load_native_engine
from .types import SUPPORTED_EVENT_TYPES, Script


class Engine:
    """Python wrapper around the native VN engine.

    Args:
        script_json: Stable JSON representation of the script.
    """

    def __init__(self, script_json: str) -> None:
        self._engine = load_native_engine()(script_json)
        self._last_audio: Any = []

    @classmethod
    def from_script(cls, script: Union[Script, Mapping[str, Any], str]) -> "Engine":
        """Create an engine from a Script object, dict, or JSON string."""

        if isinstance(script, Script):
            return cls(script.to_json())
        if isinstance(script, str):
            return cls(script)
        return cls(json.dumps(script, separators=(",", ":"), sort_keys=True))

    def current_event(self) -> Dict[str, Any]:
        """Return the current event as a Python dict."""

        return call_native_method(self._engine, "current_event", "current event access")

    def step(self) -> Dict[str, Any]:
        """Advance the engine and return the event that was processed."""

        result = call_native_method(self._engine, "step", "step execution")
        if hasattr(result, "event"):
            self._last_audio = getattr(result, "audio", [])
            return result.event
        self._last_audio = []
        return result

    def choose(self, option_index: int) -> Dict[str, Any]:
        """Apply a choice selection and return the choice event."""

        event = call_native_method(
            self._engine, "choose", "choice handling", option_index
        )
        audio_method = getattr(self._engine, "get_last_audio_commands", None)
        self._last_audio = audio_method() if audio_method is not None else []
        return event

    def register_handler(self, callback: Any) -> None:
        """Register a native ext-call callback, if exposed by the binding."""

        call_native_method(
            self._engine, "register_handler", "callback bindings", callback
        )

    def allow_ext_call_command(self, command: str) -> None:
        """Allow a single ext-call command for callback dispatch."""

        call_native_method(
            self._engine,
            "allow_ext_call_command",
            "ext-call capability bindings",
            command,
        )

    def clear_ext_call_capabilities(self) -> None:
        """Clear the ext-call capability allowlist."""

        call_native_method(
            self._engine,
            "clear_ext_call_capabilities",
            "ext-call capability bindings",
        )

    def last_ext_call_error(self) -> Optional[str]:
        """Return the last ext-call dispatch error, if any."""

        return call_native_method(self._engine, "last_ext_call_error", "error tracking")

    def current_event_json(self) -> str:
        """Return the current event in stable JSON form."""

        return call_native_method(
            self._engine, "current_event_json", "current event JSON access"
        )

    def visual_state(self) -> Dict[str, Any]:
        """Return the current visual state as a Python dict."""

        return call_native_method(self._engine, "visual_state", "visual state access")

    def ui_state(self) -> Dict[str, Any]:
        """Return the current UI state as a Python dict."""

        return call_native_method(self._engine, "ui_state", "ui_state access")

    def is_current_dialogue_read(self) -> bool:
        """Return whether the current dialogue event was already shown in this session."""

        return bool(
            call_native_method(
                self._engine,
                "is_current_dialogue_read",
                "read-tracking bindings",
            )
        )

    def choice_history(self) -> Any:
        """Return recorded choice decisions for the current engine session."""

        return call_native_method(
            self._engine, "choice_history", "choice-history bindings"
        )

    def supported_event_types(self) -> Any:
        """Return event types supported by the native runtime binding."""

        method = getattr(self._engine, "supported_event_types", None)
        if method is not None:
            return method()
        # Conservative fallback for very old native modules.
        return list(SUPPORTED_EVENT_TYPES)

    def set_prefetch_depth(self, depth: int) -> None:
        """Configure lookahead depth used by native prefetch hints."""

        call_native_method(self._engine, "set_prefetch_depth", "prefetch API", depth)

    def prefetch_assets_hint(self) -> Any:
        """Return upcoming asset paths suggested for prefetching."""

        method = getattr(self._engine, "prefetch_assets_hint", None)
        if method is not None:
            return method()
        return []

    def last_audio_commands(self) -> Any:
        """Return the audio commands emitted by the last `step()` or `choose()` call."""

        return list(self._last_audio)

    @property
    def raw(self) -> Any:
        """Return the underlying native engine instance."""

        return self._engine


_load_native_engine = load_native_engine
