import json
import shutil
import unittest
from contextlib import contextmanager
from pathlib import Path

from vnengine.types import SCRIPT_SCHEMA_VERSION


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

    def test_export_bundle_api_can_materialize_windows_executable(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "export_bundle"):
            self.fail("Native module without export_bundle API")

        with workspace_tempdir("export-bundle-exe") as root:
            project = root / "project"
            (project / "runtime").mkdir(parents=True)
            (project / "project.vnm").write_text(
                "\n".join(
                    [
                        'manifest_schema_version = "1.0"',
                        "",
                        "[metadata]",
                        'name = "Python Export"',
                        'author = "QA"',
                        'version = "0.1.0"',
                        "",
                        "[settings]",
                        "resolution = [1280, 720]",
                        'default_language = "en"',
                        'supported_languages = ["en"]',
                        'entry_point = "main.json"',
                        "",
                        "[assets]",
                    ]
                ),
                encoding="utf-8",
            )
            (project / "main.json").write_text(
                json.dumps(
                    {
                        "script_schema_version": SCRIPT_SCHEMA_VERSION,
                        "events": [
                            {
                                "type": "dialogue",
                                "speaker": "Narrator",
                                "text": "Packaged",
                            }
                        ],
                        "labels": {"start": 0},
                    },
                    separators=(",", ":"),
                    sort_keys=True,
                ),
                encoding="utf-8",
            )
            (project / "runtime" / "vn-runtime.exe").write_bytes(b"fake-exe")

            out = root / "dist"
            report = json.loads(
                vn.export_bundle(
                    str(project),
                    str(out),
                    runtime_artifact="runtime/vn-runtime.exe",
                )
            )

            self.assertEqual(report["runtime_artifact"], "runtime/vn-runtime.exe")
            self.assertEqual(report["executable"], "game.exe")
            self.assertEqual((out / "game.exe").read_bytes(), b"fake-exe")

    def test_node_graph_search_and_bookmarks(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")

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
            self.fail("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        required = ["validate", "fix_candidates", "autofix_issue", "autofix_safe"]
        if not all(hasattr(graph, attr) for attr in required):
            self.fail("Native GUI build without autofix APIs")

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
        if hasattr(graph, "operation_log") and hasattr(graph, "verification_runs"):
            operations = [
                json.loads(entry.to_json()) for entry in graph.operation_log()
            ]
            self.assertTrue(
                any(
                    entry["operation_kind"] == "quick_fix_applied"
                    and entry.get("diagnostic_id")
                    for entry in operations
                ),
                "Python autofix must leave a diagnostic-linked operation log entry",
            )
            self.assertEqual(len(graph.verification_runs()), len(graph.operation_log()))

        post_issues = graph.validate()
        self.assertTrue(
            all(issue.code != "VAL_SPEAKER_EMPTY" for issue in post_issues),
            "speaker-empty issue should be auto-fixed",
        )

    def test_node_graph_diagnostic_envelope_is_localized(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        graph.add_node(vn.StoryNode.dialogue("", "Hola"), 0.0, 0.0)
        issue = next(
            issue for issue in graph.validate() if issue.code == "VAL_SPEAKER_EMPTY"
        )

        self.assertEqual(issue.message_key, "diagnostic.val.speaker.empty")
        self.assertTrue(issue.docs_ref.startswith("docs/diagnostics/authoring.md#"))
        self.assertTrue(issue.action_steps_es)
        self.assertIsNotNone(issue.target)
        self.assertIsNotNone(issue.field_path)
        self.assertIsNotNone(issue.trace_id)
        self.assertGreaterEqual(len(issue.semantic_values), 1)
        localized = issue.localized("es")
        self.assertEqual(localized["schema"], "vnengine.diagnostic_envelope.v2")
        self.assertEqual(localized["message_key"], issue.message_key)
        self.assertEqual(localized["target"], issue.target)
        self.assertEqual(localized["field_path"], issue.field_path)
        self.assertEqual(localized["trace_id"], issue.trace_id)
        self.assertEqual(localized["semantic_values"], issue.semantic_values)
        self.assertIn("Speaker", localized["message"])
        self.assertTrue(localized["root_cause"])

    def test_node_graph_set_flag_and_authoring_save_contract(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        flag = graph.add_node(vn.StoryNode.set_flag("met_ava", True), 0.0, 100.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 200.0)
        graph.connect(start, flag)
        graph.connect(flag, end)

        script_payload = json.loads(graph.to_script_json())
        self.assertEqual(script_payload["events"][0]["type"], "set_flag")
        self.assertEqual(script_payload["events"][0]["key"], "met_ava")

        with workspace_tempdir("authoring-save") as tmp:
            path = tmp / "game.vnauthoring"
            graph.save(str(path))
            saved = path.read_text()
            self.assertIn("authoring_schema_version", saved)
            loaded = vn.NodeGraph.load(str(path))
            self.assertEqual(loaded.node_count(), graph.node_count())

    def test_node_graph_jump_if_two_ports_roundtrip_and_engine_compile(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "connect_port"):
            self.fail("Native graph binding does not expose port connections")

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
        vn.Engine(json.dumps(payload))

        restored = vn.NodeGraph.from_script_json(json.dumps(payload))
        self.assertEqual(
            json.loads(restored.to_script_json())["events"][0]["type"], "jump_if"
        )

    def test_node_graph_connect_or_branch_api_creates_real_choice_hub(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "connect_or_branch"):
            self.fail("Native graph binding does not expose branch connection API")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        source = graph.add_node(
            vn.StoryNode.dialogue("Narrator", "Pick a route"), 0.0, 100.0
        )
        first = graph.add_node(vn.StoryNode.dialogue("A", "First"), -100.0, 220.0)
        second = graph.add_node(vn.StoryNode.dialogue("B", "Second"), 100.0, 220.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 340.0)

        graph.connect(start, source)
        self.assertTrue(graph.connect_or_branch(source, 0, first))
        self.assertTrue(graph.connect_or_branch(source, 0, second))
        graph.connect(first, end)
        graph.connect(second, end)

        payload = json.loads(graph.to_script_json())
        choice = next(event for event in payload["events"] if event["type"] == "choice")
        self.assertEqual(
            [option["text"] for option in choice["options"]], ["Continue", "New route"]
        )
        target_texts = {
            payload["events"][payload["labels"][option["target"]]]["text"]
            for option in choice["options"]
        }
        self.assertEqual(target_texts, {"First", "Second"})

    def test_node_graph_connect_choice_new_option_uses_exportable_text(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "connect_or_branch"):
            self.fail("Native graph binding does not expose branch connection API")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        choice = graph.add_node(vn.StoryNode.choice("Route?", ["A"]), 0.0, 100.0)
        first = graph.add_node(vn.StoryNode.dialogue("A", "First"), -100.0, 220.0)
        second = graph.add_node(vn.StoryNode.dialogue("B", "Second"), 100.0, 220.0)
        end = graph.add_node(vn.StoryNode.end(), 0.0, 340.0)
        graph.connect(start, choice)
        graph.connect_port(choice, 0, first)
        graph.connect(first, end)
        graph.connect(second, end)

        self.assertTrue(graph.connect_or_branch(choice, 1, second))
        payload = json.loads(graph.to_script_json())
        choice_event = next(
            event for event in payload["events"] if event["type"] == "choice"
        )
        self.assertEqual(
            [option["text"] for option in choice_event["options"]],
            ["A", "New route"],
        )

    def test_python_can_inspect_branch_hub_connections_and_positions(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.fail("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "connections"):
            self.fail("Native graph binding does not expose graph inspection")

        graph = vn.NodeGraph()
        source = graph.add_node(vn.StoryNode.dialogue("N", "Go"), 0.0, 0.0)
        first = graph.add_node(vn.StoryNode.end(), -120.0, 180.0)
        second = graph.add_node(vn.StoryNode.end(), 120.0, 180.0)

        self.assertTrue(graph.connect_or_branch(source, 0, first))
        self.assertTrue(graph.connect_or_branch(source, 0, second))

        source_connection = next(
            connection for connection in graph.connections() if connection[0] == source
        )
        hub = source_connection[2]
        self.assertNotIn(hub, {source, first, second})
        self.assertEqual(set(graph.node_ids()), {source, first, second, hub})
        self.assertEqual(graph.get_node(hub).node_type, "Choice")
        self.assertEqual(graph.node_position(hub), (0.0, 90.0))
        hub_edges = {
            (port, target)
            for from_id, port, target in graph.connections()
            if from_id == hub
        }
        self.assertEqual(hub_edges, {(0, first), (1, second)})
        nodes = {node_id: node.node_type for node_id, node, _, _ in graph.nodes()}
        self.assertEqual(nodes[hub], "Choice")
