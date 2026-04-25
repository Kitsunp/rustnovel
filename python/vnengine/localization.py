"""Localization helpers for VN authoring and validation."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Set, Tuple

from .types import Script


def _extract_loc_key(value: str) -> str | None:
    text = value.strip()
    if not text.startswith("loc:"):
        return None
    key = text[4:].strip()
    return key or None


@dataclass(frozen=True)
class LocalizationCatalog:
    default_locale: str = "en"
    locales: Dict[str, Dict[str, str]] = field(default_factory=dict)

    def resolve(self, locale: str, key: str) -> str | None:
        if locale in self.locales and key in self.locales[locale]:
            return self.locales[locale][key]
        default_table = self.locales.get(self.default_locale, {})
        return default_table.get(key)

    def resolve_or_key(self, locale: str, key: str) -> str:
        return self.resolve(locale, key) or key

    def validate_keys(
        self, required_keys: Iterable[str]
    ) -> Tuple[List[str], List[str]]:
        required: Set[str] = {key.strip() for key in required_keys if key.strip()}
        missing: List[str] = []
        orphan: List[str] = []

        for locale, table in sorted(self.locales.items()):
            for key in sorted(required):
                if key not in table:
                    missing.append(f"{locale}:{key}")
            for key in sorted(table.keys()):
                if key not in required:
                    orphan.append(f"{locale}:{key}")
        return missing, orphan


def collect_script_localization_keys(script: Script) -> Set[str]:
    keys: Set[str] = set()
    for event in script.events:
        payload = event.to_dict()
        event_type = payload.get("type")
        if event_type == "dialogue":
            for field_name in ("speaker", "text"):
                value = str(payload.get(field_name, ""))
                key = _extract_loc_key(value)
                if key:
                    keys.add(key)
        elif event_type == "choice":
            prompt_key = _extract_loc_key(str(payload.get("prompt", "")))
            if prompt_key:
                keys.add(prompt_key)
            for option in payload.get("options", []):
                option_key = _extract_loc_key(str(option.get("text", "")))
                if option_key:
                    keys.add(option_key)
    return keys
