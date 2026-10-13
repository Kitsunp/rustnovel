import json

import pytest
import visual_novel_engine as vn


def make_graph():
    graph = vn.NodeGraph()
    start = graph.add_node(vn.StoryNode.start(), 0.0, 0.0)
    scene = graph.add_node(
        vn.StoryNode.scene_full(None, "room.png", "theme.ogg", []), 0.0, 100.0
    )
    line = graph.add_node(vn.StoryNode.dialogue("Ava", "Hello"), 0.0, 200.0)
    end = graph.add_node(vn.StoryNode.end(), 0.0, 300.0)
    graph.connect(start, scene)
    graph.connect(scene, line)
    graph.connect(line, end)
    return graph, scene


def test_bookmarks_keep_layers_and_prior_undo_history(tmp_path):
    graph, scene = make_graph()
    layer = next(obj for obj in graph.list_layered_objects(scene) if obj.asset_path == "room.png")
    graph.set_layer_visible(layer.object_id, False)
    assert graph.set_bookmark("intro", scene)
    operations = len(graph.operation_log())
    assert graph.set_bookmark("intro", scene)  # Existing API reports acceptance, including repeats.
    assert not graph.remove_bookmark("missing")
    assert len(graph.operation_log()) == operations
    assert graph.undo()
    assert graph.bookmark_target("intro") is None
    assert not next(obj for obj in graph.list_layered_objects(scene) if obj.object_id == layer.object_id).visible
    assert graph.undo()
    assert next(obj for obj in graph.list_layered_objects(scene) if obj.object_id == layer.object_id).visible
    assert graph.redo()
    assert graph.redo()
    assert graph.bookmark_target("intro") == scene
    path = tmp_path / "document.json"
    graph.save(str(path))
    loaded = vn.NodeGraph.load(str(path))
    assert loaded.bookmark_target("intro") == scene
    assert not next(obj for obj in loaded.list_layered_objects(scene) if obj.object_id == layer.object_id).visible
    assert len(loaded.operation_log()) == len(graph.operation_log())
    assert len(loaded.verification_runs()) == len(graph.verification_runs())
    second = tmp_path / "roundtrip.json"
    loaded.save(str(second))
    assert json.loads(path.read_text()) == json.loads(second.read_text())
    with pytest.raises(ValueError):
        loaded.save(str(tmp_path))
    assert json.loads(path.read_text()) == json.loads(second.read_text())


def test_preview_retains_audio_until_the_client_consumes_it():
    graph, scene = make_graph()
    preview = graph.preview_start_from_node(scene)
    commands = preview.take_audio_commands()
    assert any(command["type"] == "play_bgm" and command["path"] == "theme.ogg" for command in commands)
    assert preview.take_audio_commands() == []
    preview.advance()
    assert isinstance(preview.take_audio_commands(), list)


def test_empty_history_and_malformed_authoring_document_are_explicit():
    graph = vn.NodeGraph()
    assert not graph.undo()
    assert not graph.redo()
    with pytest.raises(ValueError):
        vn.NodeGraph.from_authoring_or_script_json('{"authoring_schema_version":999,"graph":{}}')
