# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Project exact relationship artifacts through the Orgize parser."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Sequence
from pathlib import Path

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    normalize_rfc_source,
    require_mapping,
)


def resolve_rfc_source_path(repository_root: Path, source_path: str) -> Path:
    """Resolve a workspace-relative RFC source without allowing escape."""

    root = repository_root.resolve()
    candidate = (root / source_path).resolve()
    try:
        candidate.relative_to(root)
    except ValueError as error:
        raise RelationshipContractVerificationError(
            f"RFC source path escapes repository: {source_path}"
        ) from error
    if not candidate.is_file():
        raise RelationshipContractVerificationError(f"missing RFC source: {source_path}")
    return candidate


def _orgize_query(outline_path: Sequence[str]) -> str:
    query = {
        "schemaVersion": 1,
        "category": "section",
        "outlinePathPrefix": list(outline_path),
        "outlinePathExactLen": len(outline_path),
    }
    return json.dumps(
        query,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    )


def _run_orgize(
    orgize: str | Path,
    source_path: Path,
    outline_path: Sequence[str],
    clause_id: str,
) -> subprocess.CompletedProcess[str]:
    command = [
        str(orgize),
        "elements-query",
        "--packet",
        _orgize_query(outline_path),
        str(source_path),
    ]
    try:
        return subprocess.run(
            command,
            check=False,
            capture_output=True,
            encoding="utf-8",
        )
    except (OSError, UnicodeError) as error:
        raise RelationshipContractVerificationError(
            f"failed to execute orgize for {clause_id}: {error}"
        ) from error


def _decode_single_section(
    result: subprocess.CompletedProcess[str],
    outline_path: Sequence[str],
    clause_id: str,
) -> str:
    if result.returncode != 0:
        detail = result.stderr.strip() or f"exit status {result.returncode}"
        raise RelationshipContractVerificationError(
            f"orgize elements-query failed for {clause_id}: {detail}"
        )
    try:
        records = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RelationshipContractVerificationError(
            f"orgize returned invalid JSON for {clause_id}: {error}"
        ) from error
    if not isinstance(records, list) or len(records) != 1:
        count = len(records) if isinstance(records, list) else "non-array"
        raise RelationshipContractVerificationError(
            f"orgize must return exactly one section for {clause_id}, observed {count}"
        )
    record = require_mapping(records[0], f"orgize section for {clause_id}")
    if record.get("category") != "section":
        raise RelationshipContractVerificationError(
            f"orgize result for {clause_id} is not a section"
        )
    if record.get("outlinePath") != list(outline_path):
        raise RelationshipContractVerificationError(
            f"orgize result outlinePath mismatch for {clause_id}"
        )
    source = require_mapping(record.get("source"), f"orgize source for {clause_id}")
    raw = source.get("raw")
    if not isinstance(raw, str):
        raise RelationshipContractVerificationError(
            f"orgize source.raw for {clause_id} must be a string"
        )
    return normalize_rfc_source(raw)


def query_rfc_section(
    orgize: str | Path,
    source_path: Path,
    outline_path: Sequence[str],
    clause_id: str,
) -> str:
    """Return one exact LF-normalized parser-owned section."""

    result = _run_orgize(orgize, source_path, outline_path, clause_id)
    return _decode_single_section(result, outline_path, clause_id)
