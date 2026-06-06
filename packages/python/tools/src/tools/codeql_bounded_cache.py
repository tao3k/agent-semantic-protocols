"""Cache keys for bounded CodeQL fixture databases."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


def codeql_fixture_cache_key(
    *,
    source_root: Path,
    query_file: Path,
    codeql_language: str,
    version_payload: dict[str, Any],
) -> str:
    payload = {
        "codeqlLanguage": codeql_language,
        "codeqlSha": str(version_payload.get("sha", "unknown")),
        "codeqlVersion": str(version_payload.get("version", "unknown")),
        "queryText": query_file.read_text(encoding="utf-8"),
        "sourceDigest": source_digest(source_root),
    }
    return _fingerprint(payload)


def source_digest(source_root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(path for path in source_root.rglob("*") if path.is_file()):
        if "target" in path.parts:
            continue
        digest.update(path.relative_to(source_root).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def _fingerprint(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()
