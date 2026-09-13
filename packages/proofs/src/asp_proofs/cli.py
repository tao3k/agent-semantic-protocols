# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Command adapter for the asp_proofs package."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from .bundle import ProofBundleAdmissionError, admit_proof_bundle_index
from .lean_audit import AuditAdmissionError, admit_audit
from .relationship_contract import (
    RelationshipContractVerificationError,
    verify_relationship_contract,
)
from .schema import ProofSchemaError

REPOSITORY_ROOT = Path(__file__).resolve().parents[4]
SCHEMAS = REPOSITORY_ROOT / "schemas"


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m asp_proofs")
    subcommands = parser.add_subparsers(dest="command", required=True)

    lean = subcommands.add_parser("lean-audit")
    lean.add_argument("audit", type=Path)
    lean.add_argument("--allow-axiom", action="append", default=[])
    lean.add_argument("--require-family", action="append", default=[])
    lean.add_argument("--require-rfc-clause", action="append", default=[])
    lean.add_argument("--output", type=Path)

    bundle = subcommands.add_parser("bundle-audit")
    bundle.add_argument("index", type=Path)
    bundle.add_argument("--output", type=Path)

    relationship = subcommands.add_parser("relationship-contract")
    relationship.add_argument("packet", type=Path)
    relationship.add_argument("--orgize", default="orgize")
    relationship.add_argument("--output", type=Path)
    return parser


def _render(receipt: dict[str, Any], output: Path | None) -> None:
    rendered = json.dumps(receipt, indent=2, sort_keys=True) + "\n"
    if output is None:
        sys.stdout.write(rendered)
    else:
        output.write_text(rendered)


def _run_lean(args: argparse.Namespace) -> dict[str, Any]:
    return admit_audit(
        args.audit,
        allowed_axioms=set(args.allow_axiom),
        required_families=set(args.require_family),
        required_rfc_clauses=set(args.require_rfc_clause),
    )


def _run_bundle(args: argparse.Namespace) -> dict[str, Any]:
    return admit_proof_bundle_index(
        args.index,
        REPOSITORY_ROOT,
        SCHEMAS / "lean-org-typst-proof-bundle-index.v1.schema.json",
        SCHEMAS / "axle-proof-bundle-audit-receipt.v1.schema.json",
    )


def _run_relationship_contract(args: argparse.Namespace) -> dict[str, Any]:
    return verify_relationship_contract(
        args.packet,
        REPOSITORY_ROOT,
        orgize=args.orgize,
    )


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "lean-audit":
            receipt = _run_lean(args)
        elif args.command == "bundle-audit":
            receipt = _run_bundle(args)
        else:
            receipt = _run_relationship_contract(args)
    except (
        AuditAdmissionError,
        ProofBundleAdmissionError,
        RelationshipContractVerificationError,
        ProofSchemaError,
        OSError,
        json.JSONDecodeError,
    ) as error:
        raise SystemExit(f"proof admission failed: {error}") from error
    _render(receipt, args.output)
    return 0
