# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Stable public facade for polyglot search conformance checks."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence

from asp_proofs._polyglot_search_conformance_document import validate_search_document
from asp_proofs._polyglot_search_conformance_model import (
    RECEIPT_SCHEMA_ID,
    RELATION_ID_PATTERN,
    ConformanceReceipt,
    Violation,
    canonical_json,
    digest_bytes,
    digest_json,
)
from asp_proofs._polyglot_search_conformance_replacement import (
    validate_replacement_certificate,
)
from asp_proofs._polyglot_search_conformance_validation import (
    validate_progressive_turn,
    validate_relation_batch,
)

__all__ = [
    "RECEIPT_SCHEMA_ID",
    "RELATION_ID_PATTERN",
    "ConformanceReceipt",
    "Violation",
    "build_parser",
    "canonical_json",
    "digest_bytes",
    "digest_json",
    "main",
    "validate_progressive_turn",
    "validate_relation_batch",
    "validate_replacement_certificate",
    "validate_search_document",
]


def build_parser() -> argparse.ArgumentParser:
    from asp_proofs._polyglot_search_conformance_cli import build_parser as cli_parser

    return cli_parser()


def main(argv: Sequence[str] | None = None) -> int:
    from asp_proofs._polyglot_search_conformance_cli import main as cli_main

    return cli_main(argv)


if __name__ == "__main__":
    sys.exit(main())
