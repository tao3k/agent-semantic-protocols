# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Strict identity validation for Search generation graph requests."""

from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any


def validate_identity(identity: Mapping[str, Any]) -> None:
    fields = {
        "projectId",
        "workspaceId",
        "sourceRootDigest",
        "providerDigest",
        "schemaDigest",
        "generationCandidateDigest",
    }
    exact_keys(identity, fields)
    for field in ("projectId", "workspaceId"):
        string(identity, field)
    for field in fields - {"projectId", "workspaceId"}:
        canonical_digest(identity.get(field))


def validate_source_snapshot(source_snapshot: Mapping[str, Any]) -> None:
    required = {
        "schemaId",
        "algorithm",
        "rootDigest",
        "sourceKind",
        "leafCount",
        "providerDigest",
    }
    exact_keys(source_snapshot, required | {"baseRootDigest", "dirtyPathsDigest"})
    if source_snapshot.get("schemaId") != "asp.source-snapshot.v1":
        raise ValueError("search generation graph source snapshot schema mismatch")
    if source_snapshot.get("algorithm") != "blake3-merkle-v1":
        raise ValueError("search generation graph source snapshot algorithm mismatch")
    if source_snapshot.get("sourceKind") not in {
        "filesystem",
        "editor-buffer",
        "git-tree",
        "derived-overlay",
    }:
        raise ValueError("search generation graph source kind mismatch")
    non_negative_integer(source_snapshot, "leafCount")
    for field in ("rootDigest", "providerDigest"):
        canonical_digest(source_snapshot.get(field))
    for field in ("baseRootDigest", "dirtyPathsDigest"):
        if field in source_snapshot:
            canonical_digest(source_snapshot.get(field))
    if ("baseRootDigest" in source_snapshot) != ("dirtyPathsDigest" in source_snapshot):
        raise ValueError("search generation graph overlay evidence is incomplete")


def validate_workspace_generation(workspace_generation: Mapping[str, Any]) -> None:
    exact_keys(
        workspace_generation,
        {"rootDigest", "rootDepth", "leafCount", "ownerCount"},
    )
    canonical_digest(workspace_generation.get("rootDigest"))
    for field in ("rootDepth", "leafCount", "ownerCount"):
        non_negative_integer(workspace_generation, field)
    if workspace_generation.get("rootDepth") not in {0, 1}:
        raise ValueError("search generation graph root depth mismatch")
    if workspace_generation.get("leafCount") == 0 or workspace_generation.get("ownerCount") == 0:
        raise ValueError("search generation graph workspace coverage is empty")


def canonical_json(value: object) -> str:
    if not isinstance(value, Mapping):
        raise ValueError("search generation graph relation must be an object")
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def mapping(payload: Mapping[str, Any], field: str) -> Mapping[str, Any]:
    value = payload.get(field)
    if not isinstance(value, Mapping):
        raise ValueError(f"{field} must be an object")
    return value


def string(payload: Mapping[str, Any], field: str) -> str:
    value = payload.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{field} must be a non-empty string")
    return value


def canonical_strings(payload: Mapping[str, Any], field: str) -> list[str]:
    value = payload.get(field)
    if not isinstance(value, list) or not value or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{field} must be a non-empty string array")
    if value != sorted(set(value)):
        raise ValueError(f"{field} must be a canonical set")
    return list(value)


def canonical_digest(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("source root digest must be a string")
    digest = value.removeprefix("blake3-256:").removeprefix("blake3:")
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise ValueError("source root digest is not BLAKE3")
    return f"blake3-256:{digest}"


def exact_keys(payload: Mapping[str, Any], allowed: set[str]) -> None:
    unknown = sorted(set(payload) - allowed)
    if unknown:
        raise ValueError(f"unsupported fields: {unknown}")


def non_negative_integer(payload: Mapping[str, Any], field: str) -> int:
    value = payload.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{field} must be a non-negative integer")
    return value
