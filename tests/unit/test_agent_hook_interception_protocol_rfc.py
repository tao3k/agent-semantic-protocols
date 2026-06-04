"""Contract checks for the agent hook interception RFC."""

from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = _REPO_ROOT / "rfcs" / "agent-hook-interception-protocol.org"


def test_codex_source_access_layer_records_no_daemon_scope() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "schemas/semantic-source-access-decision.v1.schema.json",
        "asp source-access read-file --activation <activation.json>",
        "asp source-access shell-egress --activation <activation.json>",
        "MCP surfaces are out of scope",
        "fsApi=enforced",
        "shellOutput=egress-enforced",
        "subprocessOpen=not-enforced",
        "mcp=out-of-scope",
        "daemon=not-used",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
