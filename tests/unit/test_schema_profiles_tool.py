"""Schema profile maintenance tool tests."""

from __future__ import annotations

import json
import sys
from pathlib import Path


_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(_ROOT / "packages/python/src"))

import tools.schema_profiles as schema_profiles_module  # noqa: E402
from tools.schema_profiles import (  # noqa: E402
    LanguageSchemaProfile,
    schema_profile_changes,
    schema_profile_errors,
    sync_language_schema_profiles,
)


def test_schema_profile_sync_check_reports_without_mutation(tmp_path: Path) -> None:
    profile = _write_demo_profile(tmp_path)

    changes = sync_language_schema_profiles(tmp_path, profiles=(profile,), check=True)

    assert [change.action for change in changes] == ["remove", "copy"]
    assert _load_json(tmp_path / profile.package_root / "schemas/shared.schema.json") == {
        "version": 0
    }
    assert (tmp_path / profile.package_root / "schemas/extra.schema.json").exists()


def test_schema_profile_sync_repairs_drift_and_prunes_extra(tmp_path: Path) -> None:
    profile = _write_demo_profile(tmp_path)

    changes = sync_language_schema_profiles(tmp_path, profiles=(profile,))

    assert [change.action for change in changes] == ["remove", "copy"]
    assert _load_json(tmp_path / profile.package_root / "schemas/shared.schema.json") == {
        "version": 1
    }
    assert not (tmp_path / profile.package_root / "schemas/extra.schema.json").exists()
    assert schema_profile_errors(tmp_path, profiles=(profile,)) == []


def test_schema_profile_sync_blocks_missing_provider_owned_schema(tmp_path: Path) -> None:
    profile = _write_demo_profile(tmp_path)
    (tmp_path / profile.package_root / "schemas/demo.schema.json").unlink()

    changes = sync_language_schema_profiles(tmp_path, profiles=(profile,))

    assert [change.action for change in changes] == [
        "missing-provider",
        "remove",
        "copy",
    ]
    assert _load_json(tmp_path / profile.package_root / "schemas/shared.schema.json") == {
        "version": 0
    }
    assert (tmp_path / profile.package_root / "schemas/extra.schema.json").exists()
    assert schema_profile_changes(tmp_path, profiles=(profile,)) == changes


def test_schema_profile_validate_cli_accepts_synced_profile(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    profile = _write_demo_profile(tmp_path)
    sync_language_schema_profiles(tmp_path, profiles=(profile,))
    monkeypatch.setattr(schema_profiles_module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(schema_profiles_module, "LANGUAGE_SCHEMA_PROFILES", (profile,))

    assert schema_profiles_module.main(["validate", "demo"]) == 0

    captured = capsys.readouterr()
    assert captured.out == "[schema-profiles] ok languages=demo\n"
    assert captured.err == ""


def test_schema_profile_validate_cli_fails_on_drift(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    profile = _write_demo_profile(tmp_path)
    monkeypatch.setattr(schema_profiles_module, "REPO_ROOT", tmp_path)
    monkeypatch.setattr(schema_profiles_module, "LANGUAGE_SCHEMA_PROFILES", (profile,))

    assert schema_profiles_module.main(["validate", "demo"]) == 1

    captured = capsys.readouterr()
    assert captured.out == ""
    assert "demo: remove extra.schema.json" in captured.err
    assert "demo: copy shared.schema.json reason=drifted" in captured.err


def _write_demo_profile(repo_root: Path) -> LanguageSchemaProfile:
    profile = LanguageSchemaProfile(
        language_id="demo",
        package_root="languages/demo",
        shared_schema_files=("shared.schema.json",),
        provider_schema_files=("demo.schema.json",),
    )
    root_schema_dir = repo_root / "schemas"
    package_schema_dir = repo_root / profile.package_root / "schemas"
    root_schema_dir.mkdir(parents=True)
    package_schema_dir.mkdir(parents=True)
    _write_json(root_schema_dir / "shared.schema.json", {"version": 1})
    _write_json(package_schema_dir / "shared.schema.json", {"version": 0})
    _write_json(package_schema_dir / "demo.schema.json", {"provider": "demo"})
    _write_json(package_schema_dir / "extra.schema.json", {"extra": True})
    return profile


def _write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value), encoding="utf-8")


def _load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))
