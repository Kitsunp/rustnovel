import json
import sys
import types
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from vnengine.app import EngineApp, run_script_headless
from vnengine.builder import ScriptBuilder
from vnengine.localization import LocalizationCatalog, collect_script_localization_keys
from vnengine.types import (
    AudioAction,
    CharacterPlacement,
    Choice,
    Dialogue,
    JumpIf,
    LEGACY_READ_ONLY,
    MIGRATING,
    Script,
    SCRIPT_SCHEMA_VERSION,
    SetCharacterPosition,
    SetFlag,
    SetVar,
    STRICT_CURRENT,
    SUPPORTED_EVENT_TYPES,
    Transition,
    event_from_dict,
)

SCHEMA_FIXTURE_ROOT = Path(__file__).resolve().parents[1] / "fixtures" / "schema_policy"


def _accepts(callback):
    try:
        callback()
    except Exception:
        return False
    return True


class TypesTests(unittest.TestCase):
    def test_supported_event_types_are_shared_with_builder(self):
        self.assertEqual(ScriptBuilder().supported_event_types(), SUPPORTED_EVENT_TYPES)

    def test_script_serialization_is_stable(self):
        script = Script(
            events=[Dialogue(speaker="Ava", text="Hola")],
            labels={"b": 1, "a": 0},
        )
        expected = (
            f'{{"events":[{{"speaker":"Ava","text":"Hola","type":"dialogue"}}],'
            f'"labels":{{"a":0,"b":1}},"script_schema_version":"{SCRIPT_SCHEMA_VERSION}"}}'
        )
        self.assertEqual(script.to_json(), expected)

    def test_script_rejects_missing_schema_version_by_default(self):
        with self.assertRaises(ValueError):
            Script.from_json('{"events": [], "labels": {"start": 0}}')

    def test_script_accepts_missing_schema_version_only_with_legacy_policy(self):
        parsed = Script.from_json(
            '{"events": [], "labels": {"start": 0}}',
            schema_policy=LEGACY_READ_ONLY,
        )
        self.assertEqual(parsed.labels["start"], 0)

    def test_script_rejects_legacy_major_schema_version_by_default(self):
        with self.assertRaises(ValueError):
            Script.from_json(
                '{"script_schema_version":"0.9","events":[],"labels":{"start":0}}'
            )

    def test_script_accepts_legacy_major_schema_version_only_with_policy(self):
        parsed = Script.from_json(
            '{"script_schema_version":"0.9","events":[],"labels":{"start":0}}',
            schema_policy=LEGACY_READ_ONLY,
        )
        self.assertEqual(parsed.labels["start"], 0)

    def test_python_and_rust_reject_same_schema_fixtures_by_default(self):
        import visual_novel_engine as native

        cases = {
            "valid_current.json": True,
            "corrupt.json": False,
            "legacy_runtime_no_schema.json": False,
            "authoring_corrupt.vnauthoring": False,
            "old_schema.json": False,
            "missing_schema.json": False,
        }
        for filename, accepted in cases.items():
            with self.subTest(filename=filename):
                raw = (SCHEMA_FIXTURE_ROOT / filename).read_text(encoding="utf-8")
                self.assertEqual(_accepts(lambda: Script.from_json(raw)), accepted)
                self.assertEqual(_accepts(lambda: native.Engine(raw)), accepted)

    def test_python_uses_native_schema_policy_for_explicit_modes(self):
        import visual_novel_engine as native

        cases = [
            ("valid_current.json", STRICT_CURRENT, True),
            ("old_schema.json", STRICT_CURRENT, False),
            ("missing_schema.json", STRICT_CURRENT, False),
            ("legacy_runtime_no_schema.json", LEGACY_READ_ONLY, True),
            ("old_schema.json", LEGACY_READ_ONLY, True),
            ("missing_schema.json", MIGRATING, True),
        ]
        for filename, policy, accepted in cases:
            with self.subTest(filename=filename, policy=policy):
                raw = (SCHEMA_FIXTURE_ROOT / filename).read_text(encoding="utf-8")
                payload = json.loads(raw)
                version = payload.get("script_schema_version")
                self.assertEqual(
                    _accepts(
                        lambda: native.validate_script_schema_version(version, policy)
                    ),
                    accepted,
                )
                self.assertEqual(
                    _accepts(lambda: Script.from_json(raw, schema_policy=policy)),
                    accepted,
                )

    def test_script_schema_validation_warns_on_incomplete_native_module(self):
        original_module = sys.modules.get("visual_novel_engine")
        module = types.ModuleType("visual_novel_engine")
        module.__file__ = "/tmp/visual_novel_engine.py"
        module.Engine = object
        sys.modules["visual_novel_engine"] = module
        try:
            with self.assertWarnsRegex(
                RuntimeWarning,
                "does not expose validate_script_schema_version",
            ) as ctx:
                parsed = Script.from_json(
                    f'{{"script_schema_version":"{SCRIPT_SCHEMA_VERSION}",'
                    '"events":[],"labels":{"start":0}}'
                )
        finally:
            if original_module is None:
                sys.modules.pop("visual_novel_engine", None)
            else:
                sys.modules["visual_novel_engine"] = original_module

        self.assertEqual(parsed.labels["start"], 0)
        message = str(ctx.warning)
        self.assertIn("does not expose validate_script_schema_version", message)
        self.assertIn("/tmp/visual_novel_engine.py", message)
        self.assertIn("Available public names", message)

    def test_event_from_dict_rejects_unknown_type(self):
        with self.assertRaises(ValueError):
            event_from_dict({"type": "unknown"})

    def test_choice_from_dict_requires_options_instead_of_empty_default(self):
        cases = [
            ({"type": "choice", "prompt": "Route?"}, "Choice missing required 'options' field"),
            (
                {"type": "choice", "prompt": "Route?", "options": {"text": "A"}},
                "Choice 'options' must be list",
            ),
            (
                {"type": "choice", "prompt": "Route?", "options": ["A"]},
                "ChoiceOption payload must be object",
            ),
        ]
        for payload, message in cases:
            with self.subTest(payload=payload):
                with self.assertRaisesRegex(ValueError, message):
                    Choice.from_dict(payload)

    def test_character_from_dict_coerces_optional_fields(self):
        placement = CharacterPlacement.from_dict(
            {"name": "Ava", "expression": 1, "position": True}
        )
        self.assertEqual(placement.expression, "1")
        self.assertEqual(placement.position, "True")

    def test_set_flag_from_dict_requires_bool(self):
        with self.assertRaises(ValueError):
            SetFlag.from_dict({"key": "flag", "value": "false"})

    def test_set_var_from_dict_requires_int(self):
        with self.assertRaises(ValueError):
            SetVar.from_dict({"key": "counter", "value": "5"})

    def test_jump_if_requires_cond(self):
        with self.assertRaises(ValueError):
            JumpIf.from_dict(
                {"type": "jump_if", "cond": {"kind": "unknown"}, "target": "end"}
            )

    def test_audio_transition_and_position_roundtrip(self):
        events = [
            AudioAction(channel="bgm", action="play", asset="music.ogg"),
            Transition(kind="fade", duration_ms=300),
            SetCharacterPosition(name="Ava", x=10, y=20, scale=1.0),
        ]
        script = Script(events=events, labels={"start": 0})
        parsed = Script.from_json(script.to_json())

        self.assertEqual(len(parsed.events), 3)
        self.assertEqual(parsed.events[0].to_dict()["type"], "audio_action")
        self.assertEqual(parsed.events[1].to_dict()["type"], "transition")
        self.assertEqual(parsed.events[2].to_dict()["type"], "set_character_position")

    def test_audio_action_loop_playback_requires_bool(self):
        with self.assertRaises(ValueError):
            AudioAction.from_dict(
                {
                    "channel": "bgm",
                    "action": "play",
                    "asset": None,
                    "volume": None,
                    "fade_duration_ms": None,
                    "loop_playback": "false",
                }
            )

    def test_script_labels_require_int_indices(self):
        with self.assertRaises(ValueError):
            Script.from_json(
                '{"script_schema_version":"1.0","events":[],"labels":{"start":true}}'
            )

    def test_script_rejects_missing_payload_fields_instead_of_empty_defaults(self):
        cases = [
            (
                '{"script_schema_version":"1.0","eventz":[],"labels":{"start":0}}',
                "Script missing required 'events' field",
            ),
            (
                '{"script_schema_version":"1.0","events":[]}',
                "Script missing required 'labels' field",
            ),
        ]
        for raw, message in cases:
            with self.subTest(raw=raw):
                with self.assertRaisesRegex(ValueError, message):
                    Script.from_json(raw)

    def test_script_rejects_malformed_container_fields(self):
        cases = [
            (
                '{"script_schema_version":"1.0","events":{},"labels":{"start":0}}',
                "Script 'events' must be list",
            ),
            (
                '{"script_schema_version":"1.0","events":[],"labels":[]}',
                "Script 'labels' must be object",
            ),
            (
                '{"script_schema_version":"1.0","events":["dialogue"],"labels":{"start":0}}',
                "Script event at index 0 must be object",
            ),
        ]
        for raw, message in cases:
            with self.subTest(raw=raw):
                with self.assertRaisesRegex(ValueError, message):
                    Script.from_json(raw)

    def test_set_var_rejects_bool_payload(self):
        with self.assertRaises(ValueError):
            SetVar.from_dict({"key": "counter", "value": True})

    def test_character_and_transition_numeric_fields_reject_bool(self):
        with self.assertRaises(ValueError):
            CharacterPlacement.from_dict({"name": "Ava", "x": True})
        with self.assertRaises(ValueError):
            Transition.from_dict({"kind": "fade", "duration_ms": False})
        with self.assertRaises(ValueError):
            SetCharacterPosition.from_dict(
                {"name": "Ava", "x": 1, "y": 2, "scale": True}
            )
        with self.assertRaises(ValueError):
            AudioAction.from_dict(
                {
                    "channel": "bgm",
                    "action": "play",
                    "asset": None,
                    "volume": True,
                    "fade_duration_ms": None,
                    "loop_playback": None,
                }
            )


class LocalizationTests(unittest.TestCase):
    def test_collect_and_validate_localization_keys(self):
        script = Script(
            events=[
                Dialogue(speaker="loc:speaker.narrator", text="loc:dialogue.intro"),
                AudioAction(channel="bgm", action="play", asset="theme.ogg"),
            ],
            labels={"start": 0},
        )
        keys = collect_script_localization_keys(script)
        self.assertEqual(keys, {"speaker.narrator", "dialogue.intro"})

        catalog = LocalizationCatalog(
            default_locale="en",
            locales={
                "en": {"speaker.narrator": "Narrator"},
                "es": {"speaker.narrator": "Narrador", "unused": "x"},
            },
        )
        missing, orphan = catalog.validate_keys(keys)
        self.assertIn("en:dialogue.intro", missing)
        self.assertIn("es:dialogue.intro", missing)
        self.assertIn("es:unused", orphan)

    def test_empty_localization_catalog_reports_default_locale_missing_keys(self):
        missing, orphan = LocalizationCatalog(default_locale="en").validate_keys(
            {"dialogue.intro"}
        )

        self.assertEqual(missing, ["en:dialogue.intro"])
        self.assertEqual(orphan, [])

    def test_resolve_or_key_preserves_intentional_empty_translation(self):
        catalog = LocalizationCatalog(
            default_locale="en",
            locales={"en": {"ui.hidden_label": ""}},
        )

        self.assertEqual(catalog.resolve_or_key("en", "ui.hidden_label"), "")
        self.assertEqual(catalog.resolve_or_key("en", "missing"), "missing")


class BuilderTests(unittest.TestCase):
    def test_builder_json_is_stable_across_threads(self):
        builder = ScriptBuilder()
        builder.label("start")
        builder.dialogue("Ava", "Hola")
        builder.choice("Go?", [("Yes", "end"), ("No", "start")])
        builder.label("end")
        builder.set_flag("done", True)
        builder.set_var("counter", 3)
        builder.jump_if_var("counter", "gt", 1, target="end")
        builder.patch(
            add=[("Ava", "happy", "left")], update=[("Ava", None, "center")], remove=[]
        )
        builder.audio_action("bgm", "play", asset="music/theme.ogg", loop_playback=True)
        builder.transition("fade", 250)
        builder.set_character_position("Ava", 32, 48, 1.1)
        builder.ext_call("open_minigame", ["cards"])

        with ThreadPoolExecutor(max_workers=4) as executor:
            results = list(executor.map(lambda _: builder.to_json(), range(8)))

        self.assertTrue(all(result == results[0] for result in results))
        payload = json.loads(results[0])
        self.assertEqual(payload["labels"], {"end": 2, "start": 0})
        self.assertEqual(payload["script_schema_version"], SCRIPT_SCHEMA_VERSION)
        patch_events = [
            event for event in payload["events"] if event["type"] == "patch"
        ]
        self.assertEqual(len(patch_events), 1)
        self.assertEqual(patch_events[0]["add"][0]["name"], "Ava")
        self.assertTrue(
            any(event["type"] == "audio_action" for event in payload["events"])
        )
        self.assertTrue(
            any(event["type"] == "transition" for event in payload["events"])
        )
        self.assertTrue(
            any(
                event["type"] == "set_character_position" for event in payload["events"]
            )
        )
        self.assertTrue(any(event["type"] == "ext_call" for event in payload["events"]))

    def test_builder_ext_call_rejects_non_string_args(self):
        builder = ScriptBuilder()
        with self.assertRaises(ValueError):
            builder.ext_call("open_minigame", ["cards", 7])


class EngineAppTests(unittest.TestCase):
    def test_engine_app_runs_choices(self):
        events = [
            {"type": "choice", "prompt": "Go?", "options": []},
            {"type": "dialogue", "speaker": "Ava", "text": "Done"},
        ]

        class FakeEngine:
            def __init__(self):
                self.index = 0

            def current_event(self):
                if self.index >= len(events):
                    raise ValueError("script exhausted")
                return events[self.index]

            def choose(self, option_index):
                self.index += 1
                return events[self.index - 1]

            def step(self):
                self.index += 1
                return events[self.index - 1]

        app = EngineApp(FakeEngine())
        collected = app.run(lambda _event: 0)
        self.assertEqual(len(collected), 2)
        self.assertEqual(collected[0]["type"], "choice")
        self.assertEqual(collected[1]["type"], "dialogue")

    def test_engine_app_propagates_unexpected_errors(self):
        class BrokenEngine:
            def current_event(self):
                raise RuntimeError("boom")

        app = EngineApp(BrokenEngine())
        with self.assertRaises(RuntimeError):
            app.run()

    def test_engine_app_does_not_swallow_value_errors_that_only_mention_script_exhausted(self):
        class BrokenEngine:
            def current_event(self):
                raise ValueError("cache lookup failed before script exhausted marker")

        app = EngineApp(BrokenEngine())
        with self.assertRaisesRegex(ValueError, "cache lookup failed"):
            app.run()

    def test_run_script_headless_wraps_engine_app(self):
        script = Script(
            events=[Dialogue(speaker="Ava", text="Hola")],
            labels={"start": 0},
        )

        try:
            events = run_script_headless(script)
        except RuntimeError as exc:
            self.fail(f"native engine not available: {exc}")

        self.assertEqual(events[0]["type"], "dialogue")
        self.assertEqual(events[0]["speaker"], "Ava")


if __name__ == "__main__":
    unittest.main()
