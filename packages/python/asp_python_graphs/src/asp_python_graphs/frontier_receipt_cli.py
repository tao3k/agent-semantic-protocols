"""CLI for semantic fact frontier receipt capture."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from .cli import _rank_packet
from .frontier_receipt import (
    FrontierCodeRead,
    FrontierTestCommand,
    FrontierTestResult,
    frontier_receipt_from_result,
)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    packet = _load_packet(args.packet)
    result = _rank_packet(packet, args)
    receipt = frontier_receipt_from_result(
        result,
        receipt_id=args.receipt_id,
        receipt_kind=args.receipt_kind,
        task_fingerprint=args.task_fingerprint,
        command_fingerprint=args.command_fingerprint,
        followed_node_ids=args.follow_node,
        code_reads=_code_reads(args),
        test_command=_test_command(args),
        test_result=_test_result(args),
        edit_touched_owner=tuple(args.edit_owner),
        output_fingerprint=args.output_fingerprint,
        commands_to_first_useful_locator=args.commands_to_first_useful_locator,
        commands_to_validation=args.commands_to_validation,
        fields=_fields(args),
    )
    sys.stdout.write(json.dumps(receipt, sort_keys=True) + "\n")
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("packet", nargs="?", default="-")
    parser.add_argument("--profile", default=None)
    parser.add_argument("--seed", action="append", default=[])
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--receipt-id", required=True)
    parser.add_argument("--receipt-kind", default="frontier")
    parser.add_argument("--task-fingerprint", required=True)
    parser.add_argument("--command-fingerprint", required=True)
    parser.add_argument("--follow-node", action="append", default=[])
    parser.add_argument("--read-selector", action="append", default=[])
    parser.add_argument("--read-kind", default="exact-selector")
    parser.add_argument("--read-owner", default=None)
    parser.add_argument("--test-argv-json", default=None)
    parser.add_argument("--test-status", default=None)
    parser.add_argument("--test-summary", default=None)
    parser.add_argument("--test-exit-code", type=int, default=None)
    parser.add_argument("--test-workdir", default=None)
    parser.add_argument("--test-fingerprint", default=None)
    parser.add_argument("--edit-owner", action="append", default=[])
    parser.add_argument("--output-fingerprint", default=None)
    parser.add_argument("--commands-to-first-useful-locator", type=int, default=None)
    parser.add_argument("--commands-to-validation", type=int, default=None)
    parser.add_argument("--field", action="append", default=[])
    return parser.parse_args(argv)


def _load_packet(path: str) -> Mapping[str, object]:
    if path == "-":
        packet = json.load(sys.stdin)
    else:
        packet = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(packet, Mapping):
        raise SystemExit("graph turbo packet must be a JSON object")
    return packet


def _code_reads(args: argparse.Namespace) -> tuple[FrontierCodeRead, ...]:
    return tuple(
        FrontierCodeRead(
            selector=selector,
            read_kind=args.read_kind,
            owner=args.read_owner,
        )
        for selector in args.read_selector
    )


def _test_command(args: argparse.Namespace) -> FrontierTestCommand | None:
    if args.test_argv_json is None:
        return None
    argv = json.loads(args.test_argv_json)
    if not isinstance(argv, list) or not all(isinstance(item, str) for item in argv):
        raise SystemExit("--test-argv-json must be a JSON string array")
    return FrontierTestCommand(
        argv=tuple(argv),
        workdir=args.test_workdir,
        fingerprint=args.test_fingerprint,
    )


def _test_result(args: argparse.Namespace) -> FrontierTestResult | None:
    if args.test_status is None and args.test_summary is None:
        return None
    if args.test_status is None or args.test_summary is None:
        raise SystemExit("--test-status and --test-summary must be passed together")
    return FrontierTestResult(
        status=args.test_status,
        summary=args.test_summary,
        exit_code=args.test_exit_code,
    )


def _fields(args: argparse.Namespace) -> dict[str, object]:
    fields: dict[str, object] = {}
    for item in args.field:
        key, separator, value = item.partition("=")
        if not separator or not key:
            raise SystemExit("--field values must use key=value")
        fields[key] = _field_value(value)
    return fields


def _field_value(value: str) -> Any:
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return value


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
