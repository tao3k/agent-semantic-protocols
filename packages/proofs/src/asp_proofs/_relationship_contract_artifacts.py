# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Observe typed Relationship Contract artifacts."""

from __future__ import annotations

import json
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from ._relationship_contract_model import (
    RelationshipContractVerificationError,
    normalize_rfc_source,
    sha256_text,
)
from ._relationship_contract_projection import (
    query_rfc_section,
    resolve_rfc_source_path,
)


def _canonical_json(path: Path) -> str:
    try:
        value = json.loads(path.read_bytes())
    except json.JSONDecodeError as error:
        raise RelationshipContractVerificationError(
            f"artifact is not valid JSON: {path}"
        ) from error
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def _artifact_source(
    artifact: Mapping[str, Any], repository_root: Path, orgize: str | Path
) -> str:
    path = resolve_rfc_source_path(repository_root, str(artifact["artifactPath"]))
    canonicalization = artifact["canonicalization"]
    if canonicalization == "json-sort-keys-compact-utf8-v1":
        return _canonical_json(path)
    if canonicalization == "orgize-section-source-raw-lf-utf8-v1":
        selector = artifact.get("artifactSelector")
        if not isinstance(selector, str):
            raise RelationshipContractVerificationError(
                f"org-section artifact lacks selector: {artifact['artifactId']}"
            )
        outline_path = selector.split(" / ")
        return query_rfc_section(
            orgize, path, outline_path, str(artifact["artifactId"])
        )
    if canonicalization in {
        "org-source-raw-lf-utf8-v1",
        "source-raw-lf-utf8-v1",
    }:
        return normalize_rfc_source(path.read_text(encoding="utf-8"))
    raise RelationshipContractVerificationError(
        f"unsupported artifact canonicalization: {canonicalization}"
    )


def observe_artifacts(
    artifacts: Sequence[Mapping[str, Any]],
    repository_root: Path,
    orgize: str | Path,
) -> list[dict[str, Any]]:
    """Return one digest-equal observation for every declared artifact."""

    observations = []
    for artifact in sorted(artifacts, key=lambda item: str(item["artifactId"])):
        observed_sha256 = sha256_text(
            _artifact_source(artifact, repository_root, orgize)
        )
        if observed_sha256 != artifact["expectedSha256"]:
            raise RelationshipContractVerificationError(
                f"{artifact['artifactId']} expectedSha256 mismatch: expected "
                f"{artifact['expectedSha256']}, observed {observed_sha256}"
            )
        observations.append({**dict(artifact), "observedSha256": observed_sha256})
    return observations
