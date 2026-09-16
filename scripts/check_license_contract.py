#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
"""Validate the repository-wide SPDX, REUSE, Cargo, and Python license contract."""

from __future__ import annotations

import fnmatch
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
EXPRESSION = "Apache-2.0 AND LGPL-2.1-or-later"
COPYRIGHT = "2026 tao3k team and Contributors"
SPDX_LICENSE_LABEL = "SPDX-" + "License-Identifier"
SPDX_COPYRIGHT_LABEL = "SPDX-" + "FileCopyrightText"
LICENSE_TAG = f"{SPDX_LICENSE_LABEL}: {EXPRESSION}"
COPYRIGHT_TAG = f"{SPDX_COPYRIGHT_LABEL}: {COPYRIGHT}"
SIDECAR_FILES = (Path("REUSE.toml"), Path("packages/python/REUSE.toml"))


def git_output(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def submodule_paths() -> tuple[str, ...]:
    result: list[str] = []
    for line in git_output("ls-files", "--stage").splitlines():
        metadata, path = line.split("\t", 1)
        if metadata.split()[0] == "160000":
            result.append(path)
    return tuple(result)


def project_files() -> tuple[Path, ...]:
    submodules = submodule_paths()
    result: list[Path] = []
    for relative in git_output(
        "ls-files", "--cached", "--others", "--exclude-standard"
    ).splitlines():
        if any(relative == root or relative.startswith(root + "/") for root in submodules):
            continue
        path = ROOT / relative
        if path.is_file():
            result.append(path)
    return tuple(result)


def sidecar_patterns(errors: list[str]) -> tuple[str, ...]:
    patterns: list[str] = []
    for sidecar in SIDECAR_FILES:
        path = ROOT / sidecar
        if not path.is_file():
            errors.append(f"{sidecar}: missing REUSE annotation file")
            continue
        payload = tomllib.loads(path.read_text(encoding="utf-8"))
        if payload.get("version") != 1:
            errors.append(f"{sidecar}: version must equal 1")
        base = sidecar.parent.as_posix()
        for index, annotation in enumerate(payload.get("annotations", []), start=1):
            if annotation.get("SPDX-FileCopyrightText") != COPYRIGHT:
                errors.append(f"{sidecar}: annotation {index} has invalid copyright")
            if annotation.get("SPDX-License-Identifier") != EXPRESSION:
                errors.append(f"{sidecar}: annotation {index} has invalid license")
            values: Any = annotation.get("path", [])
            if isinstance(values, str):
                values = [values]
            if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
                errors.append(f"{sidecar}: annotation {index} has invalid paths")
                continue
            for value in values:
                patterns.append(value if base == "." else f"{base}/{value}")
    return tuple(patterns)


def validate_file_coverage(files: tuple[Path, ...], errors: list[str]) -> None:
    patterns = sidecar_patterns(errors)
    for path in files:
        relative = path.relative_to(ROOT).as_posix()
        try:
            header = path.read_text(encoding="utf-8")[:8192]
        except UnicodeDecodeError:
            header = ""
        inline = LICENSE_TAG in header and COPYRIGHT_TAG in header
        annotated = any(fnmatch.fnmatchcase(relative, pattern) for pattern in patterns)
        if not inline and not annotated:
            errors.append(f"{relative}: missing SPDX header or REUSE annotation")


def validate_license_texts(errors: list[str]) -> None:
    expected = {
        "LICENSES/Apache-2.0.txt": ("Apache License", "Version 2.0, January 2004"),
        "LICENSES/LGPL-2.1-or-later.txt": (
            "GNU LESSER GENERAL PUBLIC LICENSE",
            "Version 2.1, February 1999",
        ),
    }
    for relative, markers in expected.items():
        path = ROOT / relative
        if not path.is_file():
            errors.append(f"{relative}: missing canonical license text")
            continue
        text = path.read_text(encoding="utf-8")
        for marker in markers:
            if marker not in text:
                errors.append(f"{relative}: missing {marker!r}")
    combined = (ROOT / "LICENSE").read_text(encoding="utf-8")
    if LICENSE_TAG not in combined.splitlines()[:3]:
        errors.append("LICENSE: invalid SPDX expression")
    if "The AND operator is intentional" not in combined:
        errors.append("LICENSE: AND semantics are not explicit")


def validate_cargo_metadata(files: tuple[Path, ...], errors: list[str]) -> None:
    root_manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    workspace_license = root_manifest.get("workspace", {}).get("package", {}).get("license")
    if workspace_license != EXPRESSION:
        errors.append(f"Cargo.toml: workspace.package.license must equal {EXPRESSION!r}")

    for path in files:
        relative = path.relative_to(ROOT)
        scenario_data = "scenarios" in relative.parts and bool(
            {"inputs", "expected"} & set(relative.parts)
        )
        if path.name != "Cargo.toml" or "fixtures" in relative.parts or scenario_data:
            continue
        package = tomllib.loads(path.read_text(encoding="utf-8")).get("package")
        if isinstance(package, dict) and package.get("license") != {"workspace": True}:
            errors.append(f"{relative}: package.license must inherit from the workspace")


def validate_python_metadata(files: tuple[Path, ...], errors: list[str]) -> None:
    for path in files:
        relative = path.relative_to(ROOT)
        if path.name != "pyproject.toml" or "fixtures" in relative.parts:
            continue
        project = tomllib.loads(path.read_text(encoding="utf-8")).get("project")
        if not isinstance(project, dict):
            continue
        if project.get("license") != EXPRESSION:
            errors.append(f"{relative}: project.license must equal {EXPRESSION!r}")
        if project.get("license-files") != ["LICENSE"]:
            errors.append(f"{relative}: project.license-files must equal ['LICENSE']")


def main() -> int:
    errors: list[str] = []
    files = project_files()
    validate_file_coverage(files, errors)
    validate_license_texts(errors)
    validate_cargo_metadata(files, errors)
    validate_python_metadata(files, errors)
    if errors:
        for error in errors:
            sys.stderr.write(f"license-contract: {error}\n")
        return 1
    sys.stdout.write(
        f"license-contract: ok ({len(files)} project files, "
        f"{len(submodule_paths())} submodules excluded, {EXPRESSION})\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
