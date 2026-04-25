"""Convenience runtime helpers for the VN engine."""

from __future__ import annotations

from typing import Callable, Dict, List, Optional

from .engine import Engine


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
            except ValueError as exc:
                if "script exhausted" not in str(exc):
                    raise
                break
            events.append(event)
            if event.get("type") == "choice":
                index = chooser(event) if chooser else 0
                self.engine.choose(index)
            else:
                self.engine.step()
        return events
