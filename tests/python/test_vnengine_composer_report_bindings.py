import json
import shutil
import unittest
from contextlib import contextmanager
from pathlib import Path


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


class ComposerReportBindingTests(unittest.TestCase):
    def test_composer_layer_overrides_are_visible_in_python_snapshots(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "set_layer_visible"):
            self.skipTest("Native graph binding does not expose composer layers")

        graph = vn.NodeGraph()
        scene = graph.add_node(
            vn.StoryNode.scene_full(
                None,
                "bg/room.png",
                None,
                [("Ava", "characters/ava.png", "center", 10, 20, 1.0)],
            ),
            0.0,
            0.0,
        )
        objects = graph.list_layered_objects(scene)
        character = next(obj for obj in objects if obj.character_name == "Ava")

        graph.set_layer_locked(character.object_id, True)
        locked_character = next(
            obj
            for obj in graph.compose_scene_snapshot(scene).objects
            if obj.object_id == character.object_id
        )
        self.assertTrue(locked_character.locked)
        self.assertTrue(locked_character.visible)

        graph.set_layer_visible(character.object_id, False)
        hidden_character = next(
            obj
            for obj in graph.list_layered_objects(scene)
            if obj.object_id == character.object_id
        )
        self.assertFalse(hidden_character.visible)
        self.assertTrue(hidden_character.locked)

    def test_composer_layer_overrides_roundtrip_through_authoring_save(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "set_layer_visible"):
            self.skipTest("Native graph binding does not expose composer layers")

        graph = vn.NodeGraph()
        scene = graph.add_node(
            vn.StoryNode.scene_full(
                None,
                "bg/room.png",
                None,
                [("Ava", "characters/ava.png", "center", 10, 20, 1.0)],
            ),
            0.0,
            0.0,
        )
        character = next(
            obj
            for obj in graph.list_layered_objects(scene)
            if obj.character_name == "Ava"
        )
        graph.set_layer_locked(character.object_id, True)
        graph.set_layer_visible(character.object_id, False)

        with workspace_tempdir("composer-layer-override-save") as tmp:
            path = tmp / "game.vnauthoring"
            graph.save(str(path))
            loaded = vn.NodeGraph.load(str(path))
            restored = next(
                obj
                for obj in loaded.list_layered_objects(scene)
                if obj.object_id == character.object_id
            )

        self.assertFalse(restored.visible)
        self.assertTrue(restored.locked)

    def test_validation_report_fingerprint_includes_composer_layer_overrides(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "validation_report"):
            self.skipTest("Native graph binding does not expose report v2")

        graph = vn.NodeGraph()
        scene = graph.add_node(
            vn.StoryNode.scene_full(
                None,
                "bg/room.png",
                None,
                [("Ava", "characters/ava.png", "center", 10, 20, 1.0)],
            ),
            0.0,
            0.0,
        )
        character = next(
            obj
            for obj in graph.list_layered_objects(scene)
            if obj.character_name == "Ava"
        )
        before = json.loads(graph.validation_report().fingerprints_json())

        graph.set_layer_visible(character.object_id, False)
        after = json.loads(graph.validation_report().fingerprints_json())

        self.assertEqual(
            before["story_semantic_sha256"],
            after["story_semantic_sha256"],
            "composer-only layer state must not stale semantic reports",
        )
        self.assertNotEqual(before["layout_sha256"], after["layout_sha256"])
        self.assertNotEqual(
            before["full_document_sha256"], after["full_document_sha256"]
        )

    def test_validation_report_exposes_issue_envelopes_and_stale_compare(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn, "AuthoringValidationReport"):
            self.skipTest("Native report v2 bindings are not available")

        graph = vn.NodeGraph()
        graph.add_node(
            vn.StoryNode.choice("Route?", ["Option 1"]),
            0.0,
            0.0,
        )
        report = graph.validation_report()
        issue_envelopes = [json.loads(item) for item in report.issues()]
        self.assertEqual(len(issue_envelopes), report.issue_count)
        self.assertIn("diagnostic_id", issue_envelopes[0])
        self.assertEqual(
            len(json.loads(report.issues_json())),
            len(issue_envelopes),
        )
        self.assertFalse(report.is_stale_against(report.fingerprints_json()))

        graph.add_node(
            vn.StoryNode.dialogue("Narrator", "New semantic node"), 120.0, 0.0
        )
        changed_report = graph.validation_report()
        self.assertTrue(report.is_stale_against_report(changed_report))

    def test_node_graph_preserves_operation_log_and_verifications(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "operation_log"):
            self.skipTest("Native graph binding does not expose operation log")

        graph = vn.NodeGraph()
        graph.add_node(vn.StoryNode.dialogue("Narrator", "Trace me"), 0.0, 0.0)

        with workspace_tempdir("authoring-operation-log-save") as tmp:
            path = tmp / "game.vnauthoring"
            graph.save(str(path))
            document = json.loads(path.read_text())
            document["operation_log"] = [
                {
                    "schema": "vnengine.operation_log.v2",
                    "operation_id": "op:test-python-preserve",
                    "created_unix_ms": 1,
                    "operation_kind": "node_created",
                    "operation_kind_v2": "node_created",
                    "diagnostic_id": None,
                    "semantic_fingerprint_sha256": None,
                    "status": "applied",
                    "details": "created from external tool",
                }
            ]
            document["verification_runs"] = [
                {
                    "schema": "vnengine.verification_run.v2",
                    "operation_id": "op:test-python-preserve",
                    "created_unix_ms": 2,
                    "validation_profile": "python-test",
                    "semantic_fingerprint_sha256": "semantic",
                    "story_semantic_sha256": "semantic",
                    "diagnostic_ids": [],
                    "resolved_diagnostic_ids": [],
                    "introduced_diagnostic_ids": [],
                }
            ]
            path.write_text(json.dumps(document, separators=(",", ":"), sort_keys=True))

            loaded = vn.NodeGraph.load(str(path))
            self.assertEqual(len(loaded.operation_log()), 1)
            self.assertEqual(len(loaded.verification_runs()), 1)
            self.assertIn(
                "op:test-python-preserve", loaded.operation_log()[0].to_json()
            )

            roundtrip = tmp / "roundtrip.vnauthoring"
            loaded.save(str(roundtrip))
            saved = json.loads(roundtrip.read_text())

        self.assertEqual(
            saved["operation_log"][0]["operation_id"], "op:test-python-preserve"
        )
        self.assertEqual(
            saved["verification_runs"][0]["operation_id"], "op:test-python-preserve"
        )

    def test_python_graph_mutations_emit_operation_log_and_verification_runs(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "operation_log"):
            self.skipTest("Native graph binding does not expose operation log")

        graph = vn.NodeGraph()
        start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
        scene = graph.add_node(
            vn.StoryNode.scene_full(
                None,
                "bg/room.png",
                None,
                [("Ava", "characters/ava.png", "center", 10, 20, 1.0)],
            ),
            0.0,
            100.0,
        )
        graph.connect(start, scene)
        self.assertTrue(graph.create_fragment("intro", "Intro", [scene]))
        self.assertTrue(graph.enter_fragment("intro"))
        self.assertTrue(graph.leave_fragment())
        character = next(
            obj
            for obj in graph.list_layered_objects(scene)
            if obj.character_name == "Ava"
        )
        graph.set_layer_locked(character.object_id, True)
        self.assertFalse(
            graph.move_scene_object(character.object_id, 40, 50, 1.1),
            "locked composer objects must not move through the Python headless API",
        )
        graph.set_layer_locked(character.object_id, False)
        self.assertTrue(graph.move_scene_object(character.object_id, 40, 50, 1.1))

        operations = [json.loads(entry.to_json()) for entry in graph.operation_log()]
        kinds = [entry["operation_kind"] for entry in operations]
        for expected in [
            "node_created",
            "node_connected",
            "fragment_created",
            "fragment_entered",
            "fragment_left",
            "layer_lock_changed",
            "composer_object_moved",
        ]:
            self.assertIn(expected, kinds)
        self.assertEqual(len(graph.verification_runs()), len(graph.operation_log()))
        moved = next(
            entry
            for entry in operations
            if entry["operation_kind"] == "composer_object_moved"
        )
        self.assertIn("composer.objects[", moved["field_paths"][0]["value"])
        self.assertIsNotNone(moved["before_fingerprint_sha256"])
        self.assertIsNotNone(moved["after_fingerprint_sha256"])

    def test_python_composer_choice_edits_preserve_targets_and_trace_fields(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")
        if not hasattr(vn.NodeGraph, "reorder_choice_option"):
            self.skipTest(
                "Native graph binding does not expose composer choice editing"
            )

        graph = vn.NodeGraph()
        choice = graph.add_node(
            vn.StoryNode.choice("Old prompt", ["Left", "Right"]), 0.0, 0.0
        )
        left = graph.add_node(vn.StoryNode.dialogue("A", "Left route"), -100.0, 100.0)
        right = graph.add_node(vn.StoryNode.dialogue("B", "Right route"), 100.0, 100.0)
        graph.connect_port(choice, 0, left)
        graph.connect_port(choice, 1, right)

        self.assertTrue(graph.edit_choice_prompt(choice, "New prompt"))
        self.assertTrue(graph.edit_choice_option_text(choice, 0, "Stay left"))
        self.assertTrue(graph.reorder_choice_option(choice, 0, 1))
        self.assertEqual(
            sorted(graph.connections()), [(choice, 0, right), (choice, 1, left)]
        )
        self.assertTrue(graph.set_choice_option_target(choice, 0, left))
        self.assertEqual(
            sorted(graph.connections()), [(choice, 0, left), (choice, 1, left)]
        )

        operations = [json.loads(entry.to_json()) for entry in graph.operation_log()]
        edited = [
            entry for entry in operations if entry["operation_kind"] == "field_edited"
        ]
        field_paths = [entry["field_paths"][0]["value"] for entry in edited]
        self.assertIn(f"graph.nodes[{choice}].choice.prompt", field_paths)
        self.assertIn(f"graph.nodes[{choice}].choice.options[0].text", field_paths)
        self.assertIn(f"graph.nodes[{choice}].choice.options", field_paths)
        self.assertIn(f"graph.nodes[{choice}].choice.options[0].target", field_paths)
        self.assertEqual(len(graph.verification_runs()), len(graph.operation_log()))

    def test_node_graph_validate_rejects_windows_drive_asset_path(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        graph = vn.NodeGraph()
        graph.add_node(vn.StoryNode.scene(r"C:\temp\evil.png", None, []), 0.0, 0.0)
        codes = {issue.code for issue in graph.validate()}
        self.assertIn("VAL_ASSET_UNSAFE_PATH", codes)

    def test_node_graph_validate_accepts_project_root(self):
        import visual_novel_engine as vn

        if not hasattr(vn, "NodeGraph") or not hasattr(vn, "StoryNode"):
            self.skipTest("GUI graph bindings are not available in this native build")

        with workspace_tempdir("project-root-validation") as root:
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
