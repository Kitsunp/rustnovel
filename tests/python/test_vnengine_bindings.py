import json
import shutil
import sys
import types
import unittest
import ast
from contextlib import contextmanager
from pathlib import Path

from vnengine.native import call_native_method, load_native_engine
from vnengine.types import SCRIPT_SCHEMA_VERSION, SUPPORTED_EVENT_TYPES


@contextmanager
def workspace_tempdir(name: str):
    root = Path.cwd() / "target" / "python-test-tmp" / name
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    try:
        yield root
    finally:
        shutil.rmtree(root, ignore_errors=True)


class NativeBindingsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            import visual_novel_engine as native
        except ImportError:
            cls.native = None
        else:
            cls.native = native

    def setUp(self):
        self._original_module = sys.modules.get("visual_novel_engine")
        if self.native is None:
            self.fail("visual_novel_engine native module not available")

    def tearDown(self):
        if self._original_module is None:
            sys.modules.pop("visual_novel_engine", None)
        else:
            sys.modules["visual_novel_engine"] = self._original_module

    def _dialogue_script_json(self):
        payload = {
            "script_schema_version": SCRIPT_SCHEMA_VERSION,
            "events": [
                {"type": "dialogue", "speaker": "Ava", "text": "Hola"},
                {"type": "dialogue", "speaker": "Ava", "text": "Continuar"},
            ],
            "labels": {"start": 0},
        }
        return json.dumps(payload, separators=(",", ":"), sort_keys=True)

    def _ext_call_script_json(self):
        payload = {
            "script_schema_version": SCRIPT_SCHEMA_VERSION,
            "events": [
                {"type": "ext_call", "command": "minigame_start", "args": ["poker"]},
                {"type": "dialogue", "speaker": "Ava", "text": "Hola"},
            ],
            "labels": {"start": 0},
        }
        return json.dumps(payload, separators=(",", ":"), sort_keys=True)

    def _supports_ext_call(self):
        probe = self.native.Engine(self._dialogue_script_json())
        if hasattr(probe, "supported_event_types"):
            return "ext_call" in set(probe.supported_event_types())
        return False

    def test_load_native_engine_prefers_engine_binding(self):
        module = types.ModuleType("visual_novel_engine")

        class FakeEngine:
            def __init__(self, script_json):
                self.script_json = script_json

        module.Engine = FakeEngine
        sys.modules["visual_novel_engine"] = module

        engine_cls = load_native_engine()
        self.assertIs(engine_cls, FakeEngine)

    def test_load_native_engine_reports_missing_surface(self):
        module = types.ModuleType("visual_novel_engine")
        module.__file__ = "/tmp/visual_novel_engine.py"
        sys.modules["visual_novel_engine"] = module

        with self.assertRaises(RuntimeError) as ctx:
            load_native_engine()

        message = str(ctx.exception)
        self.assertIn("does not expose Engine or PyEngine", message)
        self.assertIn("Available public names", message)
        self.assertIn("/tmp/visual_novel_engine.py", message)

    def test_call_native_method_reports_missing_capability(self):
        with self.assertRaises(RuntimeError) as ctx:
            call_native_method(object(), "register_handler", "callback bindings")

        self.assertIn("missing 'register_handler'", str(ctx.exception))
        self.assertIn("callback bindings", str(ctx.exception))

    def test_resource_config_and_memory_usage(self):
        if not hasattr(self.native, "ResourceConfig"):
            self.fail("Native engine without ResourceConfig API")
        engine = self.native.Engine(self._dialogue_script_json())
        config = self.native.ResourceConfig(
            max_texture_memory=123, max_script_bytes=456
        )
        engine.set_resources(config)
        usage = engine.get_memory_usage()
        self.assertEqual(usage["max_texture_memory"], 123)
        self.assertEqual(usage["max_script_bytes"], 456)

    def test_ext_call_handler_and_resume(self):
        if not self._supports_ext_call():
            self.fail("Native engine without ext_call support")

        engine = self.native.Engine(self._ext_call_script_json())
        calls = []

        def handler(command, args):
            calls.append((command, args))

        if hasattr(engine, "allow_ext_call_command"):
            engine.allow_ext_call_command("minigame_start")
        engine.register_handler(handler)
        result = engine.step()
        event = result.event
        self.assertEqual(event["type"], "ext_call")
        self.assertEqual(calls, [("minigame_start", ["poker"])])
        if hasattr(engine, "last_ext_call_error"):
            self.assertIsNone(engine.last_ext_call_error())

        engine.resume()
        next_result = engine.step()
        next_event = next_result.event
        self.assertEqual(next_event["type"], "dialogue")
        if hasattr(engine, "last_ext_call_error"):
            self.assertIsNone(engine.last_ext_call_error())

    def test_ext_call_handler_is_denied_without_explicit_capability(self):
        if not self._supports_ext_call():
            self.fail("Native engine without ext_call support")

        engine = self.native.Engine(self._ext_call_script_json())
        calls = []

        def handler(command, args):
            calls.append((command, args))

        engine.register_handler(handler)
        result = engine.step()
        self.assertEqual(result.event["type"], "ext_call")
        self.assertEqual(calls, [])
        if hasattr(engine, "last_ext_call_error"):
            self.assertIn("denied", engine.last_ext_call_error())

    def test_audio_controller_and_prefetch_api(self):
        engine = self.native.Engine(self._dialogue_script_json())
        if not hasattr(engine, "set_prefetch_depth"):
            self.fail("Native engine without prefetch API")
        if not hasattr(engine, "audio"):
            self.fail("Native engine without audio controller API")
        engine.set_prefetch_depth(3)
        if hasattr(engine, "prefetch_assets_hint"):
            self.assertIsInstance(engine.prefetch_assets_hint(), list)
        with self.assertRaises(NotImplementedError):
            engine.is_loading()

        audio = engine.audio()
        audio.play_bgm("theme_song", loop=True, fade_in=0.5)

        step_result = engine.step()
        commands = step_result.audio
        self.assertEqual(len(commands), 1)
        self.assertEqual(commands[0]["type"], "play_bgm")
        self.assertTrue(commands[0]["loop"])
        self.assertEqual(commands[0]["fade_in"], 0.5)

        audio.stop_all(fade_out=0.1)
        audio.play_sfx("click")

    def test_native_event_contract_matches_python_contract(self):
        engine = self.native.Engine(self._dialogue_script_json())
        if not hasattr(engine, "supported_event_types"):
            self.fail("Native engine without event contract API")

        self.assertEqual(tuple(engine.supported_event_types()), SUPPORTED_EVENT_TYPES)
        if hasattr(self.native, "ScriptBuilder"):
            builder = self.native.ScriptBuilder()
            self.assertEqual(
                tuple(builder.supported_event_types()), SUPPORTED_EVENT_TYPES
            )

    def test_engine_choice_history_and_read_tracking(self):
        if not hasattr(self.native.Engine, "is_current_dialogue_read"):
            self.fail("Engine binding without read-tracking API")
        payload = {
            "script_schema_version": SCRIPT_SCHEMA_VERSION,
            "events": [
                {"type": "dialogue", "speaker": "Ava", "text": "Hola"},
                {
                    "type": "choice",
                    "prompt": "Ir?",
                    "options": [{"text": "Volver", "target": "start"}],
                },
            ],
            "labels": {"start": 0},
        }
        engine = self.native.Engine(
            json.dumps(payload, separators=(",", ":"), sort_keys=True)
        )

        self.assertFalse(engine.is_current_dialogue_read())
        engine.step()
        engine.choose(0)
        self.assertTrue(engine.is_current_dialogue_read())

        history = engine.choice_history()
        self.assertEqual(len(history), 1)
        self.assertEqual(history[0]["option_index"], 0)
        self.assertEqual(history[0]["option_text"], "Volver")

    def test_typed_route_scene_theme_and_layout_api_objects(self):
        for name in (
            "RouteTree",
            "SceneFrame",
            "UiThemeValidationReport",
            "LayoutResolution",
            "validate_ui_theme",
            "resolve_layout",
        ):
            self.assertTrue(hasattr(self.native, name), f"missing {name}")

        engine = self.native.Engine(self._dialogue_script_json())
        route_tree = engine.route_tree()
        self.assertTrue(hasattr(route_tree, "to_dict"))
        route = route_tree.to_dict()
        self.assertEqual(route["root"], 0)
        self.assertEqual(route["coverage"]["total_nodes"], 2)

        scene_frame = engine.scene_frame()
        self.assertTrue(hasattr(scene_frame, "to_dict"))
        frame = scene_frame.to_dict()
        self.assertEqual(frame["frame_schema"], "vnengine.scene_frame.v1")
        self.assertIn("commands", frame)
        self.assertIn("route", frame)

        theme = {
            "id": "typed-test",
            "locale": "en",
            "colors": {"stage.background": "#101218"},
            "typography": {},
            "spacing": {},
            "radii": {},
            "alpha": {},
            "action_text": {"continue": "Continue"},
            "components": {"components": {}},
        }
        theme_report = self.native.validate_ui_theme(json.dumps(theme))
        self.assertTrue(hasattr(theme_report, "to_dict"))
        self.assertTrue(theme_report.to_dict()["valid"])

        display = {
            "logical_size": [800.0, 600.0],
            "physical_size": [1600, 1200],
            "dpi": None,
            "ppi": None,
            "tpi": None,
            "scale_factor": 2.0,
            "user_scale": 1.0,
            "safe_area": {"left": 0.0, "right": 0.0, "top": 0.0, "bottom": 0.0},
            "window_mode": "windowed",
            "orientation": "landscape",
        }
        layout = self.native.resolve_layout(json.dumps(display))
        self.assertTrue(hasattr(layout, "to_dict"))
        resolved = layout.to_dict()
        self.assertEqual(resolved["breakpoint"], "normal")
        self.assertGreater(resolved["stage_rect"]["width"], 0)

    def test_pyi_top_level_public_api_matches_native_module(self):
        stub_path = (
            Path(__file__).resolve().parents[2] / "python" / "visual_novel_engine.pyi"
        )
        module_ast = ast.parse(stub_path.read_text(encoding="utf-8"))
        stub_public = {
            node.name
            for node in module_ast.body
            if isinstance(node, (ast.ClassDef, ast.FunctionDef))
            and not node.name.startswith("_")
        }
        native_public = {
            name
            for name in dir(self.native)
            if not name.startswith("_")
            and name not in {"PyEngine", "visual_novel_engine"}
        }
        missing_from_native = sorted(stub_public - native_public - {"PyEngine"})
        missing_from_stub = sorted(native_public - stub_public)
        self.assertEqual(missing_from_native, [])
        self.assertEqual(missing_from_stub, [])

        class_defs = {
            node.name: node
            for node in module_ast.body
            if isinstance(node, ast.ClassDef) and hasattr(self.native, node.name)
        }
        for class_name, class_def in class_defs.items():
            native_cls = getattr(self.native, class_name)
            for item in class_def.body:
                if isinstance(item, ast.FunctionDef) and not item.name.startswith("_"):
                    self.assertTrue(
                        hasattr(native_cls, item.name),
                        f"{class_name}.{item.name} missing from native module",
                    )
