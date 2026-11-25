#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Zero-dependency stdlib helper to materialize external repos declared in providers.toml.

ASP's Rust workspace is a pure-Rust library workspace (zero standalone CLI binaries);
library consumers embed `agent_semantic_schema_manager::provider_workspace` directly,
while CI and `Justfile` invoke this lightweight stdlib helper without compiling Rust.
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Callable

try:
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # Python < 3.11 (e.g. macOS system python3 3.9)
    tomllib = None


def emit_line(message: str) -> None:
    sys.stdout.write(f"{message}\n")


def _parse_providers_toml_fallback(text: str) -> dict:
    """Minimal parser for providers.toml on Python 3.9/3.10 without tomllib."""
    data: dict = {"profiles": {}, "packages": {}}
    section: tuple[str, str | None] = ("root", None)
    active_list_key: str | None = None
    for raw_line in text.splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if active_list_key is not None:
            data["profiles"][active_list_key].extend(re.findall(r'"([^"]+)"', line))
            if "]" in line:
                active_list_key = None
            continue
        if line == "[profiles]":
            section = ("profiles", None)
            continue
        pkg_match = re.fullmatch(r"\[packages\.([A-Za-z0-9_.-]+)\]", line)
        if pkg_match:
            pkg_id = pkg_match.group(1)
            data["packages"][pkg_id] = {}
            section = ("package", pkg_id)
            continue
        if "=" not in line:
            continue
        key, raw_val = [part.strip() for part in line.split("=", 1)]
        if section[0] == "root" and key == "schema_version":
            data["schema_version"] = int(raw_val)
        elif section[0] == "profiles":
            data["profiles"][key] = re.findall(r'"([^"]+)"', raw_val)
            if "[" in raw_val and "]" not in raw_val:
                active_list_key = key
        elif section[0] == "package" and section[1] is not None:
            str_match = re.fullmatch(r'"([^"]*)"', raw_val)
            if str_match:
                data["packages"][section[1]][key] = str_match.group(1)
    return data


def run_git(cwd: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args],
        cwd=cwd,
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed in {cwd}: {proc.stderr.strip()}"
        )
    return proc.stdout.strip()


def load_manifest(workspace: Path) -> dict:
    manifest_path = workspace / "providers.toml"
    if tomllib is not None:
        with manifest_path.open("rb") as fh:
            data = tomllib.load(fh)
    else:
        data = _parse_providers_toml_fallback(manifest_path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise RuntimeError(
            f"unsupported providers.toml schema_version: {data.get('schema_version')}"
        )
    return data


def select_packages(
    manifest: dict,
    profile: str | None,
    packages: list[str],
) -> list[tuple[str, dict]]:
    all_packages: dict[str, dict] = manifest.get("packages", {})
    profiles: dict[str, list[str]] = manifest.get("profiles", {})
    if packages:
        ids = packages
    elif profile is None or profile == "all":
        ids = sorted(all_packages.keys())
    else:
        if profile not in profiles:
            raise RuntimeError(f"unknown providers.toml profile: {profile}")
        ids = profiles[profile]
    selected: list[tuple[str, dict]] = []
    for pkg_id in ids:
        if pkg_id not in all_packages:
            raise RuntimeError(f"unknown provider package: {pkg_id}")
        selected.append((pkg_id, all_packages[pkg_id]))
    return selected


def inspect_checkout(target: Path, locked_rev: str) -> tuple[str, str, bool]:
    if not target.exists() and not target.is_symlink():
        return ("missing", "-", False)
    is_symlink = target.is_symlink()
    try:
        actual = run_git(target, "rev-parse", "HEAD")
    except RuntimeError:
        actual = "unknown"
    try:
        dirty = bool(run_git(target, "status", "--porcelain"))
    except RuntimeError:
        dirty = False
    if is_symlink:
        state = "symlinked"
    elif actual == locked_rev:
        state = "synced"
    else:
        state = "drifted"
    return (state, actual, dirty)


def cmd_status(workspace: Path, manifest: dict, args: argparse.Namespace) -> None:
    for pkg_id, spec in select_packages(manifest, args.profile, args.package):
        state, actual, dirty = inspect_checkout(workspace / spec["path"], spec["rev"])
        emit_line(
            f"[provider-pkg] id={pkg_id} state={state} locked={spec['rev']} "
            f"actual={actual} dirty={str(dirty).lower()} path={spec['path']}"
        )


def cmd_sync(workspace: Path, manifest: dict, args: argparse.Namespace) -> None:
    for pkg_id, spec in select_packages(manifest, args.profile or "core", args.package):
        target = workspace / spec["path"]
        if target.is_symlink():
            emit_line(f"[provider-pkg] id={pkg_id} state=symlinked-skipped path={spec['path']}")
            continue
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            run_git(
                workspace,
                "clone",
                "--no-checkout",
                "--filter=blob:none",
                spec["repo"],
                spec["path"],
            )
        state, _, dirty = inspect_checkout(target, spec["rev"])
        if state == "synced":
            emit_line(f"[provider-pkg] id={pkg_id} state=synced rev={spec['rev']}")
            continue
        if dirty:
            raise RuntimeError(
                f"refusing to overwrite dirty checkout for `{pkg_id}` at {target}"
            )
        run_git(target, "fetch", "--depth", "1", "origin", spec["rev"])
        run_git(target, "checkout", "--detach", spec["rev"])
        emit_line(f"[provider-pkg] id={pkg_id} state=updated rev={spec['rev']}")


def cmd_lock(workspace: Path, manifest: dict, args: argparse.Namespace) -> None:
    manifest_path = workspace / "providers.toml"
    text = manifest_path.read_text(encoding="utf-8")
    for pkg_id, spec in select_packages(manifest, args.profile or "core", args.package):
        target = workspace / spec["path"]
        if not target.exists():
            raise RuntimeError(f"cannot lock `{pkg_id}`: missing checkout at {target}")
        head = run_git(target, "rev-parse", "HEAD")
        if head != spec["rev"]:
            old_line = f'rev = "{spec["rev"]}"'
            new_line = f'rev = "{head}"'
            text = text.replace(old_line, new_line, 1)
            emit_line(f"[provider-pkg] id={pkg_id} locked={head}")
    manifest_path.write_text(text, encoding="utf-8")


def resolve_ghq_source(ghq_root: Path, repo_url: str) -> Path:
    for prefix in ("https://", "ssh://git@"):
        if repo_url.startswith(prefix):
            trimmed = repo_url[len(prefix) :].removesuffix(".git")
            return ghq_root / trimmed
    raise RuntimeError(f"unsupported repo URL for ghq resolution: {repo_url}")


def cmd_link_local(workspace: Path, manifest: dict, args: argparse.Namespace) -> None:
    if not args.ghq_root:
        raise RuntimeError("link-local requires --ghq-root <DIR>")
    ghq_root = Path(args.ghq_root).expanduser().resolve()
    for pkg_id, spec in select_packages(manifest, args.profile or "core", args.package):
        source = resolve_ghq_source(ghq_root, spec["repo"])
        if not source.exists():
            raise RuntimeError(f"local ghq checkout not found for `{pkg_id}` at {source}")
        target = workspace / spec["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.is_symlink() or target.is_file():
            target.unlink()
        elif target.exists():
            shutil.rmtree(target)
        os.symlink(source, target)
        emit_line(f"[provider-pkg] id={pkg_id} linked={target} -> {source}")


ACTION_HANDLERS: dict[str, Callable[[Path, dict, argparse.Namespace], None]] = {
    "status": cmd_status,
    "sync": cmd_sync,
    "lock": cmd_lock,
    "link-local": cmd_link_local,
}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Manage ASP providers.toml external checkouts.")
    parser.add_argument("action", choices=sorted(ACTION_HANDLERS.keys()))
    parser.add_argument("--workspace", default=".")
    parser.add_argument("--profile", default=None)
    parser.add_argument("--package", action="append", default=[])
    parser.add_argument("--ghq-root", default=None)
    args = parser.parse_args(argv)

    workspace = Path(args.workspace).resolve()
    manifest = load_manifest(workspace)
    ACTION_HANDLERS[args.action](workspace, manifest, args)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except RuntimeError as error:
        sys.stderr.write(f"error: {error}\n")
        raise SystemExit(1) from error
