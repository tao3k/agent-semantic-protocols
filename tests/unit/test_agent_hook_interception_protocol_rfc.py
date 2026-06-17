"""Contract checks for the agent hook interception RFC."""

from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = (
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.15-agent-hook-interception-protocol.org"
)


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


def test_hook_rfc_defines_markdown_recovery_prompt_and_runtime_binaries() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "the rendered reason text is Markdown",
        "# ASP Hook Recovery",
        "## Run Next",
        "## Detected Binaries",
        "command=<runtime-profile argv>",
        "Start from asp <language> guide --workspace .",
        "runtime profile's resolved =argv=",
        "The public route in",
        "=routes[].argv= still uses the =asp <language>= facade",
        "experimental.semanticAstPatch.enabled = false",
        "=apply_patch=",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_hook_rfc_closure_gates_cover_recovery_prompt_and_retired_routes() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "Prompt-facing deny output is a Markdown recovery prompt",
        "=# ASP Hook Recovery=",
        "=## Detected Binaries=",
        "current runtime profile command argv",
        "=routes[].argv= remains the public =asp <language>=",
        "experimental.semanticAstPatch.enabled = false",
        "=apply_patch=",
        "Generated skills and hook prompts must not reintroduce",
        "agent-prefixed guide spelling",
        "old search-wrapper hook query",
        "accepted guide entrypoint is =asp <language> guide --workspace .=",
        "help surface is =asp <language> guide --help .=",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
