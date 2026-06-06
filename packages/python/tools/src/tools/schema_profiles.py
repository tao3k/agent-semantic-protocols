"""Language package schema downsync profiles."""

from __future__ import annotations

import json
import shutil
import sys
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path

from tools.console import emit
from tools.schema_profile_catalog import (
    LANGUAGE_SCHEMA_PROFILES,
    LanguageSchemaProfile,
    select_schema_profiles,
)
from tools.paths import repo_root

REPO_ROOT = repo_root()


@dataclass(frozen=True)
class SchemaProfileChange:
    """One concrete filesystem action needed to match a schema profile."""

    language_id: str
    action: str
    schema_name: str
    detail: str

    def render(self) -> str:
        return f"{self.language_id}: {self.action} {self.schema_name} {self.detail}".rstrip()


def schema_profiles(language_ids: Iterable[str] = ()) -> tuple[LanguageSchemaProfile, ...]:
    return select_schema_profiles(LANGUAGE_SCHEMA_PROFILES, language_ids)


def schema_profile_errors(
    repo_root: Path = REPO_ROOT,
    *,
    profiles: Iterable[LanguageSchemaProfile] | None = None,
) -> list[str]:
    return [change.render() for change in schema_profile_changes(repo_root, profiles=profiles)]


def schema_profile_changes(
    repo_root: Path = REPO_ROOT,
    *,
    profiles: Iterable[LanguageSchemaProfile] | None = None,
) -> list[SchemaProfileChange]:
    selected_profiles = tuple(LANGUAGE_SCHEMA_PROFILES if profiles is None else profiles)
    return [
        change
        for profile in selected_profiles
        for change in _profile_changes(repo_root, profile)
    ]


def _profile_changes(
    repo_root: Path,
    profile: LanguageSchemaProfile,
) -> tuple[SchemaProfileChange, ...]:
    schema_dir = repo_root / profile.package_root / "schemas"
    present = {path.name for path in schema_dir.glob("*.json")}
    allowed = set(profile.allowed_schema_files)
    return (
        *_missing_schema_changes(profile, allowed - present),
        *_extra_schema_changes(profile, present - allowed),
        *_drift_schema_changes(repo_root, profile, schema_dir),
    )


def _missing_schema_changes(
    profile: LanguageSchemaProfile,
    schema_names: Iterable[str],
) -> tuple[SchemaProfileChange, ...]:
    return tuple(
        SchemaProfileChange(
            profile.language_id,
            "missing-provider" if schema_name in profile.provider_schema_files else "copy",
            schema_name,
            (
                "owner=provider"
                if schema_name in profile.provider_schema_files
                else "from=root"
            ),
        )
        for schema_name in sorted(schema_names)
    )


def _extra_schema_changes(
    profile: LanguageSchemaProfile,
    schema_names: Iterable[str],
) -> tuple[SchemaProfileChange, ...]:
    return tuple(
        SchemaProfileChange(
            profile.language_id,
            "remove",
            schema_name,
            f"path={profile.package_root}/schemas/{schema_name}",
        )
        for schema_name in sorted(schema_names)
    )


def _drift_schema_changes(
    repo_root: Path,
    profile: LanguageSchemaProfile,
    schema_dir: Path,
) -> tuple[SchemaProfileChange, ...]:
    return tuple(
        change
        for schema_name in profile.shared_schema_files
        for change in _shared_schema_change(repo_root, profile, schema_dir, schema_name)
    )


def _shared_schema_change(
    repo_root: Path,
    profile: LanguageSchemaProfile,
    schema_dir: Path,
    schema_name: str,
) -> tuple[SchemaProfileChange, ...]:
    root_schema = repo_root / "schemas" / schema_name
    package_schema = schema_dir / schema_name
    if not root_schema.exists():
        return (
            SchemaProfileChange(
                profile.language_id,
                "missing-root",
                schema_name,
                "owner=root",
            ),
        )
    if not package_schema.exists() or _load_json(package_schema) == _load_json(root_schema):
        return ()
    return (
        SchemaProfileChange(
            profile.language_id,
            "copy",
            schema_name,
            "reason=drifted",
        ),
    )


def assert_language_schema_profiles(repo_root: Path = REPO_ROOT) -> None:
    errors = schema_profile_errors(repo_root)
    assert not errors, "\n".join(errors)


def sync_language_schema_profiles(
    repo_root: Path = REPO_ROOT,
    *,
    profiles: Iterable[LanguageSchemaProfile] | None = None,
    check: bool = False,
) -> list[SchemaProfileChange]:
    selected_profiles = tuple(LANGUAGE_SCHEMA_PROFILES if profiles is None else profiles)
    changes = schema_profile_changes(repo_root, profiles=selected_profiles)
    if check:
        return changes
    blocking_actions = {"missing-provider", "missing-root"}
    blocking = [change for change in changes if change.action in blocking_actions]
    if blocking:
        return changes
    for change in changes:
        profile = _profile_by_language(selected_profiles, change.language_id)
        schema_dir = repo_root / profile.package_root / "schemas"
        package_schema = schema_dir / change.schema_name
        if change.action == "copy":
            root_schema = repo_root / "schemas" / change.schema_name
            schema_dir.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(root_schema, package_schema)
        elif change.action == "remove":
            package_schema.unlink()
    return changes


def _profile_by_language(
    profiles: Iterable[LanguageSchemaProfile],
    language_id: str,
) -> LanguageSchemaProfile:
    for profile in profiles:
        if profile.language_id == language_id:
            return profile
    raise ValueError(f"unknown language schema profile: {language_id}")


def _load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if args in ([], ["help"], ["--help"], ["-h"]):
        emit("usage: python -m tools schema profiles <list|validate|sync> [--check] [language ...]")
        return 0 if args else 2
    command = args[0]
    check = "--check" in args[1:]
    language_ids = [arg for arg in args[1:] if arg != "--check"]
    try:
        profiles = schema_profiles(language_ids)
    except ValueError as error:
        emit(str(error), file=sys.stderr)
        return 2

    if command == "list":
        for profile in profiles:
            emit(
                f"{profile.language_id} shared={len(profile.shared_schema_files)} "
                f"provider={len(profile.provider_schema_files)} "
                f"total={len(profile.allowed_schema_files)}"
            )
        return 0
    if command == "validate":
        errors = schema_profile_errors(REPO_ROOT, profiles=profiles)
        if errors:
            for error in errors:
                emit(error, file=sys.stderr)
            return 1
        emit(
            "[schema-profiles] ok "
            f"languages={','.join(profile.language_id for profile in profiles)}"
        )
        return 0
    if command == "sync":
        changes = sync_language_schema_profiles(REPO_ROOT, profiles=profiles, check=check)
        if not changes:
            emit(
                "[schema-profiles] ok "
                f"languages={','.join(profile.language_id for profile in profiles)}"
            )
            return 0
        for change in changes:
            emit(change.render(), file=sys.stderr if check else sys.stdout)
        if check:
            return 1
        blocking = [change for change in changes if change.action.startswith("missing-")]
        return 1 if blocking else 0

    emit(f"unknown schema profile command: {command}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
