#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
"""Validate the SPDX license contract for every Python workspace project."""

from __future__ import annotations

import argparse
import sys
import tomllib
from pathlib import Path
from typing import Any


LICENSE_EXPRESSION = "Apache-2.0 AND LGPL-2.1-or-later"
LICENSE_FILES = ["LICENSE"]
SPDX_LICENSE_TAG = "SPDX-" + "License-Identifier"
SPDX_LINE = f"{SPDX_LICENSE_TAG}: {LICENSE_EXPRESSION}"
REQUIRED_LICENSE_TEXT = (
    "Apache License",
    "Version 2.0, January 2004",
    "GNU LESSER GENERAL PUBLIC LICENSE",
    "Version 2.1, February 1999",
)
COPYRIGHT_TEXT = "2026 tao3k team and Contributors"
COMMENTABLE_SUFFIXES = frozenset({".lean", ".md", ".py", ".toml"})
ANNOTATED_SUFFIXES = frozenset({".json", ".lock", ".typed"})


def license_project_files(root: Path) -> tuple[Path, ...]:
    """Return project metadata files while excluding hidden tool environments."""
    return tuple(
        path
        for path in sorted(root.rglob("pyproject.toml"))
        if not any(part.startswith(".") for part in path.relative_to(root).parts)
    )


def workspace_files(root: Path) -> tuple[Path, ...]:
    """Return non-generated workspace files that require SPDX coverage."""
    return tuple(
        path
        for path in sorted(root.rglob("*"))
        if path.is_file()
        and not any(part.startswith(".") for part in path.relative_to(root).parts)
        and "__pycache__" not in path.parts
    )


def annotation_paths(root: Path, errors: list[str]) -> set[str]:
    """Load exact non-commentable-file annotations from REUSE.toml."""
    reuse_path = root / "REUSE.toml"
    if not reuse_path.is_file():
        errors.append("REUSE.toml: missing file annotations")
        return set()
    payload = tomllib.loads(reuse_path.read_text(encoding="utf-8"))
    if payload.get("version") != 1:
        errors.append("REUSE.toml: version must equal 1")
    result: set[str] = set()
    for index, annotation in enumerate(payload.get("annotations", []), start=1):
        if not isinstance(annotation, dict):
            errors.append(f"REUSE.toml: annotation {index} must be a table")
            continue
        if annotation.get("SPDX-FileCopyrightText") != COPYRIGHT_TEXT:
            errors.append(f"REUSE.toml: annotation {index} has invalid copyright")
        if annotation.get("SPDX-License-Identifier") != LICENSE_EXPRESSION:
            errors.append(f"REUSE.toml: annotation {index} has invalid license")
        paths: Any = annotation.get("path", [])
        if isinstance(paths, str):
            result.add(paths)
        elif isinstance(paths, list) and all(isinstance(path, str) for path in paths):
            result.update(paths)
        else:
            errors.append(f"REUSE.toml: annotation {index} has invalid paths")
    return result


def validate_file_spdx_coverage(root: Path) -> list[str]:
    """Validate inline headers and REUSE annotations for every workspace file."""
    errors: list[str] = []
    annotated = annotation_paths(root, errors)
    expected_annotations: set[str] = set()
    copyright_tag = f"SPDX-FileCopyrightText: {COPYRIGHT_TEXT}"
    license_tag = f"{SPDX_LICENSE_TAG}: {LICENSE_EXPRESSION}"

    for path in workspace_files(root):
        relative = path.relative_to(root).as_posix()
        if path.name == "REUSE.toml" or path.name.startswith("LICENSE"):
            continue
        if path.suffix in COMMENTABLE_SUFFIXES:
            header = "\n".join(path.read_text(encoding="utf-8").splitlines()[:12])
            if copyright_tag not in header:
                errors.append(f"{relative}: missing SPDX copyright header")
            if license_tag not in header:
                errors.append(f"{relative}: missing SPDX license header")
        elif path.suffix in ANNOTATED_SUFFIXES and path.stat().st_size > 0:
            expected_annotations.add(relative)

    for relative in sorted(expected_annotations - annotated):
        errors.append(f"{relative}: missing REUSE.toml annotation")
    for relative in sorted(annotated - expected_annotations):
        errors.append(f"REUSE.toml: stale annotation for {relative}")
    return errors


def validate_license_contract(root: Path) -> list[str]:
    """Return deterministic license-contract violations under *root*."""
    errors = validate_file_spdx_coverage(root)
    projects = license_project_files(root)
    if not projects:
        return [f"{root}: no pyproject.toml files found"]

    for pyproject in projects:
        relative = pyproject.relative_to(root)
        project = tomllib.loads(pyproject.read_text(encoding="utf-8")).get("project")
        if not isinstance(project, dict):
            errors.append(f"{relative}: missing [project] table")
            continue

        if project.get("license") != LICENSE_EXPRESSION:
            errors.append(
                f"{relative}: project.license must equal {LICENSE_EXPRESSION!r}"
            )
        if project.get("license-files") != LICENSE_FILES:
            errors.append(f"{relative}: project.license-files must equal {LICENSE_FILES!r}")

        license_path = pyproject.parent / "LICENSE"
        if not license_path.is_file():
            errors.append(f"{license_path.relative_to(root)}: missing license declaration")
            continue
        lines = license_path.read_text(encoding="utf-8").splitlines()
        if SPDX_LINE not in lines[:3]:
            errors.append(f"{license_path.relative_to(root)}: invalid SPDX declaration")
        text = "\n".join(lines)
        if "The AND operator is intentional" not in text:
            errors.append(f"{license_path.relative_to(root)}: AND semantics are not explicit")
        for required_text in REQUIRED_LICENSE_TEXT:
            if required_text not in text:
                errors.append(
                    f"{license_path.relative_to(root)}: missing {required_text!r}"
                )

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Python workspace root containing pyproject.toml files",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()
    errors = validate_license_contract(root)
    if errors:
        for error in errors:
            sys.stderr.write(f"license-contract: {error}\n")
        return 1
    sys.stdout.write(
        f"license-contract: ok ({len(license_project_files(root))} projects, "
        f"{LICENSE_EXPRESSION})\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
