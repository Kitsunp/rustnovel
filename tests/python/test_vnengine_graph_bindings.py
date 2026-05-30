import hashlib
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


def minimal_pe_exe() -> bytes:
    payload = bytearray(128)
    payload[0:2] = b"MZ"
    payload[0x3C:0x40] = (0x40).to_bytes(4, "little")
    payload[0x40:0x44] = b"PE\0\0"
    payload[0x44:0x46] = (0x8664).to_bytes(2, "little")
    return bytes(payload)


class GuiBindingTests(unittest.TestCase):
    def test_run_visual_novel_rejects_invalid_json(self):
        import visual_novel_engine as vn

        with self.assertRaises(vn.VnValidationError):
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
            runtime_bytes = minimal_pe_exe()
            (project / "runtime" / "vn-runtime.exe").write_bytes(runtime_bytes)

            out = root / "dist"
            plan_obj = vn.plan_export(
                str(project),
                str(out),
                runtime_artifact="runtime/vn-runtime.exe",
            )
            self.assertTrue(hasattr(plan_obj, "to_dict"))
            plan = plan_obj.to_dict()
            self.assertEqual(plan["schema"], "vnengine.export_plan.v1")
            self.assertEqual(plan["executable"], "game.exe")
            self.assertNotIn("warnings", plan)
            self.assertNotIn("errors", plan)
            self.assertNotIn("warnings", plan["capabilities"])

            report_obj = vn.export_bundle(
                str(project),
                str(out),
                runtime_artifact="runtime/vn-runtime.exe",
            )
            self.assertTrue(hasattr(report_obj, "to_dict"))
            report = report_obj.to_dict()

            self.assertEqual(report["runtime_artifact"], "runtime/vn-runtime.exe")
            self.assertNotIn("warnings", report["capabilities"])
            self.assertEqual(
                report["runtime_artifact_sha256"],
                hashlib.sha256(runtime_bytes).hexdigest(),
            )
            self.assertEqual(report["executable"], "game.exe")
            self.assertEqual(report["smoke_result"]["status"], "not_run")
            self.assertTrue(
                report["smoke_result"]["trace_id"].startswith("export-smoke-")
            )
            self.assertEqual((out / "game.exe").read_bytes(), runtime_bytes)
            package_report = json.loads((out / "meta/package_report.json").read_text())
            compat_report = json.loads((out / "meta/compat_report.json").read_text())
            manifest_bytes = (out / "meta/bundle_file_manifest.json").read_bytes()
            manifest = json.loads(manifest_bytes)
            manifest_hash = hashlib.sha256(manifest_bytes).hexdigest()
            self.assertEqual(
                compat_report["runtime_artifact_sha256"],
                package_report["runtime_artifact_sha256"],
            )
            self.assertEqual(report["generator_os"], compat_report["generator_os"])
            self.assertEqual(
                report["expected_executable"], compat_report["expected_executable"]
            )
            self.assertEqual(
                report["graphics_backend"], compat_report["graphics_backend"]
            )
            self.assertEqual(report["wgpu_fallback"], compat_report["wgpu_fallback"])
            self.assertEqual(report["total_size"], compat_report["total_size"])
            self.assertEqual(report["hashes"], manifest["files"])
            self.assertEqual(compat_report["hashes"], manifest["files"])
            self.assertEqual(report["bundle_file_manifest_sha256"], manifest_hash)
            self.assertEqual(
                package_report["bundle_file_manifest_sha256"], manifest_hash
            )
            self.assertEqual(
                compat_report["bundle_file_manifest_sha256"], manifest_hash
            )
            self.assertEqual(package_report["smoke_result"], report["smoke_result"])
            self.assertEqual(compat_report["smoke_result"], report["smoke_result"])

    def test_export_bundle_api_preserves_core_error_type_and_trace_context(self):
        import visual_novel_engine as vn

        with workspace_tempdir("export-bundle-error") as root:
            project = root / "project"
            project.mkdir(parents=True)
            (project / "project.vnm").write_text(
                "\n".join(
                    [
                        'manifest_schema_version = "1.0"',
                        "",
                        "[metadata]",
                        'name = "Python Export Error"',
                        'author = "QA"',
                        'version = "0.1.0"',
                        "",
                        "[settings]",
                        "resolution = [1280, 720]",
                        'default_language = "en"',
                        'supported_languages = ["en"]',
                        'entry_point = "main.json"',
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
                                "text": "Missing runtime should stay typed",
                            }
                        ],
                        "labels": {"start": 0},
                    },
                    separators=(",", ":"),
                    sort_keys=True,
                ),
                encoding="utf-8",
            )

            out = root / "dist"
            with self.assertRaises(vn.VnValidationError) as raised:
                vn.export_bundle(
                    str(project),
                    str(out),
                    require_executable=True,
                )
            message = str(raised.exception)
            self.assertIn("export.runtime_artifact.missing", message)
            self.assertIn("trace_id=export-", message)
            self.assertIn("field=runtime_artifact", message)
            self.assertIn("action=", message)
            self.assertFalse(out.exists(), "failed Python export must not publish output")

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

    def test_node_graph_review_autofix_is_explicit_for_missing_start(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph"):
            self.fail("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        issues = graph.validate()
        idx = next(
            i for i, issue in enumerate(issues) if issue.code == "VAL_START_MISSING"
        )
        candidates = graph.fix_candidates(idx)
        self.assertEqual(candidates[0].fix_id, "graph_add_start")
        with self.assertRaises(ValueError):
            graph.autofix_issue(idx, False)

        applied = graph.autofix_issue(idx, True)
        self.assertEqual(applied, "graph_add_start")
        self.assertNotIn(
            "VAL_START_MISSING", {issue.code for issue in graph.validate()}
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
        hub_node = graph.get_node(hub)
        self.assertIsNotNone(hub_node)
        self.assertEqual(hub_node.node_type, "Choice")
        self.assertEqual(graph.node_position(hub), (0.0, 90.0))
        hub_edges = {
            (port, target)
            for from_id, port, target in graph.connections()
            if from_id == hub
        }
        self.assertEqual(hub_edges, {(0, first), (1, second)})
        nodes = {node_id: node.node_type for node_id, node, _, _ in graph.nodes()}
        self.assertEqual(nodes[hub], "Choice")
