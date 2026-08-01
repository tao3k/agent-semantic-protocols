"""Test parser-owned relationship projection and CLI rendering."""

from __future__ import annotations

import json
from contextlib import redirect_stdout
from io import StringIO
from unittest import mock

from asp_proofs import cli
from asp_proofs.relationship_contract import (
    RelationshipContractVerificationError,
    verify_relationship_contract,
)

from ._support import (
    CLAUSE_A,
    CLAUSE_B,
    RECEIPT_CHAIN_ID,
    RelationshipContractFixture,
    dependency_digest,
)


class RelationshipContractSourceTest(RelationshipContractFixture):
    """Verify exact sections, normalization, and receipt output."""

    def test_verifies_source_and_dependency_commitments(self) -> None:
        raw_a = f"** Cost vector\r\n\r\n={CLAUSE_A}=\r\nBody.\r\n"
        raw_b = f"** Objective order\r\r={CLAUSE_B}=\rBody.\r"
        self.write_source(
            "docs/rfc.org",
            ["Contract", "Cost vector"],
            raw_a,
            records=[
                {
                    "category": "section",
                    "outlinePath": ["Contract", "Cost vector"],
                    "source": {"raw": raw_a},
                },
                {
                    "category": "section",
                    "outlinePath": ["Contract", "Objective order"],
                    "source": {"raw": raw_b},
                },
            ],
        )
        commitment_a = self.commitment(
            CLAUSE_A, "docs/rfc.org", ["Contract", "Cost vector"], raw_a
        )
        commitment_b = self.commitment(
            CLAUSE_B,
            "docs/rfc.org",
            ["Contract", "Objective order"],
            raw_b,
            depends_on=[CLAUSE_A],
            dependency_set_sha256=dependency_digest(commitment_a),
        )

        receipt = verify_relationship_contract(
            self.write_packet([commitment_b, commitment_a]),
            self.root,
            orgize=self.orgize,
        )

        profile = receipt["applicationProfiles"]["sectionCommitments"]
        self.assertEqual(profile["commitmentCount"], 2)
        self.assertEqual(receipt["receiptChainId"], RECEIPT_CHAIN_ID)
        self.assertEqual(
            [item["clauseId"] for item in profile["commitments"]],
            [CLAUSE_A, CLAUSE_B],
        )

    def test_cli_renders_receipt_and_accepts_orgize_override(self) -> None:
        raw = f"** Cost vector\n={CLAUSE_A}=\n"
        self.write_source("docs/rfc.org", ["Contract", "Cost vector"], raw)
        packet = self.write_packet(
            [
                self.commitment(
                    CLAUSE_A,
                    "docs/rfc.org",
                    ["Contract", "Cost vector"],
                    raw,
                )
            ]
        )
        stdout = StringIO()
        with (
            mock.patch.object(cli, "REPOSITORY_ROOT", self.root),
            redirect_stdout(stdout),
        ):
            result = cli.main(
                ["relationship-contract", str(packet), "--orgize", str(self.orgize)]
            )

        self.assertEqual(result, 0)
        self.assertEqual(
            json.loads(stdout.getvalue())["receiptChainId"], RECEIPT_CHAIN_ID
        )

    def test_rejects_source_digest_mismatch(self) -> None:
        raw = f"** Cost vector\n={CLAUSE_A}=\n"
        self.write_source("docs/rfc.org", ["Contract", "Cost vector"], raw)
        commitment = self.commitment(
            CLAUSE_A, "docs/rfc.org", ["Contract", "Cost vector"], raw
        )
        commitment["sourceSha256"] = "0" * 64

        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "sourceSha256 mismatch"
        ):
            verify_relationship_contract(
                self.write_packet([commitment]), self.root, orgize=self.orgize
            )

    def test_rejects_non_unique_section_or_failed_org_contract(self) -> None:
        raw = "** Cost vector\nmarker omitted\n"
        outline = ["Contract", "Cost vector"]
        commitment = self.commitment(CLAUSE_A, "docs/rfc.org", outline, raw)
        record = {
            "category": "section",
            "outlinePath": outline,
            "source": {"raw": raw},
        }
        self.write_source("docs/rfc.org", outline, raw, records=[record, record])
        packet = self.write_packet([commitment])
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "exactly one section"
        ):
            verify_relationship_contract(packet, self.root, orgize=self.orgize)

        self.write_source("docs/rfc.org", outline, raw, contract_status="failed")
        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "Org contract assertions failed"
        ):
            verify_relationship_contract(packet, self.root, orgize=self.orgize)

    def test_rejects_changed_org_contract_source(self) -> None:
        raw = f"** Cost vector\n={CLAUSE_A}=\n"
        outline = ["Contract", "Cost vector"]
        self.write_source("docs/rfc.org", outline, raw)
        commitment = self.commitment(CLAUSE_A, "docs/rfc.org", outline, raw)
        packet = self.write_packet([commitment])
        self.contract_path.write_text("* changed contract semantics\n")

        with self.assertRaisesRegex(
            RelationshipContractVerificationError, "contractSha256 mismatch"
        ):
            verify_relationship_contract(packet, self.root, orgize=self.orgize)
