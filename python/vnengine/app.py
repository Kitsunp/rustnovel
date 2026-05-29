"""Convenience runtime helpers for the VN engine."""

from __future__ import annotations

from typing import Callable, Dict, List, Mapping, Optional, Union

from .engine import Engine
from .types import Script


def _is_script_exhausted(exc: BaseException) -> bool:
    try:
        import visual_novel_engine as native
    except ImportError:
        native = None

    end_of_script_error = getattr(native, "VnEndOfScriptError", None)
    if end_of_script_error is not None and isinstance(exc, end_of_script_error):
        return True
    return isinstance(exc, ValueError) and "script exhausted" in str(exc)


class EngineApp:
    """Drive an engine until completion.

    Args:
        engine: Engine instance to run.
    """

    def __init__(self, engine: Engine) -> None:
        self.engine = engine

    def run(
        self, chooser: Optional[Callable[[Dict[str, object]], int]] = None
    ) -> List[Dict[str, object]]:
        """Run the engine until the end and collect events.

        Args:
            chooser: Optional callback invoked for choice events. The callback
                receives the choice event dict and returns the selected index.

        Returns:
            List of event dictionaries in the order they were processed.
        """

        events: List[Dict[str, object]] = []
        while True:
            try:
                event = self.engine.current_event()
            except Exception as exc:
                if not _is_script_exhausted(exc):
                    raise
                break
            events.append(event)
            if event.get("type") == "choice":
                index = chooser(event) if chooser else 0
                self.engine.choose(index)
            else:
                self.engine.step()
        return events


def run_script_headless(
    script: Union[Script, Mapping[str, object], str],
    chooser: Optional[Callable[[Dict[str, object]], int]] = None,
) -> List[Dict[str, object]]:
    """Run a script through the native headless runtime and collect events."""

    return EngineApp(Engine.from_script(script)).run(chooser)
