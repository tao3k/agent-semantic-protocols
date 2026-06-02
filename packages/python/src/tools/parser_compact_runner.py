"""Provider command execution and artifact checks for parser compact snapshots."""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from tools.parser_compact_model import (
    ParserCompactCase,
    ParserCompactOutputSet,
    REPO_ROOT,
    case_label,
)
from tools.parser_compact_tokenizers import Tokenizer


@dataclass(frozen=True)
class ParserCompactArtifacts:
    line: str
    code: str
    query_packet: str
    token_cost: str


def refresh_case(case: ParserCompactCase, tokenizer: Tokenizer) -> None:
    artifacts = run_case_artifacts(case, tokenizer)
    write_output_set(case.real_output, artifacts)
    write_output_set(case.expected_output, artifacts)


def check_case(
    case: ParserCompactCase,
    tokenizer: Tokenizer,
    *,
    check_provider: bool,
) -> list[str]:
    failures: list[str] = []
    expected_artifacts = read_output_set(case.expected_output)
    expected_token_cost = build_token_report(
        case,
        tokenizer,
        expected_artifacts.line,
        expected_artifacts.code,
        expected_artifacts.query_packet,
    )
    if report_text(expected_token_cost) != expected_artifacts.token_cost:
        failures.append(f"{case_label(case)}: token report drift")
    if check_provider:
        real_artifacts = run_case_artifacts(case, tokenizer)
        write_output_set(case.real_output, real_artifacts)
        failures.extend(compare_artifacts(case, real_artifacts, expected_artifacts))
    return failures


def run_case_artifacts(
    case: ParserCompactCase,
    tokenizer: Tokenizer,
) -> ParserCompactArtifacts:
    line_text = run_case_command(case.line_command, case)
    query_packet_text, code_text = query_packet_artifacts(
        run_case_command(case.json_command, case),
        case,
    )
    token_cost_text = report_text(
        build_token_report(case, tokenizer, line_text, code_text, query_packet_text)
    )
    return ParserCompactArtifacts(
        line=line_text,
        code=code_text,
        query_packet=query_packet_text,
        token_cost=token_cost_text,
    )


def compare_artifacts(
    case: ParserCompactCase,
    real: ParserCompactArtifacts,
    expected: ParserCompactArtifacts,
) -> list[str]:
    artifact_names = ("line", "code", "query_packet", "token_cost")
    return [
        f"{case_label(case)}: {artifact_name} snapshot drift"
        for artifact_name in artifact_names
        if getattr(real, artifact_name) != getattr(expected, artifact_name)
    ]


def read_output_set(output: ParserCompactOutputSet) -> ParserCompactArtifacts:
    return ParserCompactArtifacts(
        line=read_text_artifact(output.line),
        code=read_text_artifact(output.code),
        query_packet=read_text_artifact(output.query_packet),
        token_cost=read_text_artifact(output.token_cost),
    )


def write_output_set(
    output: ParserCompactOutputSet,
    artifacts: ParserCompactArtifacts,
) -> None:
    output.root.mkdir(parents=True, exist_ok=True)
    output.line.write_text(artifacts.line, encoding="utf-8")
    output.code.write_text(artifacts.code, encoding="utf-8")
    output.query_packet.write_text(artifacts.query_packet, encoding="utf-8")
    output.token_cost.write_text(artifacts.token_cost, encoding="utf-8")


def build_token_report(
    case: ParserCompactCase,
    tokenizer: Tokenizer,
    line_text: str,
    code_text: str,
    query_packet_text: str,
) -> dict[str, object]:
    raw_source = case.raw_source_path.read_text(encoding="utf-8")
    raw_tokens = tokenizer.count(raw_source)
    line_tokens = tokenizer.count(line_text)
    code_tokens = tokenizer.count(code_text)
    query_packet_tokens = tokenizer.count(query_packet_text)
    return {
        "schemaId": "agent.semantic-protocols.parser-compact-token-cost",
        "schemaVersion": "1",
        "caseId": case.case_id,
        "variantId": case.variant_id,
        "languageId": case.language_id,
        "providerId": case.provider_id,
        "featureClass": case.feature_class,
        "tokenizerId": tokenizer.tokenizer_id,
        "rawSourceTokens": raw_tokens,
        "compactLineTokens": line_tokens,
        "compactCodeTokens": code_tokens,
        "queryPacketTokens": query_packet_tokens,
        "compactLineDelta": raw_tokens - line_tokens,
        "compactCodeDelta": raw_tokens - code_tokens,
        "queryPacketDelta": raw_tokens - query_packet_tokens,
        "compactLineRatio": round(line_tokens / raw_tokens, 4) if raw_tokens else 0,
        "compactCodeRatio": round(code_tokens / raw_tokens, 4) if raw_tokens else 0,
        "queryPacketRatio": round(query_packet_tokens / raw_tokens, 4) if raw_tokens else 0,
    }


def run_case_command(command: Sequence[str], case: ParserCompactCase) -> str:
    return run_provider_command(resolve_provider_command(command, case), case.fixture_root)


def run_provider_command(command: Sequence[str], cwd: Path) -> str:
    result = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"provider command failed ({result.returncode}): {' '.join(command)}\n{result.stderr}"
        )
    return normalize_text(result.stdout)


def resolve_provider_command(command: Sequence[str], case: ParserCompactCase) -> list[str]:
    replacements = {
        "{repoRoot}": str(REPO_ROOT),
        "{fixtureRoot}": str(case.fixture_root),
    }
    return [_replace_command_placeholders(argument, replacements) for argument in command]


def _replace_command_placeholders(argument: str, replacements: dict[str, str]) -> str:
    for token, value in replacements.items():
        argument = argument.replace(token, value)
    return argument


def normalize_text(text: str) -> str:
    return text.replace("\r\n", "\n").rstrip() + "\n"


def query_packet_artifacts(text: str, case: ParserCompactCase) -> tuple[str, str]:
    packet = normalize_snapshot_paths(json.loads(text), case)
    code_text = compact_code_text(packet)
    for match in packet.get("matches", []):
        if isinstance(match, dict):
            match.pop("code", None)
    return json.dumps(packet, indent=2, sort_keys=True) + "\n", code_text


def normalize_snapshot_paths(value: object, case: ParserCompactCase) -> object:
    if isinstance(value, dict):
        return {key: normalize_snapshot_paths(item, case) for key, item in value.items()}
    if isinstance(value, list):
        return [normalize_snapshot_paths(item, case) for item in value]
    if isinstance(value, str):
        return normalize_snapshot_path(value, case)
    return value


def normalize_snapshot_path(value: str, case: ParserCompactCase) -> str:
    repo_root = str(REPO_ROOT)
    fixture_root = str(case.fixture_root)
    fixture_relative = case.fixture_root.relative_to(REPO_ROOT).as_posix()
    if value == fixture_root:
        return fixture_relative
    if value.startswith(f"{fixture_root}/"):
        return f"{fixture_relative}/{value[len(fixture_root) + 1:]}"
    if value == repo_root:
        return "."
    if value.startswith(f"{repo_root}/"):
        return value[len(repo_root) + 1 :]
    return value


def compact_code_text(packet: object) -> str:
    if not isinstance(packet, dict):
        raise ValueError("query packet must be a JSON object")
    matches = packet.get("matches")
    if not isinstance(matches, list):
        raise ValueError("query packet must contain matches")
    code_chunks: list[str] = []
    for match in matches:
        if not isinstance(match, dict):
            continue
        code = match.get("code")
        if code is None:
            continue
        if not isinstance(code, str):
            raise ValueError("query packet match code must be a string")
        code_chunks.append(normalize_text(code).rstrip("\n"))
    if not code_chunks:
        raise ValueError("query packet did not include compact code")
    return normalize_text("\n\n".join(code_chunks))


def read_text_artifact(path: Path) -> str:
    return normalize_text(path.read_text(encoding="utf-8"))


def report_text(report: dict[str, object]) -> str:
    return json.dumps(report, indent=2, sort_keys=True) + "\n"
