# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Pure admission service for Lean, Org, and Typst proof bundles."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
from typing import Any

from .models import (
    AdmittedProofBundle,
    ProofBundleAuditReceipt,
    ProofBundleIndex,
    ProofBundleSpec,
)
from .schema import ProofSchemaError, validate_mapping


TYPST_BLOCK = re.compile(r"(?im)^#\+begin_src[ \t]+typst(?:[ \t]|$)")


class ProofBundleAdmissionError(ValueError):
    """Raised when a three-view proof bundle cannot be admitted."""


def _repository_path(
    repository_root: Path, relative_path: str, expected_suffix: str
) -> Path:
    candidate = (repository_root / relative_path).resolve()
    try:
        candidate.relative_to(repository_root.resolve())
    except ValueError as error:
        raise ProofBundleAdmissionError(f"path escapes repository: {relative_path}") from error
    if candidate.suffix != expected_suffix:
        raise ProofBundleAdmissionError(
            f"expected {expected_suffix} path, received: {relative_path}"
        )
    if not candidate.is_file():
        raise ProofBundleAdmissionError(f"missing proof bundle member: {relative_path}")
    return candidate


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _covered_lean_paths(
    repository_root: Path, enforced_lean_globs: tuple[str, ...]
) -> set[str]:
    covered: set[str] = set()
    for pattern in enforced_lean_globs:
        covered.update(
            path.relative_to(repository_root).as_posix()
            for path in repository_root.glob(pattern)
            if path.is_file()
        )
    return covered


def _validate_unique_ownership(index: ProofBundleIndex) -> None:
    lean_paths = [bundle.lean_path for bundle in index.bundles]
    if len(lean_paths) != len(set(lean_paths)):
        raise ProofBundleAdmissionError("one Lean proof is owned by multiple bundles")
    bundle_ids = [bundle.bundle_id for bundle in index.bundles]
    if len(bundle_ids) != len(set(bundle_ids)):
        raise ProofBundleAdmissionError("proof bundle identifiers must be unique")


def _validate_inventory(index: ProofBundleIndex, covered: set[str]) -> None:
    indexed = {bundle.lean_path for bundle in index.bundles}
    missing = sorted(covered - indexed)
    stale = sorted(indexed - covered)
    if missing:
        raise ProofBundleAdmissionError(
            "Lean proof lacks an Org and Typst bundle: " + ", ".join(missing)
        )
    if stale:
        raise ProofBundleAdmissionError(
            "bundle Lean path is outside the enforced inventory: " + ", ".join(stale)
        )


def _admit_bundle(
    repository_root: Path, bundle: ProofBundleSpec
) -> AdmittedProofBundle:
    lean = _repository_path(repository_root, bundle.lean_path, ".lean")
    org = _repository_path(repository_root, bundle.org_path, ".org")
    org_text = org.read_text()
    if bundle.lean_path not in org_text:
        raise ProofBundleAdmissionError(
            f"Org authority does not reference Lean proof: {bundle.bundle_id}"
        )
    typst_count = len(TYPST_BLOCK.findall(org_text))
    if typst_count < bundle.minimum_typst_blocks:
        raise ProofBundleAdmissionError(
            f"Org authority has {typst_count} Typst blocks; "
            f"{bundle.minimum_typst_blocks} required: {bundle.bundle_id}"
        )
    return AdmittedProofBundle(
        bundle_id=bundle.bundle_id,
        lean_path=bundle.lean_path,
        org_path=bundle.org_path,
        lean_sha256=_sha256(lean),
        org_sha256=_sha256(org),
        typst_block_count=typst_count,
    )


def admit_proof_bundle_index(
    index_path: Path,
    repository_root: Path,
    index_schema_path: Path,
    receipt_schema_path: Path,
) -> dict[str, Any]:
    """Admit a complete inventory and return a schema-valid receipt mapping."""

    index_bytes = index_path.read_bytes()
    raw_index = json.loads(index_bytes)
    if not isinstance(raw_index, dict):
        raise ProofSchemaError("proof bundle index must be a JSON object")
    validate_mapping(index_schema_path, raw_index)
    index = ProofBundleIndex.from_mapping(raw_index)
    _validate_unique_ownership(index)
    covered = _covered_lean_paths(repository_root, index.enforced_lean_globs)
    _validate_inventory(index, covered)
    receipt = ProofBundleAuditReceipt(
        index_sha256=hashlib.sha256(index_bytes).hexdigest(),
        covered_lean_count=len(covered),
        bundles=tuple(
            _admit_bundle(repository_root, bundle) for bundle in index.bundles
        ),
    ).to_mapping()
    validate_mapping(receipt_schema_path, receipt)
    return receipt
