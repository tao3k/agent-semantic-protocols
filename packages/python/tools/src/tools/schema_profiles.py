"""Synchronize language-local copies of shared ASP schemas.

The root ``schemas/`` directory is the schema authority.  Language packages
may carry exact copies of shared contracts plus provider-owned schemas, but
they must not invent a second shared-schema authority.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from .console import emit


def _find_repo_root() -> Path:
    for candidate in Path(__file__).resolve().parents:
        if (candidate / "schemas" / "language-schema-profiles.json").is_file():
            return candidate
    raise RuntimeError("unable to locate ASP repository root")


REPO_ROOT = _find_repo_root()
PROFILE_REGISTRY = REPO_ROOT / "schemas" / "language-schema-profiles.json"


@dataclass(frozen=True)
class LanguageSchemaProfile:
    language_id: str
    package_root: str
    shared_schema_files: tuple[str, ...]
    provider_schema_files: tuple[str, ...]
    bundle_root: str | None = None


@dataclass(frozen=True)
class SchemaProfileChange:
    language_id: str
    action: str
    schema_name: str
    reason: str | None = None


def _load_profiles() -> tuple[LanguageSchemaProfile, ...]:
    document = json.loads(PROFILE_REGISTRY.read_text(encoding="utf-8"))
    root_sets = document["rootSets"]
    profiles = []
    for entry in document["profiles"]:
        shared = [
            schema_name
            for root_set in entry.get("rootSets", [])
            for schema_name in root_sets[root_set]
        ]
        provider_owned = [*entry.get("roots", []), *entry.get("providerOwned", [])]
        profiles.append(
            LanguageSchemaProfile(
                language_id=entry["languageId"],
                package_root=entry["packageRoot"],
                bundle_root=entry.get("bundleRoot"),
                shared_schema_files=tuple(dict.fromkeys(shared)),
                provider_schema_files=tuple(dict.fromkeys(provider_owned)),
            )
        )
    return tuple(profiles)


LANGUAGE_SCHEMA_PROFILES = _load_profiles()


def _schema_dir(repo_root: Path, profile: LanguageSchemaProfile) -> Path:
    return repo_root / (profile.bundle_root or f"{profile.package_root}/schemas")


def _org_owned(schema_name: str) -> bool:
    return schema_name.startswith(("semantic-org-", "org-"))


def schema_profile_changes(
    repo_root: Path, *, profiles: Iterable[LanguageSchemaProfile] = LANGUAGE_SCHEMA_PROFILES
) -> list[SchemaProfileChange]:
    changes: list[SchemaProfileChange] = []
    root_schema_dir = repo_root / "schemas"
    for profile in profiles:
        package_schema_dir = _schema_dir(repo_root, profile)
        shared = set(profile.shared_schema_files)
        provider_owned = set(profile.provider_schema_files)
        if profile.language_id != "org" and profile.language_id != "md":
            for schema_name in sorted(shared):
                if _org_owned(schema_name):
                    changes.append(
                        SchemaProfileChange(
                            profile.language_id,
                            "shared-schema-owned-by-org",
                            schema_name,
                        )
                    )
        for schema_name in sorted(provider_owned):
            if not (package_schema_dir / schema_name).is_file():
                changes.append(
                    SchemaProfileChange(
                        profile.language_id, "missing-provider", schema_name
                    )
                )
        if not package_schema_dir.is_dir():
            for schema_name in sorted(shared):
                if (root_schema_dir / schema_name).is_file():
                    changes.append(
                        SchemaProfileChange(
                            profile.language_id, "copy", schema_name, "missing"
                        )
                    )
            continue
        expected = shared | provider_owned
        for path in sorted(package_schema_dir.glob("*.schema.json")):
            if path.name not in expected:
                # Some older package bundles contain an exact mirror of a
                # current root schema without listing it in the compact
                # profile.  It is still safe: the root copy remains the
                # authority and an identical mirror cannot become a second
                # definition.  Non-root or divergent extras remain drift.
                root_path = root_schema_dir / path.name
                if root_path.is_file():
                    try:
                        if json.loads(path.read_text(encoding="utf-8")) == json.loads(
                            root_path.read_text(encoding="utf-8")
                        ):
                            continue
                    except (OSError, json.JSONDecodeError):
                        pass
                changes.append(
                    SchemaProfileChange(profile.language_id, "remove", path.name)
                )
        for schema_name in sorted(shared):
            root_path = root_schema_dir / schema_name
            package_path = package_schema_dir / schema_name
            if not root_path.is_file():
                continue
            reason = "missing"
            if package_path.is_file():
                try:
                    same = json.loads(package_path.read_text(encoding="utf-8")) == json.loads(
                        root_path.read_text(encoding="utf-8")
                    )
                except (OSError, json.JSONDecodeError):
                    same = False
                if same:
                    continue
                reason = "drifted"
            changes.append(
                SchemaProfileChange(profile.language_id, "copy", schema_name, reason)
            )
    return changes


def sync_language_schema_profiles(
    repo_root: Path,
    *,
    profiles: Iterable[LanguageSchemaProfile] = LANGUAGE_SCHEMA_PROFILES,
    check: bool = False,
) -> list[SchemaProfileChange]:
    profiles = tuple(profiles)
    changes = schema_profile_changes(repo_root, profiles=profiles)
    if check:
        return changes
    for change in changes:
        profile = next(profile for profile in profiles if profile.language_id == change.language_id)
        package_schema_dir = _schema_dir(repo_root, profile)
        package_schema_dir.mkdir(parents=True, exist_ok=True)
        path = package_schema_dir / change.schema_name
        if change.action == "remove":
            path.unlink(missing_ok=True)
        elif change.action == "copy":
            source = repo_root / "schemas" / change.schema_name
            if source.is_file():
                path.write_bytes(source.read_bytes())
    return changes


def schema_profile_errors(
    repo_root: Path, *, profiles: Iterable[LanguageSchemaProfile] = LANGUAGE_SCHEMA_PROFILES
) -> list[str]:
    errors = []
    for change in schema_profile_changes(repo_root, profiles=profiles):
        if change.action == "shared-schema-owned-by-org":
            errors.append(
                f"{change.language_id}: shared-schema-owned-by-org {change.schema_name}"
            )
        elif change.action == "missing-provider":
            errors.append(f"{change.language_id}: missing-provider {change.schema_name}")
        elif change.action == "remove":
            errors.append(f"{change.language_id}: remove {change.schema_name}")
        elif change.action == "copy":
            errors.append(
                f"{change.language_id}: copy {change.schema_name} reason={change.reason}"
            )
    return errors


def assert_language_schema_profiles(repo_root: Path) -> None:
    errors = schema_profile_errors(repo_root)
    if errors:
        raise AssertionError("\n".join(errors))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="schema-profiles")
    parser.add_argument("command", choices=("validate",))
    parser.add_argument("languages", nargs="*")
    args = parser.parse_args(argv)
    selected = tuple(
        profile
        for profile in LANGUAGE_SCHEMA_PROFILES
        if not args.languages or profile.language_id in args.languages
    )
    errors = schema_profile_errors(REPO_ROOT, profiles=selected)
    if errors:
        emit("\n".join(errors), file=sys.stderr)
        return 1
    emit(f"[schema-profiles] ok languages={','.join(profile.language_id for profile in selected)}")
    return 0
