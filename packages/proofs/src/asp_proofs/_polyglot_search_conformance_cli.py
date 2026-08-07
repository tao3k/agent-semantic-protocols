"""Own command-line presentation for polyglot search conformance receipts."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Sequence

from asp_proofs._cli_output import write_stdout
from asp_proofs._polyglot_search_conformance_model import canonical_json
from asp_proofs.polyglot_search_conformance import (
    validate_progressive_turn,
    validate_relation_batch,
    validate_replacement_certificate,
    validate_search_document,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="asp-proofs-polyglot-conformance")
    parser.add_argument(
        "--kind",
        required=True,
        choices=("document", "relation-batch", "turn", "replacement"),
    )
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--state-digest", required=True)
    parser.add_argument("--registered-predicate", action="append", default=[])
    parser.add_argument("--expected-snapshot-digest")
    parser.add_argument("--expected-provider-digest")
    parser.add_argument("--expected-semantic-digest")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.kind == "document":
        receipt = validate_search_document(
            args.input.read_text(encoding="utf-8"),
            registered_predicates=args.registered_predicate,
            state_digest=args.state_digest,
        )
    else:
        packet = json.loads(args.input.read_text(encoding="utf-8"))
        if args.kind == "relation-batch":
            receipt = validate_relation_batch(
                packet,
                expected_snapshot_digest=args.expected_snapshot_digest or "",
                expected_provider_digest=args.expected_provider_digest or "",
                state_digest=args.state_digest,
            )
        elif args.kind == "turn":
            receipt = validate_progressive_turn(
                packet,
                expected_semantic_digest=args.expected_semantic_digest or "",
                state_digest=args.state_digest,
            )
        else:
            receipt = validate_replacement_certificate(
                packet,
                state_digest=args.state_digest,
            )
    write_stdout(canonical_json(receipt.to_dict()))
    return 0 if receipt.admitted else 1
