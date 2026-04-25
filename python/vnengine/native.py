"""Helpers for loading and calling the native Python extension."""

from __future__ import annotations

from typing import Any


def load_native_engine() -> Any:
    """Return the native engine class, with a traceable error if unavailable."""

    try:
        import visual_novel_engine as native
    except ImportError as exc:  # pragma: no cover - environment dependent
        raise RuntimeError(
            "Could not import the native module 'visual_novel_engine'. "
            "Build or install the Python extension first, then import vnengine "
            "from any working directory."
        ) from exc

    engine_cls = getattr(native, "Engine", None)
    if engine_cls is not None:
        return engine_cls

    py_engine = getattr(native, "PyEngine", None)
    if py_engine is not None:
        return py_engine

    raise RuntimeError(
        f"Native module 'visual_novel_engine' was imported from "
        f"{getattr(native, '__file__', 'unknown location')!r}, but it does not "
        f"expose Engine or PyEngine. Available public names: "
        f"{_summarize_public_names(native)}"
    )


def call_native_method(
    native_obj: Any, method_name: str, capability_label: str, *args: Any, **kwargs: Any
) -> Any:
    """Call a native method or raise a traceable runtime error."""

    method = getattr(native_obj, method_name, None)
    if method is None:
        raise RuntimeError(
            f"Native engine object of type {type(native_obj).__name__} does not "
            f"provide {capability_label} (missing '{method_name}'; available: "
            f"{_summarize_public_names(native_obj)})"
        )
    return method(*args, **kwargs)


def _summarize_public_names(value: Any, limit: int = 8) -> str:
    names = [name for name in dir(value) if not name.startswith("_")]
    if not names:
        return "none"
    preview = ", ".join(names[:limit])
    if len(names) > limit:
        return f"{preview}, ..."
    return preview
