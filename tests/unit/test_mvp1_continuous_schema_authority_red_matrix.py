"""MVP1 schema-authority red matrix.

This is deliberately an acceptance oracle, rather than a compatibility test.
It describes the contract that the continuous MVP1 package must satisfy and
keeps the current implementation visible when it is still red.
"""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
DEVENV = ROOT / ".devenv/devenv-profile-exec"
LANGUAGES = (
    "rust",
    "typescript",
    "python",
    "julia",
    "gerbil-scheme",
    "org",
    "md",
)
PHASES = ("materialize", "verify", "materialize-second", "verify-second")
NEGATIVE_NAMES = (
    "legacy-string",
    "second-authority",
    "Queued",
    "projection",
    "cross-generation",
    "command",
    "fallback",
    "version",
    "extension",
    "compatibility",
    "alias",
    "dual-path",
    "silent-retry",
    "URL-heuristic",
)


def _run_schema_phase(language: str, phase: str) -> subprocess.CompletedProcess[str]:
    operation = phase.removesuffix("-second")
    argv = [
        str(DEVENV),
        "target/debug/asp",
        "schema",
        operation,
        "--workspace",
        str(ROOT),
        "--language",
        language,
    ]
    result = subprocess.run(argv, cwd=ROOT, text=True, capture_output=True, check=False)
    print(
        "[schema-materialization-receipt]"
        f" language={language} phase={phase} exit={result.returncode}"
        f" argv={json.dumps(argv)} stdout={result.stdout.strip()!r}"
        f" stderr={result.stderr.strip()!r}"
    )
    return result


def _receipt_for(language: str) -> dict[str, object]:
    roots = {
        "rust": ROOT / "languages/asp-rust/schemas",
        "typescript": ROOT / "languages/typescript-lang-project-harness/schemas",
        "python": ROOT / "languages/asp-python/schemas",
        "julia": ROOT / "languages/AspJulia.jl/schemas",
        "gerbil-scheme": ROOT / "languages/gerbil-scheme-language-project-harness/schemas",
        "org": ROOT / "languages/orgize/provider/org/schemas",
        "md": ROOT / "languages/orgize/provider/md/schemas",
    }
    return json.loads(
        (roots[language] / ".asp-schema-manager-receipt.json").read_text(
            encoding="utf-8"
        )
    )


@pytest.mark.parametrize("language", LANGUAGES)
@pytest.mark.parametrize("phase", PHASES)
def test_all_seven_materialization_workflows_have_exact_receipts(
    language: str, phase: str
) -> None:
    result = _run_schema_phase(language, phase)
    assert result.returncode == 0, result.stderr or result.stdout
    match = re.search(
        r"\[schema-bundle\] language=(?P<language>\S+) schemas=(?P<schemas>\d+) "
        r"changed=(?P<changed>\d+) removed=(?P<removed>\d+) "
        r"digest=(?P<digest>\S+) receipt=(?P<receipt>\S+)",
        result.stdout,
    )
    assert match, f"missing typed schema-bundle receipt: {result.stdout!r}"
    assert match.group("language") == language
    assert match.group("schemas").isdigit()
    assert match.group("digest").startswith("blake3-256:")
    if phase in {"materialize-second", "verify-second"}:
        assert match.group("changed") == "0"
        assert match.group("removed") == "0"


MANAGER_RESULT_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "required": ["schemaId", "schemaVersion", "schemaDigest"],
    "properties": {
        "schemaId": {"type": "string", "minLength": 1},
        "schemaVersion": {"const": "1"},
        "schemaDigest": {
            "type": "string",
            "pattern": r"^blake3-256:[0-9a-f]{64}$",
        },
    },
}


def test_manager_result_positive_is_exact_triplet() -> None:
    result = {
        "schemaId": "agent.semantic-protocols.asp-client-schema-bundle-response",
        "schemaVersion": "1",
        "schemaDigest": "blake3-256:" + ("a" * 64),
    }
    assert list(Draft202012Validator(MANAGER_RESULT_SCHEMA).iter_errors(result)) == []
    assert set(result) == {"schemaId", "schemaVersion", "schemaDigest"}


@pytest.mark.parametrize("negative", NEGATIVE_NAMES)
def test_manager_result_rejects_mvp1_negative(negative: str) -> None:
    result: object = {
        "schemaId": "agent.semantic-protocols.asp-client-schema-bundle-response",
        "schemaVersion": "1",
        "schemaDigest": "blake3-256:" + ("a" * 64),
    }
    if negative == "legacy-string":
        result = "agent.semantic-protocols.asp-client-schema-bundle-response/1"
    elif negative == "version":
        result = {
            "schemaId": "agent.semantic-protocols.asp-client-schema-bundle-response",
            "schemaVersion": "2",
            "schemaDigest": "blake3-256:" + ("a" * 64),
        }
    else:
        result = {
            "schemaId": "agent.semantic-protocols.asp-client-schema-bundle-response",
            "schemaVersion": "1",
            "schemaDigest": "blake3-256:" + ("a" * 64),
            negative: True,
        }
    assert list(Draft202012Validator(MANAGER_RESULT_SCHEMA).iter_errors(result)), negative


@pytest.mark.parametrize("language", LANGUAGES)
def test_manager_result_is_exact_schema_reference_triplet(language: str) -> None:
    """The current receipt is expected to go red until manager result identity is fixed."""

    receipt = _receipt_for(language)
    errors = sorted(Draft202012Validator(MANAGER_RESULT_SCHEMA).iter_errors(receipt), key=str)
    assert not errors, f"manager result drift for {language}: {[error.message for error in errors]}"
    assert set(receipt) == {"schemaId", "schemaVersion", "schemaDigest"}


SCHEMA_REFERENCE_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "required": ["schemaId", "schemaVersion"],
    "properties": {
        "schemaId": {"type": "string", "minLength": 1},
        "schemaVersion": {"const": "1"},
    },
}


@pytest.mark.parametrize(
    "reference",
    [
        {"schemaId": "asp.mvp1.contract", "schemaVersion": "1"},
        {"schemaId": "provider.route", "schemaVersion": "1"},
    ],
)
def test_schema_reference_positive_is_exactly_two_fields(reference: dict[str, str]) -> None:
    assert list(Draft202012Validator(SCHEMA_REFERENCE_SCHEMA).iter_errors(reference)) == []
    assert set(reference) == {"schemaId", "schemaVersion"}


@pytest.mark.parametrize("negative", NEGATIVE_NAMES)
def test_schema_reference_rejects_mvp1_negative(negative: str) -> None:
    value: object = {"schemaId": "asp.mvp1.contract", "schemaVersion": "1"}
    if negative == "legacy-string":
        value = "asp.mvp1.contract/1"
    elif negative == "version":
        value = {"schemaId": "asp.mvp1.contract", "schemaVersion": "2"}
    else:
        value = {"schemaId": "asp.mvp1.contract", "schemaVersion": "1", negative: True}
    assert list(Draft202012Validator(SCHEMA_REFERENCE_SCHEMA).iter_errors(value)), negative


@pytest.mark.parametrize(
    ("owner", "path", "required_pattern"),
    [
        (
            "Schema Manager",
            "crates/agent-semantic-schema-manager/src/manager.rs",
            r"pub struct SchemaManager\b",
        ),
        (
            "ASP Server",
            "crates/agent-semantic-runtime-server/src/schema_bundle.rs",
            r"pub struct RuntimeSchemaBundleCatalog\b",
        ),
    ],
)
def test_writer_authority_has_one_named_owner(
    owner: str, path: str, required_pattern: str
) -> None:
    source = (ROOT / path).read_text(encoding="utf-8")
    assert len(re.findall(required_pattern, source)) == 1, f"{owner} writer token is not unique"


def test_language_providers_do_not_define_a_second_schema_manager_writer() -> None:
    definitions: list[str] = []
    for path in sorted((ROOT / "languages").glob("**/*")):
        if path.is_file() and path.suffix in {".rs", ".py", ".ts", ".jl", ".ss"}:
            text = path.read_text(encoding="utf-8", errors="replace")
            if re.search(r"(?:struct|class|def|function)\s+SchemaManager", text):
                definitions.append(str(path.relative_to(ROOT)))
    assert definitions == [], f"language-provider schema writers found: {definitions}"


def test_grpc_and_http_bind_the_same_bidirectional_frame_owner() -> None:
    runtime_server = (ROOT / "crates/agent-semantic-runtime-server/src/lib.rs").read_text(
        encoding="utf-8"
    )
    assert "serve_asp_client_grpc_unix" in runtime_server
    assert "serve_http_json" in runtime_server
