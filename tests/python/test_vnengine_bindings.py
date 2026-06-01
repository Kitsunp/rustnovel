import json
import shutil
import sys
import types
import unittest
import ast
import inspect
from contextlib import contextmanager
from pathlib import Path

import pytest

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

    def _script_json(self, events, labels=None):
        payload = {
            "script_schema_version": SCRIPT_SCHEMA_VERSION,
            "events": events,
            "labels": labels or {"start": 0},
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
        with self.assertRaises(RuntimeError):
            engine.step()
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

    def test_asset_cache_control_stats_and_eviction(self):
        engine = self.native.Engine(
            self._script_json(
                [
                    {
                        "type": "scene",
                        "background": "bg/room.png",
                        "music": "audio/theme.ogg",
                    },
                    {"type": "dialogue", "speaker": "Ava", "text": "Hola"},
                ]
            )
        )
        for name in (
            "cache_asset",
            "get_cached_asset",
            "invalidate_cached_asset",
            "clear_asset_cache",
            "evict_unused_assets",
            "asset_cache_stats",
        ):
            self.assertTrue(hasattr(engine, name), f"missing cache API {name}")

        engine.set_prefetch_depth(1)
        self.assertEqual(
            set(engine.prefetch_assets_hint()), {"bg/room.png", "audio/theme.ogg"}
        )

        stats = engine.asset_cache_stats()
        self.assertEqual(stats["total_cached_assets"], 0)
        self.assertEqual(stats["allocated_bytes"], 0)
        self.assertEqual(stats["cache_hits"], 0)
        self.assertEqual(stats["cache_misses"], 0)
        self.assertEqual(stats["hit_rate"], 0.0)

        engine.cache_asset("bg/room.png", b"abc")
        engine.cache_asset("stale/old.png", b"zz")
        stats = engine.asset_cache_stats()
        self.assertEqual(stats["total_cached_assets"], 2)
        self.assertEqual(stats["allocated_bytes"], 5)

        self.assertEqual(engine.get_cached_asset("bg/room.png"), b"abc")
        self.assertIsNone(engine.get_cached_asset("missing.png"))
        stats = engine.asset_cache_stats()
        self.assertEqual(stats["cache_hits"], 1)
        self.assertEqual(stats["cache_misses"], 1)
        self.assertEqual(stats["hit_rate"], 0.5)

        evicted = engine.evict_unused_assets()
        self.assertEqual(evicted, 1)
        self.assertEqual(engine.asset_cache_stats()["total_cached_assets"], 1)
        self.assertEqual(engine.get_cached_asset("bg/room.png"), b"abc")
        self.assertIsNone(engine.get_cached_asset("stale/old.png"))

        self.assertTrue(engine.invalidate_cached_asset("bg/room.png"))
        self.assertFalse(engine.invalidate_cached_asset("bg/room.png"))
        self.assertEqual(engine.asset_cache_stats()["total_cached_assets"], 0)

        engine.cache_asset("audio/theme.ogg", b"theme")
        usage = engine.get_memory_usage()
        self.assertEqual(usage["asset_cache_entries"], 1)
        self.assertEqual(usage["asset_cache_allocated_bytes"], 5)
        engine.clear_asset_cache()
        self.assertEqual(engine.asset_cache_stats()["total_cached_assets"], 0)

    def test_asset_cache_budget_eviction_stabilizes_memory(self):
        engine = self.native.Engine(self._dialogue_script_json())
        engine.set_resources(self.native.ResourceConfig(max_texture_memory=10))
        engine.cache_asset("asset/a.bin", b"123456")
        engine.cache_asset("asset/b.bin", b"abcdef")
        stats = engine.asset_cache_stats()
        self.assertLessEqual(stats["allocated_bytes"], 10)
        self.assertEqual(stats["total_cached_assets"], 1)

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

    def test_authoring_command_bus_errors_raise_python_exception(self):
        if not hasattr(self.native, "NodeGraph"):
            self.fail("Native module without NodeGraph API")
        graph = self.native.NodeGraph()

        with self.assertRaises(RuntimeError) as ctx:
            graph.edit_dialogue(999, "Ava", "Missing node")

        message = str(ctx.exception)
        self.assertIn("edit_dialogue failed", message)
        self.assertIn("999", message)

    def test_typed_route_scene_theme_and_layout_api_objects(self):
        for name in (
            "RouteTree",
            "SceneFrame",
            "UiThemeValidationReport",
            "LayoutResolution",
            "validate_ui_theme",
            "preview_theme_color",
            "preview_typography_token",
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

        color_preview = json.loads(self.native.preview_theme_color("#3366CC80"))
        self.assertTrue(color_preview["valid"])
        self.assertEqual(color_preview["rgba"], {"r": 51, "g": 102, "b": 204, "a": 128})

        typography_preview = json.loads(
            self.native.preview_typography_token(
                json.dumps(
                    {
                        "font_family": "serif",
                        "size": 20.0,
                        "weight": 700,
                        "line_height": 1.4,
                    }
                ),
                "Custom preview",
            )
        )
        self.assertEqual(typography_preview["sample"], "Custom preview")
        self.assertEqual(typography_preview["line_height_px"], 28.0)

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
        all_stub_classes = {
            node.name: node
            for node in module_ast.body
            if isinstance(node, ast.ClassDef)
        }
        stub_classes = {
            name: node
            for name, node in all_stub_classes.items()
            if not name.startswith("_")
        }
        stub_functions = {
            node.name
            for node in module_ast.body
            if isinstance(node, ast.FunctionDef) and not node.name.startswith("_")
        }
        stub_public = set(stub_classes) | stub_functions
        native_public = {
            name
            for name, value in inspect.getmembers(self.native)
            if not name.startswith("_")
            and name != "visual_novel_engine"
            and (
                inspect.isclass(value)
                or inspect.isbuiltin(value)
                or inspect.isfunction(value)
            )
        }
        missing_from_native = sorted(stub_public - native_public)
        missing_from_stub = sorted(native_public - stub_public)
        self.assertEqual(missing_from_native, [])
        self.assertEqual(missing_from_stub, [])

        def base_name(base):
            if isinstance(base, ast.Name):
                return base.id
            if isinstance(base, ast.Attribute):
                return base.attr
            return None

        def stub_methods_for(class_name, seen=None):
            seen = set() if seen is None else seen
            if class_name in seen or class_name not in all_stub_classes:
                return set()
            seen.add(class_name)
            class_def = all_stub_classes[class_name]
            methods = {
                item.name
                for item in class_def.body
                if isinstance(item, ast.FunctionDef) and not item.name.startswith("_")
            }
            for base in class_def.bases:
                parent = base_name(base)
                if parent:
                    methods.update(stub_methods_for(parent, seen))
            return methods

        def native_methods_for(native_cls):
            return {
                name
                for name in dir(native_cls)
                if not name.startswith("_")
                and callable(getattr(native_cls, name, None))
            }

        for class_name in sorted(stub_classes):
            if not hasattr(self.native, class_name):
                continue
            native_cls = getattr(self.native, class_name)
            if not inspect.isclass(native_cls) or issubclass(native_cls, BaseException):
                continue
            stub_methods = stub_methods_for(class_name)
            native_methods = native_methods_for(native_cls)
            self.assertEqual(
                sorted(stub_methods - native_methods),
                [],
                f"{class_name} methods declared in .pyi but missing from native",
            )
            self.assertEqual(
                sorted(native_methods - stub_methods),
                [],
                f"{class_name} native public methods missing from .pyi",
            )


def _script_json(events, labels=None):
    payload = {
        "script_schema_version": SCRIPT_SCHEMA_VERSION,
        "events": events,
        "labels": labels or {"start": 0},
    }
    return json.dumps(payload, separators=(",", ":"), sort_keys=True)


def test_pytest_conftest_is_loaded(vnengine_pytest_conftest_loaded):
    assert vnengine_pytest_conftest_loaded


def test_vnerror_variants_raise_typed_python_exceptions():
    import visual_novel_engine as native

    for name in (
        "VnError",
        "VnValidationError",
        "VnSecurityPolicyError",
        "VnResourceLimitError",
        "VnEndOfScriptError",
    ):
        assert hasattr(native, name), f"missing typed exception {name}"

    invalid_choice_script = _script_json(
        [{"type": "choice", "prompt": "Ir?", "options": []}]
    )
    with pytest.raises(native.VnValidationError, match="choice must have options"):
        native.Engine(invalid_choice_script)

    empty_speaker_script = _script_json(
        [{"type": "dialogue", "speaker": "", "text": "Hola"}]
    )
    with pytest.raises(native.VnSecurityPolicyError, match="speaker cannot be empty"):
        native.Engine(empty_speaker_script)

    oversized_asset_script = _script_json(
        [{"type": "scene", "background": "bg/" + ("x" * 200)}]
    )
    with pytest.raises(native.VnResourceLimitError, match="background asset"):
        native.Engine(oversized_asset_script)

    engine = native.Engine(
        _script_json([{"type": "dialogue", "speaker": "Ava", "text": "Fin"}])
    )
    engine.step()
    with pytest.raises(native.VnEndOfScriptError, match="script exhausted"):
        engine.current_event()


def test_typed_exceptions_are_usable_as_common_base_class():
    import visual_novel_engine as native

    malformed = json.dumps(
        {
            "script_schema_version": SCRIPT_SCHEMA_VERSION,
            "events": [{"type": "jump", "target": "missing"}],
            "labels": {"start": 0},
        }
    )
    with pytest.raises(native.VnError) as exc_info:
        native.validate_runtime_config(malformed)

    assert isinstance(exc_info.value, native.VnValidationError)
    assert "jump target 'missing' not found" in str(exc_info.value)
