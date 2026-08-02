"""Hard end-to-end wall-time gate for agent-facing ASP query/search commands."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Final, Sequence

AGENT_FACING_WALL_BUDGET_MICROS: Final = 1_000_000
RECEIPT_SCHEMA_ID: Final = (
    "agent.semantic-protocols.agent-facing-search-wall-receipt"
)


def command_digest(argv: Sequence[str]) -> str:
    encoded = json.dumps(list(argv), separators=(",", ":"), ensure_ascii=True).encode()
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def budget_status(wall_time_micros: int) -> str:
    return (
        "within-budget"
        if wall_time_micros < AGENT_FACING_WALL_BUDGET_MICROS
        else "budget-exceeded"
    )


def build_receipt(
    *,
    language_id: str,
    surface: str,
    argv: Sequence[str],
    wall_time_micros: int,
    reply_kind: str,
    exit_code: int,
) -> dict[str, object]:
    return {
        "schemaId": RECEIPT_SCHEMA_ID,
        "schemaVersion": "1",
        "surface": surface,
        "languageId": language_id,
        "commandDigest": command_digest(argv),
        "timingScope": "agent-facing-end-to-end",
        "wallTimeMicros": wall_time_micros,
        "budgetMicros": AGENT_FACING_WALL_BUDGET_MICROS,
        "budgetStatus": budget_status(wall_time_micros),
        "replyKind": reply_kind,
        "exitCode": exit_code,
    }


def validate_asp_command(argv: Sequence[str], language_id: str, surface: str) -> None:
    if len(argv) < 3:
        raise ValueError("expected `asp <language> <query|search> ...`")
    if Path(argv[0]).name != "asp":
        raise ValueError("the measured command must use the ASP facade")
    if argv[1] != language_id or argv[2] != surface:
        raise ValueError("command language/surface does not match receipt identity")


def run_gate(argv: Sequence[str], language_id: str, surface: str) -> int:
    validate_asp_command(argv, language_id, surface)
    started = time.perf_counter_ns()
    process = subprocess.Popen(
        list(argv),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        stdout, _stderr = process.communicate(
            timeout=AGENT_FACING_WALL_BUDGET_MICROS / 1_000_000
        )
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGTERM)
        try:
            stdout, _stderr = process.communicate(timeout=0.1)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, _stderr = process.communicate()
    wall_time_micros = (time.perf_counter_ns() - started) // 1_000
    exit_code = 124 if timed_out else process.returncode
    if timed_out:
        reply_kind = "no-reply"
    elif exit_code != 0:
        reply_kind = "failure"
    elif stdout:
        reply_kind = "evidence"
    else:
        reply_kind = "unavailable"
    receipt = build_receipt(
        language_id=language_id,
        surface=surface,
        argv=argv,
        wall_time_micros=wall_time_micros,
        reply_kind=reply_kind,
        exit_code=exit_code,
    )
    sys.stdout.write(json.dumps(receipt, separators=(",", ":"), sort_keys=True))
    sys.stdout.write("\n")
    return 0 if receipt["budgetStatus"] == "within-budget" and exit_code == 0 else 1


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--language", required=True)
    parser.add_argument("--surface", choices=("query", "search"), required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        return run_gate(args.command, args.language, args.surface)
    except ValueError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    raise SystemExit(main())
