import json

import pytest
import visual_novel_engine as native


def test_import_project_asset_reuses_content_and_preserves_manifest_on_error(tmp_path):
    project = tmp_path / "project"
    project.mkdir()
    manifest = project / "project.vnm"
    manifest.write_text(
        """manifest_schema_version = "1.0"
[metadata]
name = "Import"
author = "Tests"
version = "0.1.0"
[settings]
resolution = [1280, 720]
[assets]
""",
        encoding="utf-8",
    )
    source = tmp_path / "theme.ogg"
    source.write_bytes(b"audio fixture")
    first = json.loads(
        native.import_project_asset_json(str(manifest), str(source), "audio")
    )
    assert first["imported"] and first["copied"]
    assert (project / first["path"]).read_bytes() == source.read_bytes()
    saved = manifest.read_bytes()
    renamed = tmp_path / "renamed.ogg"
    renamed.write_bytes(source.read_bytes())
    repeated = json.loads(
        native.import_project_asset_json(str(manifest), str(renamed), "audio")
    )
    assert not repeated["imported"] and not repeated["copied"]
    assert repeated["path"] == first["path"]
    assert manifest.read_bytes() == saved
    source.write_bytes(b"different")
    for kind, limit in [("audio", 1), ("background", 100), ("unknown", 100)]:
        with pytest.raises(ValueError):
            native.import_project_asset_json(str(manifest), str(source), kind, limit)
        assert manifest.read_bytes() == saved
    assert list((project / "assets" / "audio").iterdir()) == [project / first["path"]]
