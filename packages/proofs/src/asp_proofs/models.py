"""Immutable domain values for proof admission."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Mapping


@dataclass(frozen=True, slots=True)
class ProofBundleSpec:
    """One Lean proof and its authoritative Org mathematical view."""

    bundle_id: str
    lean_path: str
    org_path: str
    minimum_typst_blocks: int

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> ProofBundleSpec:
        return cls(
            bundle_id=str(value["bundleId"]),
            lean_path=str(value["leanPath"]),
            org_path=str(value["orgPath"]),
            minimum_typst_blocks=int(value["minimumTypstBlocks"]),
        )


@dataclass(frozen=True, slots=True)
class ProofBundleIndex:
    """Typed inventory that makes unbundled Lean proofs observable."""

    enforced_lean_globs: tuple[str, ...]
    bundles: tuple[ProofBundleSpec, ...]

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> ProofBundleIndex:
        return cls(
            enforced_lean_globs=tuple(value["enforcedLeanGlobs"]),
            bundles=tuple(
                ProofBundleSpec.from_mapping(bundle)
                for bundle in value["bundles"]
            ),
        )


@dataclass(frozen=True, slots=True)
class AdmittedProofBundle:
    """Digest-bound evidence for one admitted three-view bundle."""

    bundle_id: str
    lean_path: str
    org_path: str
    lean_sha256: str
    org_sha256: str
    typst_block_count: int

    def to_mapping(self) -> dict[str, Any]:
        return {
            "bundleId": self.bundle_id,
            "leanPath": self.lean_path,
            "orgPath": self.org_path,
            "leanSha256": self.lean_sha256,
            "orgSha256": self.org_sha256,
            "typstBlockCount": self.typst_block_count,
        }


@dataclass(frozen=True, slots=True)
class ProofBundleAuditReceipt:
    """Complete admission result for an enforced proof inventory."""

    index_sha256: str
    covered_lean_count: int
    bundles: tuple[AdmittedProofBundle, ...]

    def to_mapping(self) -> dict[str, Any]:
        return {
            "schemaId": "asp.axle-proof-bundle-audit-receipt.v1",
            "schemaVersion": "1",
            "admissionLevel": "structural",
            "orgizeLifecycleState": "command-executor-unavailable",
            "indexSha256": self.index_sha256,
            "bundleCount": len(self.bundles),
            "coveredLeanCount": self.covered_lean_count,
            "bundles": [bundle.to_mapping() for bundle in self.bundles],
        }
