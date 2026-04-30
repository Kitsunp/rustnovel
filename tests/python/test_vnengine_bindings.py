import json
import sys
import types
import unittest

from vnengine.native import call_native_method, load_native_engine
from vnengine.types import SCRIPT_SCHEMA_VERSION, SUPPORTED_EVENT_TYPES


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
            self.skipTest("visual_novel_engine native module not available")

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
            self.skipTest("Native engine without ResourceConfig API")
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
            self.skipTest("Native engine without ext_call support")

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

    def test_audio_controller_and_prefetch_api(self):
        engine = self.native.Engine(self._dialogue_script_json())
        if not hasattr(engine, "set_prefetch_depth"):
            self.skipTest("Native engine without prefetch API")
        if not hasattr(engine, "audio"):
            self.skipTest("Native engine without audio controller API")
        engine.set_prefetch_depth(3)
        if hasattr(engine, "prefetch_assets_hint"):
            self.assertIsInstance(engine.prefetch_assets_hint(), list)
        self.assertIsInstance(engine.is_loading(), bool)

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
            self.skipTest("Native engine without event contract API")

        self.assertEqual(tuple(engine.supported_event_types()), SUPPORTED_EVENT_TYPES)
        if hasattr(self.native, "ScriptBuilder"):
            builder = self.native.ScriptBuilder()
            self.assertEqual(
                tuple(builder.supported_event_types()), SUPPORTED_EVENT_TYPES
            )

    def test_engine_choice_history_and_read_tracking(self):
        if not hasattr(self.native.Engine, "is_current_dialogue_read"):
            self.skipTest("Engine binding without read-tracking API")
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


class GuiBindingTests(unittest.TestCase):
    def test_run_visual_novel_rejects_invalid_json(self):
        import visual_novel_engine as vn

        with self.assertRaises(ValueError):
            vn.run_visual_novel("{invalid", None)

    def test_gui_bindings_exist(self):
        import visual_novel_engine as vn

        config = vn.VnConfig(width=800.0, height=600.0, fullscreen=False)
        self.assertIsNotNone(config)
        self.assertTrue(callable(vn.run_visual_novel))

    def test_node_graph_search_and_bookmarks(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        dialogue = graph.add_node(
            vn.StoryNode.dialogue("Narrador", "Castillo"), 0.0, 100.0
        )
        graph.connect(start, dialogue)

        hits = graph.search_nodes("castillo")
        self.assertIn(dialogue, hits)

        self.assertTrue(graph.set_bookmark("intro", dialogue))
        self.assertEqual(graph.bookmark_target("intro"), dialogue)
        bookmarks = dict(graph.list_bookmarks())
        self.assertEqual(bookmarks["intro"], dialogue)

    def test_node_graph_autofix_bindings(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        required = ["validate", "fix_candidates", "autofix_issue", "autofix_safe"]
        if not all(hasattr(graph, attr) for attr in required):
            self.skipTest("Native GUI build without autofix APIs")

        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        dialogue = graph.add_node(vn.StoryNode.dialogue("", "Hola"), 0.0, 100.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 200.0)
        graph.connect(start, dialogue)
        graph.connect(dialogue, end)

        issues = graph.validate()
        idx = next(
            i for i, issue in enumerate(issues) if issue.code == "VAL_SPEAKER_EMPTY"
        )
        candidates = graph.fix_candidates(idx)
        self.assertGreaterEqual(len(candidates), 1)
        applied_fix = graph.autofix_issue(idx, False)
        self.assertIsNotNone(applied_fix)

        post_issues = graph.validate()
        self.assertTrue(
            all(issue.code != "VAL_SPEAKER_EMPTY" for issue in post_issues),
            "speaker-empty issue should be auto-fixed",
        )

    def test_node_graph_diagnostic_envelope_is_localized(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        graph.add_node(vn.StoryNode.dialogue("", "Hola"), 0.0, 0.0)
        issue = next(
            issue for issue in graph.validate() if issue.code == "VAL_SPEAKER_EMPTY"
        )

        self.assertEqual(issue.message_key, "diagnostic.val.speaker.empty")
        self.assertTrue(issue.docs_ref.startswith("docs/diagnostics/authoring.md#"))
        self.assertTrue(issue.action_steps_es)
        localized = issue.localized("es")
        self.assertEqual(localized["schema"], "vnengine.diagnostic_envelope.v2")
        self.assertEqual(localized["message_key"], issue.message_key)
        self.assertIn("Speaker", localized["message"])
        self.assertTrue(localized["root_cause"])

    def test_node_graph_set_flag_and_authoring_save_contract(self):
        import tempfile
        from pathlib import Path

        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        flag = graph.add_node(vn.StoryNode.set_flag("met_ava", True), 0.0, 100.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 200.0)
        graph.connect(start, flag)
        graph.connect(flag, end)

        script_payload = json.loads(graph.to_script_json())
        self.assertEqual(script_payload["events"][0]["type"], "set_flag")
        self.assertEqual(script_payload["events"][0]["key"], "met_ava")

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "game.vnauthoring"
            graph.save(str(path))
            saved = path.read_text()
            self.assertIn("authoring_schema_version", saved)
            loaded = vn.NodeGraph.load(str(path))
            self.assertEqual(loaded.node_count(), graph.node_count())

    def test_node_graph_jump_if_two_ports_roundtrip_and_engine_compile(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "connect_port"):
            self.skipTest("Native graph binding does not expose port connections")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        jump_if = graph.add_node(
            vn.StoryNode.jump_if_flag("seen_true", True, "true_branch"), 0.0, 100.0
        )
        false_branch = graph.add_node(
            vn.StoryNode.dialogue("Narrator", "False branch"), -100.0, 200.0
        )
        true_branch = graph.add_node(
            vn.StoryNode.dialogue("Narrator", "True branch"), 100.0, 200.0
        )
        end = graph.add_node(vn.StoryNode.end(), 0.0, 300.0)
        graph.connect(start, jump_if)
        graph.connect_port(jump_if, 0, true_branch)
        graph.connect_port(jump_if, 1, false_branch)
        graph.connect(false_branch, end)
        graph.connect(true_branch, end)

        payload = json.loads(graph.to_script_json())
        self.assertEqual(payload["events"][0]["type"], "jump_if")
        target_ip = payload["labels"][payload["events"][0]["target"]]
        self.assertEqual(payload["events"][target_ip]["text"], "True branch")
        self.assertEqual(payload["events"][1]["text"], "False branch")
        self.native.Engine(json.dumps(payload))

        restored = vn.NodeGraph.from_script_json(json.dumps(payload))
        self.assertEqual(
            json.loads(restored.to_script_json())["events"][0]["type"], "jump_if"
        )

    def test_node_graph_validate_rejects_windows_drive_asset_path(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        graph.add_node(vn.StoryNode.scene(r"C:\temp\evil.png", None, []), 0.0, 0.0)
        codes = {issue.code for issue in graph.validate()}
        self.assertIn("VAL_ASSET_UNSAFE_PATH", codes)

    def test_node_graph_validate_accepts_project_root(self):
        import tempfile
        from pathlib import Path

        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "assets" / "bg").mkdir(parents=True)
            (root / "assets" / "bg" / "room.png").write_bytes(b"png")
            graph = vn.NodeGraph()
            graph.add_node(vn.StoryNode.scene("assets/bg/room.png", None, []), 0.0, 0.0)
            self.assertNotIn(
                "VAL_ASSET_NOT_FOUND", {issue.code for issue in graph.validate_no_io()}
            )
            self.assertNotIn(
                "VAL_ASSET_NOT_FOUND",
                {issue.code for issue in graph.validate(project_root=str(root))},
            )

    def test_story_node_full_scene_and_extcall_constructors(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        scene = graph.add_node(
            vn.StoryNode.scene_full(
                None,
                "bg/room.png",
                None,
                [("Ava", "characters/ava.png", "left", 10, 20, 1.25)],
            ),
            0.0,
            100.0,
        )
        ext = graph.add_node(vn.StoryNode.ext_call("analytics", ["intro"]), 0.0, 200.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 300.0)
        graph.connect(start, scene)
        graph.connect(scene, ext)
        graph.connect(ext, end)

        payload = json.loads(graph.to_script_json())
        self.assertEqual(payload["events"][0]["characters"][0]["x"], 10)
        self.assertEqual(payload["events"][0]["characters"][0]["scale"], 1.25)
        self.assertEqual(payload["events"][1]["type"], "ext_call")
