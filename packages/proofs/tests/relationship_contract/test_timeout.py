from __future__ import annotations

import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

from asp_proofs._relationship_contract_authority import _query_node_properties
from asp_proofs._relationship_contract_model import (
    RelationshipContractVerificationError,
)
from asp_proofs._relationship_contract_org import _run_contract_trace


class RelationshipContractTimeoutTest(unittest.TestCase):
    def test_authority_query_timeout_denies_admission(self) -> None:
        with patch(
            "asp_proofs._relationship_contract_authority.subprocess.run",
            side_effect=subprocess.TimeoutExpired(["orgize", "elements-query"], 30),
        ), self.assertRaisesRegex(
            RelationshipContractVerificationError,
            "authority node-property query timed out after 30 seconds",
        ):
            _query_node_properties("orgize", Path("authority"))

    def test_org_contract_trace_timeout_denies_admission(self) -> None:
        with patch(
            "asp_proofs._relationship_contract_org.subprocess.run",
            side_effect=subprocess.TimeoutExpired(["orgize", "contract", "trace"], 30),
        ), self.assertRaisesRegex(
            RelationshipContractVerificationError,
            "Org contract trace timed out for contract.v1 after 30 seconds",
        ):
            _run_contract_trace(
                "orgize",
                Path("contract"),
                Path("target"),
                "contract.v1",
            )


if __name__ == "__main__":
    unittest.main()
