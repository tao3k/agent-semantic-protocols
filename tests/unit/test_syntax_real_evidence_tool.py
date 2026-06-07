"""Tests for the RFC 011 syntax real-project evidence helper."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_TOOLS_SRC = _REPO_ROOT / "packages" / "python" / "tools" / "src"


def _tools_env() -> dict[str, str]:
    env = os.environ.copy()
    current = env.get("PYTHONPATH")
    env["PYTHONPATH"] = str(_TOOLS_SRC) if not current else f"{_TOOLS_SRC}{os.pathsep}{current}"
    return env


def test_record_syntax_real_evidence_renders_review_record() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "tools",
            "syntax",
            "real-evidence",
            "--language",
            "rust",
            "--provider",
            "rs-harness",
            "--project",
            "tokio",
            "--command-count",
            "4",
            "--provider-process-count",
            "4",
            "--packet-bytes",
            "4096",
            "--cold-elapsed-ms",
            "1200",
            "--warm-elapsed-ms",
            "800",
            "--syntax-query-count",
            "2",
            "--exact-code-count",
            "1",
            "--manual-range-scan-count",
            "0",
            "--repeated-trigger-reduction",
            "3",
            "--cache-claim",
            "warm-provider",
        ],
        check=True,
        capture_output=True,
        env=_tools_env(),
        text=True,
    )

    assert result.stdout.splitlines() == [
        "[syntax-real-evidence] language=rust provider=rs-harness project=tokio",
        "commands=search-prime,syntax-frontier,exact-selector-code,hook-recovery",
        "metrics=commandCount=4,providerProcessCount=4,packetBytes=4096,coldElapsedMs=1200,warmElapsedMs=800",
        "metrics=syntaxQueryCount=2,exactCodeCount=1,manualRangeScanCount=0,repeatedTriggerReduction=3",
        "outputs=frontier-no-code,pure-code-stdout,registry-descriptor,query-corpus",
        "cacheClaim=warm-provider",
    ]


def test_record_syntax_real_evidence_rejects_unproven_cache_hit() -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "tools",
            "syntax",
            "real-evidence",
            "--language",
            "typescript",
            "--provider",
            "ts-harness",
            "--project",
            "playwright",
            "--command-count",
            "4",
            "--provider-process-count",
            "4",
            "--packet-bytes",
            "4096",
            "--cold-elapsed-ms",
            "1200",
            "--warm-elapsed-ms",
            "800",
            "--syntax-query-count",
            "2",
            "--exact-code-count",
            "1",
            "--manual-range-scan-count",
            "0",
            "--repeated-trigger-reduction",
            "3",
            "--cache-claim",
            "hit",
        ],
        check=False,
        capture_output=True,
        env=_tools_env(),
        text=True,
    )

    assert result.returncode == 2
    assert "--cache-claim hit requires" in result.stderr
