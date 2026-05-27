from __future__ import annotations

import importlib
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
EXPECTED_PUBLIC_NAMES = {
    "AuthoringValidationReport",
    "Engine",
    "NodeGraph",
    "PyEngine",
    "ResourceConfig",
    "ScriptBuilder",
    "StoryNode",
    "export_bundle",
    "py_validate_graph",
}


def python_native_module_origin_healthcheck() -> dict[str, Any]:
    module = importlib.import_module("visual_novel_engine")
    module_file_raw = getattr(module, "__file__", None)
    module_file = Path(module_file_raw).resolve() if module_file_raw else None
    sys_prefix = Path(sys.prefix).resolve()
    sys_base_prefix = Path(getattr(sys, "base_prefix", sys.prefix)).resolve()
    in_virtualenv = sys_prefix != sys_base_prefix
    public_names = {name for name in dir(module) if not name.startswith("_")}
    missing_names = sorted(EXPECTED_PUBLIC_NAMES - public_names)

    in_repo = module_file is not None and _is_relative_to(module_file, REPO_ROOT)
    in_active_prefix = module_file is not None and _is_relative_to(
        module_file, sys_prefix
    )
    imported_from_allowed_origin = in_repo or (in_virtualenv and in_active_prefix)

    ok = bool(module_file) and imported_from_allowed_origin and not missing_names
    return {
        "ok": ok,
        "sys_prefix": str(sys_prefix),
        "sys_base_prefix": str(sys_base_prefix),
        "in_virtualenv": in_virtualenv,
        "module_file": str(module_file) if module_file else None,
        "in_repo": in_repo,
        "in_active_prefix": in_active_prefix,
        "missing_public_names": missing_names,
        "public_names": sorted(public_names),
        "message": _failure_message(
            module_file=module_file,
            sys_prefix=sys_prefix,
            sys_base_prefix=sys_base_prefix,
            in_virtualenv=in_virtualenv,
            in_repo=in_repo,
            in_active_prefix=in_active_prefix,
            missing_names=missing_names,
        ),
    }


def _failure_message(
    *,
    module_file: Path | None,
    sys_prefix: Path,
    sys_base_prefix: Path,
    in_virtualenv: bool,
    in_repo: bool,
    in_active_prefix: bool,
    missing_names: list[str],
) -> str:
    if (
        module_file
        and (in_repo or (in_virtualenv and in_active_prefix))
        and not missing_names
    ):
        return "visual_novel_engine import origin and public API are valid"

    venv_root = Path("target") / "py-audit-venv"
    venv_bin = "Scripts" if sys.platform == "win32" else "bin"
    venv_python = (
        venv_root / venv_bin / ("python.exe" if sys.platform == "win32" else "python")
    )
    venv_maturin = (
        venv_root / venv_bin / ("maturin.exe" if sys.platform == "win32" else "maturin")
    )

    return "\n".join(
        [
            "visual_novel_engine Python tests must run against the local native build.",
            f"sys.prefix={sys_prefix}",
            f"sys.base_prefix={sys_base_prefix}",
            f"module_file={module_file}",
            f"in_virtualenv={in_virtualenv}",
            f"in_repo={in_repo}",
            f"in_active_prefix={in_active_prefix}",
            f"missing_public_names={missing_names}",
            "Expected either a repo-local module or a module installed into the active venv.",
            "Use:",
            f"  {Path(sys.executable)} -m venv {venv_root}",
            f"  {venv_python} -m pip install --upgrade pip",
            f"  {venv_python} -m pip install maturin pytest",
            f"  {venv_maturin} develop --manifest-path crates/py/Cargo.toml --features extension-module",
            f"  {venv_python} -m pytest -q",
        ]
    )


def _is_relative_to(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True
