# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Test fail-closed Relationship Contract dependency validation."""

from __future__ import annotations

from asp_proofs.relationship_contract import (
    RelationshipContractVerificationError,
    verify_relationship_contract,
)

from ._digests import dependency_digest
from ._support import (
    CLAUSE_A,
    CLAUSE_B,
    RelationshipContractFixture,
)


class RelationshipContractDependencyTest(RelationshipContractFixture):
    """Reject missing, stale, transitive, and cyclic dependency evidence."""

    def test_rejects_missing_dependency(self) -> None:
        raw = f"** Cost vector\n={CLAUSE_A}=\n"
        commitment = self.commitment(
            CLAUSE_A,
            "docs/rfc.org",
            ["Contract", "Cost vector"],
            raw,
            depends_on=[CLAUSE_B],
        )
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "missing dependencies"
        ):
            verify_relationship_contract(
                self.write_packet([commitment]), self.root, orgize=self.orgize
            )

    def test_rejects_dependency_digest_mismatch(self) -> None:
        raw_a = f"** Cost vector\n={CLAUSE_A}=\n"
        raw_b = f"** Objective order\n={CLAUSE_B}=\n"
        commitment_a = self.commitment(
            CLAUSE_A, "docs/rfc.org", ["Contract", "Cost vector"], raw_a
        )
        commitment_b = self.commitment(
            CLAUSE_B,
            "docs/rfc.org",
            ["Contract", "Objective order"],
            raw_b,
            depends_on=[CLAUSE_A],
            dependency_set_sha256="0" * 64,
        )
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "dependencySetSha256 mismatch"
        ):
            verify_relationship_contract(
                self.write_packet([commitment_a, commitment_b]),
                self.root,
                orgize=self.orgize,
            )

    def test_transitive_dependency_change_reaches_second_hop(self) -> None:
        clause_c = "ASP-RFC-10.05.64-BOUNDED-RADIX"
        commitment_a = self.commitment(
            CLAUSE_A, "docs/rfc.org", ["Contract", "Cost vector"], "a"
        )
        commitment_b = self.commitment(
            CLAUSE_B,
            "docs/rfc.org",
            ["Contract", "Objective order"],
            "b",
            depends_on=[CLAUSE_A],
            dependency_set_sha256=dependency_digest(commitment_a),
        )
        commitment_c = self.commitment(
            clause_c,
            "docs/rfc.org",
            ["Contract", "Bounded scalarization"],
            "c",
            depends_on=[CLAUSE_B],
            dependency_set_sha256=dependency_digest(commitment_b),
        )
        commitment_a["sourceSha256"] = "1" * 64
        commitment_b["dependencySetSha256"] = dependency_digest(commitment_a)

        with self.assertRaisesRegex(
            RelationshipContractVerificationError,
            f"{clause_c} dependencySetSha256 mismatch",
        ):
            verify_relationship_contract(
                self.write_packet([commitment_a, commitment_b, commitment_c]),
                self.root,
                orgize=self.orgize,
            )

    def test_rejects_dependency_cycle(self) -> None:
        commitment_a = self.commitment(
            CLAUSE_A,
            "docs/rfc.org",
            ["Contract", "Cost vector"],
            "a",
            depends_on=[CLAUSE_B],
        )
        commitment_b = self.commitment(
            CLAUSE_B,
            "docs/rfc.org",
            ["Contract", "Objective order"],
            "b",
            depends_on=[CLAUSE_A],
        )
        commitment_a["dependencySetSha256"] = dependency_digest(commitment_b)
        commitment_b["dependencySetSha256"] = dependency_digest(commitment_a)

        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "dependency cycle"
        ):
            verify_relationship_contract(
                self.write_packet([commitment_a, commitment_b]),
                self.root,
                orgize=self.orgize,
            )
