"""CLI entrypoint for bounded ASP CodeQL evidence."""

from __future__ import annotations

import argparse
import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Sequence

try:
    from tools.codeql_bounded_payload import build_codeql_bounded_evidence
    from tools.codeql_bounded_runtime import run_codeql_bounded_fixture
except ModuleNotFoundError:
    from codeql_bounded_payload import build_codeql_bounded_evidence
    from codeql_bounded_runtime import run_codeql_bounded_fixture


def emit_codeql_bounded_evidence(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    source_root = args.source_root.resolve()
    query_file = args.query_file.resolve()
    run = run_codeql_bounded_fixture(
        source_root=source_root,
        query_file=query_file,
        codeql_language=args.codeql_language,
        cache_dir=None if args.no_cache else args.cache_dir.resolve(),
    )
    evidence = build_codeql_bounded_evidence(
        run=run,
        language_id=args.language_id,
        provider_id=args.provider_id,
        project_root=args.project_root,
        source_root=source_root,
        codeql_language=args.codeql_language,
        query_file=query_file,
        generated_at=args.generated_at or _utc_now(),
    )
    sys.stdout.write(json.dumps(evidence, sort_keys=True, separators=(",", ":")) + "\n")
    return 0


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-root",
        default="sandtables/fixtures/asp/codeql-rust-flow",
        type=Path,
    )
    parser.add_argument(
        "--query-file",
        default="sandtables/fixtures/asp/codeql-rust-flow/queries/source-file.ql",
        type=Path,
    )
    parser.add_argument("--language-id", default="rust")
    parser.add_argument("--provider-id", default="rs-harness")
    parser.add_argument("--project-root", default=".")
    parser.add_argument("--codeql-language", default="rust")
    parser.add_argument(
        "--cache-dir",
        default=".cache/agent-semantic-protocol/codeql-fixtures",
        type=Path,
    )
    parser.add_argument("--no-cache", action="store_true")
    parser.add_argument("--generated-at")
    return parser.parse_args(argv)


def _utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    raise SystemExit(emit_codeql_bounded_evidence(sys.argv[1:]))
