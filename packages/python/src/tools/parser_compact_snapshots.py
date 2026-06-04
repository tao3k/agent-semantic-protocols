"""CLI facade for parser compact snapshot and token-cost checks."""

from __future__ import annotations

import argparse
import sys
from typing import Sequence

from tools.parser_compact_model import (
    ParserCompactCase,
    case_label,
    iter_case_paths,
    load_case,
    load_matching_cases,
)
from tools.parser_compact_runner import check_case, refresh_case
from tools.parser_compact_tokenizers import Tokenizer, load_tokenizer

__all__ = [
    "ParserCompactCase",
    "Tokenizer",
    "case_label",
    "iter_case_paths",
    "load_case",
    "load_tokenizer",
    "main",
]


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_cli_args(argv)
    tokenizer = load_tokenizer(args.tokenizer)
    cases = load_matching_cases(args.case_id, args.language_id)
    if not cases:
        sys.stderr.write("no parser compact cases matched\n")
        return 1
    failures = _run_selected_cases(
        cases,
        tokenizer,
        refresh=args.refresh,
        check_provider=args.check_provider,
    )
    return _finish_with_failures(failures)


def _parse_cli_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", dest="case_id")
    parser.add_argument("--language", dest="language_id")
    parser.add_argument("--tokenizer", default="byte")
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--check-provider", action="store_true")
    return parser.parse_args(argv)


def _run_selected_cases(
    cases: Sequence[ParserCompactCase],
    tokenizer: Tokenizer,
    *,
    refresh: bool,
    check_provider: bool,
) -> list[str]:
    failures: list[str] = []
    for case in cases:
        if refresh:
            refresh_case(case, tokenizer)
        else:
            failures.extend(check_case(case, tokenizer, check_provider=check_provider))
    return failures


def _finish_with_failures(failures: Sequence[str]) -> int:
    if not failures:
        return 0
    for failure in failures:
        sys.stderr.write(f"{failure}\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
