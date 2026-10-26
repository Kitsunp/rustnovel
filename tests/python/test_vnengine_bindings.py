import ast
import gc
import inspect
import json
import shutil
import sys
import types
import unittest
import weakref
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
            self.assertEqual(engine.current_event()["type"], "ext_call")
            self.assertEqual(engine.pending_external_call()["command"], command)
            self.assertIsInstance(engine.visual_state(), dict)
            for advance in (engine.step, engine.resume, engine.complete_external_call):
                with self.assertRaisesRegex(RuntimeError, "callback is active"):
                    advance()
            engine.audio().play_sfx("callback-click")

        if hasattr(engine, "allow_ext_call_command"):
            engine.allow_ext_call_command("minigame_start")
        engine.register_handler(handler)
        engine.audio().play_bgm("pre-dispatch")
        result = engine.step()
        event = result.event
        self.assertEqual(event["type"], "ext_call")
        self.assertEqual(calls, [("minigame_start", ["poker"])])
        self.assertEqual(
            [item["type"] for item in result.audio], ["play_bgm", "play_sfx"]
        )
        self.assertEqual(result.audio[1]["path"], "callback-click")
        if hasattr(engine, "last_ext_call_error"):
            self.assertIsNone(engine.last_ext_call_error())

        next_result = engine.step()
        next_event = next_result.event
        self.assertEqual(next_event["type"], "dialogue")
        if hasattr(engine, "last_ext_call_error"):
            self.assertIsNone(engine.last_ext_call_error())

    def test_compiled_events_preserve_conditions_and_patch_coordinates(self):
        from vnengine.types import event_from_dict

        cases = [
            (
                {"type": "set_var", "key": "counter", "value": 17},
                {"type": "set_var", "key": "0", "value": 17},
            ),
            (
                {
                    "type": "jump_if",
                    "cond": {"kind": "flag", "key": "ready", "is_set": True},
                    "target": "end",
                },
                {
                    "type": "jump_if",
                    "cond": {"kind": "flag", "key": "0", "is_set": True},
                    "target": "1",
                },
            ),
            (
                {
                    "type": "jump_if",
                    "cond": {
                        "kind": "var_cmp",
                        "key": "counter",
                        "op": "ge",
                        "value": 4,
                    },
                    "target": "end",
                },
                {
                    "type": "jump_if",
                    "cond": {"kind": "var_cmp", "key": "0", "op": "ge", "value": 4},
                    "target": "1",
                },
            ),
        ]
        for event, expected in cases:
            with self.subTest(event=event):
                engine = self.native.Engine(
                    self._script_json(
                        [event, {"type": "dialogue", "speaker": "A", "text": "done"}],
                        {"start": 0, "end": 1},
                    )
                )
                current = engine.current_event()
                self.assertEqual(event_from_dict(current).to_dict(), expected)
                self.assertEqual(engine.step().event, current)

        patch = {
            "type": "patch",
            "update": [{"name": "Ava", "x": 12, "y": 34, "scale": 1.25}],
        }
        engine = self.native.Engine(self._script_json([patch]))
        current = engine.current_event()
        self.assertEqual(
            event_from_dict(current).update[0].to_dict(),
            {
                "name": "Ava",
                "expression": None,
                "position": None,
                "x": 12,
                "y": 34,
                "scale": 1.25,
            },
        )

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

        def failing_handler(command, args):
            self.assertEqual(engine.current_event()["type"], "ext_call")
            raise ValueError("callback failed")

        engine.allow_ext_call_command("minigame_start")
        engine.register_handler(failing_handler)
        with self.assertRaisesRegex(RuntimeError, "callback failed"):
            engine.step()
        self.assertEqual(engine.pending_external_call()["event_ip"], 0)
        engine.register_handler(handler)
        self.assertEqual(engine.step().event["type"], "ext_call")
        self.assertEqual(engine.current_event()["type"], "dialogue")
        self.assertIsNone(engine.last_ext_call_error())

    def test_handler_reference_cycles_are_collectable_with_engine_or_audio(self):
        class Marker:
            pass

        def make_cycle(via_audio):
            engine = self.native.Engine(self._ext_call_script_json())
            marker = Marker()
            owner = engine.audio() if via_audio else engine

            def handler(command, args, owner=owner, marker=marker):
                return owner, marker

            engine.register_handler(handler)
            return weakref.ref(marker)

        for via_audio in (False, True):
            with self.subTest(via_audio=via_audio):
                references = [make_cycle(via_audio) for _ in range(16)]
                gc.collect()
                self.assertTrue(all(reference() is None for reference in references))

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
        for invalid in (float("inf"), float("nan"), 1e300):
            with self.subTest(fade_in=invalid), self.assertRaises(ValueError):
                audio.play_bgm("invalid", fade_in=invalid)
            with self.subTest(fade_out=invalid), self.assertRaises(ValueError):
                audio.stop_all(fade_out=invalid)
        for method in (audio.play_bgm, audio.play_sfx, audio.play_voice):
            for invalid in (float("inf"), float("-inf"), float("nan")):
                with self.subTest(
                    method=method.__name__, volume=invalid
                ), self.assertRaises(ValueError):
                    method("invalid", volume=invalid)

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

    def test_asset_cache_normalizes_lookup_and_invalidation(self):
        engine = self.native.Engine(self._dialogue_script_json())
        engine.cache_asset("  asset/a.bin  ", b"payload")
        self.assertEqual(engine.get_cached_asset(" asset/a.bin "), b"payload")
        self.assertTrue(engine.invalidate_cached_asset(" asset/a.bin "))
        self.assertEqual(engine.asset_cache_stats()["total_cached_assets"], 0)

    def test_empty_assets_have_bounded_cache_metadata(self):
        engine = self.native.Engine(self._dialogue_script_json())
        for index in range(5000):
            engine.cache_asset(f"asset/{index}.bin", b"")
        stats = engine.asset_cache_stats()
        self.assertLessEqual(stats["total_cached_assets"], 4096)
        self.assertIsNone(engine.get_cached_asset("asset/0.bin"))
        self.assertEqual(engine.get_cached_asset("asset/4999.bin"), b"")

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

    def test_character_coordinates_match_builders_and_story_nodes(self):
        from vnengine.builder import ScriptBuilder
        from vnengine.types import Patch

        characters = [("Ava", "happy", "left"), ("Mira", None, None, 12, 34, 1.25)]
        native = self.native.ScriptBuilder()
        python = ScriptBuilder()
        for builder in (native, python):
            builder.scene(characters=characters)
            builder.patch(add=characters, update=characters)
        native_events = json.loads(native.build_json())["events"]
        python_events = json.loads(python.to_json())["events"]
        for events in (native_events, python_events):
            self.assertNotIn("background", events[1])
            self.assertNotIn("music", events[1])
            events[0]["background"] = "bg/room.png"
            events[0]["music"] = "music/theme.ogg"
            events.append(
                Patch.from_dict(
                    {
                        "type": "patch",
                        "background": None,
                        "music": None,
                    }
                ).to_dict()
            )
            engine = self.native.Engine(
                json.dumps(
                    {
                        "script_schema_version": SCRIPT_SCHEMA_VERSION,
                        "events": events,
                        "labels": {"start": 0},
                    }
                )
            )
            engine.step()
            self.assertNotIn("background", engine.current_event())
            self.assertIsNone(Patch.from_dict(engine.current_event()).background)
            engine.step()
            self.assertEqual(engine.visual_state()["background"], "bg/room.png")
            self.assertEqual(engine.visual_state()["music"], "music/theme.ogg")
            engine.step()
            self.assertIsNone(engine.visual_state()["background"])
            self.assertIsNone(engine.visual_state()["music"])
            events.pop()
            events[0]["background"] = None
            events[0]["music"] = None
        for constructor, kwargs, expected in [
            (self.native.StoryNode.scene, {"characters": characters}, native_events[0]),
            (
                self.native.StoryNode.scene_full,
                {"characters": characters},
                native_events[0],
            ),
            (
                self.native.StoryNode.scene_patch,
                {"add": characters, "update": characters},
                native_events[1],
            ),
            (
                self.native.StoryNode.scene_patch_full,
                {"add": characters, "update": characters},
                native_events[1],
            ),
        ]:
            with self.subTest(constructor=constructor.__name__):
                graph = self.native.NodeGraph()
                start = graph.add_node(self.native.StoryNode.start(), 0.0, 0.0)
                node = graph.add_node(constructor(**kwargs), 0.0, 100.0)
                end = graph.add_node(self.native.StoryNode.end(), 0.0, 200.0)
                graph.connect(start, node)
                graph.connect(node, end)
                event = json.loads(graph.to_script_json())["events"][0]
                self.assertEqual(event, expected)
        for native_event, python_event in zip(native_events, python_events):
            for field in ("characters", "add", "update"):
                if field in native_event:
                    pairs = zip(native_event[field], python_event[field])
                    for native_character, python_character in pairs:
                        for key in (
                            "name",
                            "expression",
                            "position",
                            "x",
                            "y",
                            "scale",
                        ):
                            self.assertEqual(
                                native_character.get(key),
                                python_character.get(key),
                            )
        for builder in (native, python):
            with self.assertRaises((TypeError, ValueError)):
                builder.patch(update=[("Ava", None, None, 12)])

    def test_runtime_media_events_roundtrip_through_python_types(self):
        from vnengine.types import Script, event_from_dict

        cases = []
        for channel_id, channel in enumerate(("bgm", "sfx", "voice")):
            for action_id, action in enumerate(("play", "stop", "fade_out")):
                cases.append(
                    (
                        {
                            "type": "audio_action",
                            "channel": f" {channel.upper()} ",
                            "action": f" {action.upper()} ",
                            "asset": "sound.ogg",
                            "volume": 0.5,
                            "fade_duration_ms": 125,
                            "loop_playback": False,
                        },
                        {"channel": channel, "action": action},
                        {"channel": channel_id, "action": action_id},
                    )
                )
        for kind_id, kind in enumerate(("fade", "dissolve", "cut")):
            cases.append(
                (
                    {
                        "type": "transition",
                        "kind": f" {kind.upper()} ",
                        "duration_ms": 250,
                        "color": "#000000",
                    },
                    {"kind": kind},
                    {"kind": kind_id},
                )
            )
        cases.append(
            (
                {
                    "type": "transition",
                    "kind": "fade_black",
                    "duration_ms": 250,
                },
                {"kind": "fade"},
                {"kind": 0},
            )
        )
        for event, names, codes in cases:
            with self.subTest(event=event):
                engine = self.native.Engine(
                    json.dumps(
                        {
                            "script_schema_version": SCRIPT_SCHEMA_VERSION,
                            "events": [event],
                            "labels": {"start": 0},
                        }
                    )
                )
                compiled = engine.current_event()
                normalized = event_from_dict(compiled)
                for field, expected in names.items():
                    self.assertEqual(getattr(normalized, field), expected)
                    self.assertEqual(compiled[field], codes[field])
                    self.assertEqual(compiled[field + "_name"], expected)
                rebuilt = self.native.Engine(
                    Script(events=[normalized], labels={"start": 0}).to_json()
                )
                self.assertEqual(rebuilt.current_event(), compiled)
                self.assertEqual(engine.step().event, compiled)
                kwargs = {key: value for key, value in event.items() if key != "type"}
                constructor = (
                    self.native.StoryNode.audio_action
                    if event["type"] == "audio_action"
                    else self.native.StoryNode.transition
                )
                graph = self.native.NodeGraph()
                start = graph.add_node(self.native.StoryNode.start(), 0.0, 0.0)
                node = graph.add_node(constructor(**kwargs), 0.0, 100.0)
                end = graph.add_node(self.native.StoryNode.end(), 0.0, 200.0)
                graph.connect(start, node)
                graph.connect(node, end)
                codes = {issue.code for issue in graph.validate_no_io()}
                self.assertTrue(
                    codes.isdisjoint(
                        {
                            "VAL_AUDIO_CHANNEL_INVALID",
                            "VAL_AUDIO_ACTION_INVALID",
                            "VAL_TRANSITION_KIND_INVALID",
                        }
                    ),
                    codes,
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
