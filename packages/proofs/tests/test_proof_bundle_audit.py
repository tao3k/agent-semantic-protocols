"""Verify fail-closed Lean, Org, and Typst proof-bundle admission."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from asp_proofs import (
    ProofBundleAdmissionError,
    admit_proof_bundle_index,
)


LEAN_PATH = "packages/proofs/ASPProof/Example.lean"
ORG_PATH = "docs/10-19-rfcs/example.org"
REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
INDEX_SCHEMA = (
    REPOSITORY_ROOT / "schemas/lean-org-typst-proof-bundle-index.v1.schema.json"
)
RECEIPT_SCHEMA = (
    REPOSITORY_ROOT / "schemas/axle-proof-bundle-audit-receipt.v1.schema.json"
)


class ProofBundleAuditTest(unittest.TestCase):
    def write_fixture(
        self,
        root: Path,
        *,
        org_text: str | None = None,
        include_bundle: bool = True,
    ) -> Path:
        lean = root / LEAN_PATH
        lean.parent.mkdir(parents=True)
        lean.write_text("theorem example : True := by trivial\n")
        org = root / ORG_PATH
        org.parent.mkdir(parents=True)
        org.write_text(
            org_text
            if org_text is not None
            else (
                f"* Formal model\nLean source: ={LEAN_PATH}=\n"
                "#+begin_src typst\n#assert(true)\n#+end_src\n"
            )
        )
        bundles = []
        if include_bundle:
            bundles.append(
                {
                    "bundleId": "example-proof",
                    "leanPath": LEAN_PATH,
                    "orgPath": ORG_PATH,
                    "minimumTypstBlocks": 1,
                }
            )
        index = {
            "schemaId": "asp.lean-org-typst-proof-bundle-index.v1",
            "schemaVersion": "1",
            "enforcedLeanGlobs": [
                "packages/proofs/ASPProof/Example*.lean"
            ],
            "bundles": bundles,
        }
        index_path = root / "index.json"
        index_path.write_text(json.dumps(index))
        return index_path

    def test_three_view_bundle_is_admitted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = admit_proof_bundle_index(
                self.write_fixture(root), root, INDEX_SCHEMA, RECEIPT_SCHEMA
            )

        self.assertEqual(receipt["bundleCount"], 1)
        self.assertEqual(receipt["coveredLeanCount"], 1)
        self.assertEqual(receipt["bundles"][0]["typstBlockCount"], 1)

    def test_lean_without_bundle_is_blocked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            index = self.write_fixture(root, include_bundle=False)
            with self.assertRaises(ProofBundleAdmissionError):
                admit_proof_bundle_index(index, root, INDEX_SCHEMA, RECEIPT_SCHEMA)

    def test_org_without_exact_lean_reference_is_blocked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            index = self.write_fixture(
                root,
                org_text="#+begin_src typst\n#assert(true)\n#+end_src\n",
            )
            with self.assertRaises(ProofBundleAdmissionError):
                admit_proof_bundle_index(index, root, INDEX_SCHEMA, RECEIPT_SCHEMA)

    def test_org_without_typst_is_blocked(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            index = self.write_fixture(
                root,
                org_text=f"Lean source: ={LEAN_PATH}=\n",
            )
            with self.assertRaises(ProofBundleAdmissionError):
                admit_proof_bundle_index(index, root, INDEX_SCHEMA, RECEIPT_SCHEMA)


if __name__ == "__main__":
    unittest.main()
