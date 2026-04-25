import os
import shutil
import subprocess
import sys
import types
import unittest
from pathlib import Path
from uuid import uuid4

from vnengine.engine import Engine, _load_native_engine
from vnengine.types import SCRIPT_SCHEMA_VERSION, SUPPORTED_EVENT_TYPES


class EngineWrapperTests(unittest.TestCase):
    def setUp(self):
        self._original_module = sys.modules.get("visual_novel_engine")

    def tearDown(self):
        if self._original_module is None:
            sys.modules.pop("visual_novel_engine", None)
        else:
            sys.modules["visual_novel_engine"] = self._original_module

    def test_engine_wrapper_prefers_engine_binding(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine_cls = _load_native_engine()
        self.assertIs(engine_cls, FakeEngine)

    def test_engine_from_script_accepts_mapping(self):
        captured = {}
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                captured["payload"] = script_json

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        self.assertIsInstance(engine.raw, FakeEngine)
        self.assertEqual(
            captured["payload"],
            f'{{"events":[],"labels":{{"start":0}},"script_schema_version":"{SCRIPT_SCHEMA_VERSION}"}}',
        )

    def test_engine_wrapper_extcall_policy_methods_delegate(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json
                self.allowed = []
                self.handler = None
                self.error = None

            def allow_ext_call_command(self, command):
                self.allowed.append(command)

            def clear_ext_call_capabilities(self):
                self.allowed.clear()

            def register_handler(self, callback):
                self.handler = callback

            def last_ext_call_error(self):
                return self.error

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        sentinel = object()
        engine.allow_ext_call_command("minigame_start")
        engine.register_handler(sentinel)
        engine.clear_ext_call_capabilities()
        self.assertEqual(engine.last_ext_call_error(), None)
        self.assertEqual(engine.raw.allowed, [])
        self.assertIs(engine.raw.handler, sentinel)

    def test_engine_step_normalizes_native_step_result_and_tracks_audio(self):
        module = types.ModuleType("visual_novel_engine")

        class StepResult:
            def __init__(self):
                self.event = {"type": "dialogue", "speaker": "Ava", "text": "Hola"}
                self.audio = [{"type": "play_bgm", "path": "theme.ogg"}]

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

            def step(self):
                return StepResult()

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        event = engine.step()
        self.assertEqual(event["type"], "dialogue")
        self.assertEqual(
            engine.last_audio_commands(), [{"type": "play_bgm", "path": "theme.ogg"}]
        )

    def test_engine_choose_tracks_native_audio_commands(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

            def choose(self, option_index):
                return {"type": "choice", "selected": option_index}

            def get_last_audio_commands(self):
                return [{"type": "play_bgm", "path": "branch.ogg"}]

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        event = engine.choose(0)
        self.assertEqual(event, {"type": "choice", "selected": 0})
        self.assertEqual(
            engine.last_audio_commands(), [{"type": "play_bgm", "path": "branch.ogg"}]
        )

    def test_engine_ui_state_calls_native(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

            def ui_state(self):
                return {"type": "choice", "prompt": "Go?", "options": ["Yes", "No"]}

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        self.assertEqual(
            engine.ui_state(),
            {"type": "choice", "prompt": "Go?", "options": ["Yes", "No"]},
        )

    def test_engine_ui_state_requires_binding(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        with self.assertRaises(RuntimeError):
            engine.ui_state()

    def test_engine_read_tracking_wrapper_methods(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

            def is_current_dialogue_read(self):
                return True

            def choice_history(self):
                return [{"option_index": 0}]

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        self.assertTrue(engine.is_current_dialogue_read())
        self.assertEqual(engine.choice_history(), [{"option_index": 0}])

    def test_engine_prefetch_wrapper_methods(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json
                self.depth = 0

            def set_prefetch_depth(self, depth):
                self.depth = depth

            def prefetch_assets_hint(self):
                return ["bg/room.png"] if self.depth > 0 else []

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        engine.set_prefetch_depth(2)
        self.assertEqual(engine.prefetch_assets_hint(), ["bg/room.png"])

    def test_engine_supported_event_types_fallback_uses_python_contract(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine = Engine.from_script(
            {
                "script_schema_version": SCRIPT_SCHEMA_VERSION,
                "events": [],
                "labels": {"start": 0},
            }
        )
        self.assertEqual(engine.supported_event_types(), list(SUPPORTED_EVENT_TYPES))

    def test_engine_can_be_created_from_any_cwd(self):
        repo_python = Path(__file__).resolve().parents[2] / "python"
        temp_root = Path(__file__).resolve().parents[2] / "target"
        module_dir = temp_root / f"vnengine_python_module_{uuid4().hex}"
        cwd_dir = temp_root / f"vnengine_python_cwd_{uuid4().hex}"
        module_dir.mkdir(parents=True, exist_ok=False)
        cwd_dir.mkdir(parents=True, exist_ok=False)

        try:
            module_path = module_dir / "visual_novel_engine.py"
            module_path.write_text(
                """
class Engine:
    def __init__(self, script_json):
        self.script_json = script_json

    def current_event(self):
        return {"type": "dialogue", "speaker": "Ava", "text": "Hola"}
""".strip()
            )

            code = """
from vnengine.engine import Engine

engine = Engine.from_script({
    "script_schema_version": "1.0",
    "events": [{"type": "dialogue", "speaker": "Ava", "text": "Hola"}],
    "labels": {"start": 0},
})
print(engine.current_event()["type"])
print(engine.raw.script_json)
""".strip()

            env = os.environ.copy()
            pythonpath_parts = [str(module_dir), str(repo_python)]
            if env.get("PYTHONPATH"):
                pythonpath_parts.append(env["PYTHONPATH"])
            env["PYTHONPATH"] = os.pathsep.join(pythonpath_parts)

            result = subprocess.run(
                [sys.executable, "-c", code],
                cwd=cwd_dir,
                env=env,
                capture_output=True,
                text=True,
                check=True,
            )
        finally:
            shutil.rmtree(module_dir, ignore_errors=True)
            shutil.rmtree(cwd_dir, ignore_errors=True)

        self.assertEqual(
            result.stdout.splitlines(),
            [
                "dialogue",
                '{"events":[{"speaker":"Ava","text":"Hola","type":"dialogue"}],"labels":{"start":0},"script_schema_version":"1.0"}',
            ],
        )


if __name__ == "__main__":
    unittest.main()
