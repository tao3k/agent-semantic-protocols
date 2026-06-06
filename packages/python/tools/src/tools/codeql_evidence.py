"""Normalize CodeQL CLI metadata into ASP evidence artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import UTC, datetime
from typing import Any, Sequence


def emit_codeql_evidence(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--language-id", default="rust")
    parser.add_argument("--provider-id", default="rs-harness")
    parser.add_argument("--project-root", default=".")
    parser.add_argument("--codeql-language", default="rust")
    parser.add_argument("--generated-at")
    args = parser.parse_args(argv)

    version_payload = _run_codeql_json(["version", "--format=json"])
    languages_payload = _run_codeql_json(["resolve", "languages", "--format=json"])
    evidence = _build_evidence(
        language_id=args.language_id,
        provider_id=args.provider_id,
        project_root=args.project_root,
        codeql_language=args.codeql_language,
        generated_at=args.generated_at or _utc_now(),
        version_payload=version_payload,
        languages_payload=languages_payload,
    )
    sys.stdout.write(json.dumps(evidence, sort_keys=True, separators=(",", ":")) + "\n")
    return 0


def _run_codeql_json(args: list[str]) -> Any:
    completed = subprocess.run(
        ["codeql", *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return json.loads(completed.stdout)


def _build_evidence(
    *,
    language_id: str,
    provider_id: str,
    project_root: str,
    codeql_language: str,
    generated_at: str,
    version_payload: dict[str, Any],
    languages_payload: dict[str, Any],
) -> dict[str, Any]:
    visible_languages = sorted(str(language) for language in languages_payload)
    language_paths = languages_payload.get(codeql_language, [])
    language_available = isinstance(language_paths, list) and bool(language_paths)
    row_count = 1 if language_available else 0
    codeql_version = str(version_payload.get("version", "unknown"))
    codeql_sha = str(version_payload.get("sha", "unknown"))
    fingerprint = _fingerprint(
        {
            "codeqlLanguage": codeql_language,
            "codeqlSha": codeql_sha,
            "codeqlVersion": codeql_version,
            "languageAvailable": language_available,
            "visibleLanguages": visible_languages,
        }
    )
    evidence = {
        "schemaId": "agent.semantic-protocols.semantic-codeql-evidence",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "artifactId": f"codeql-evidence/metadata/{codeql_language}-resolve-languages.json",
        "databaseFingerprint": f"codeql-cli:{codeql_version}:{codeql_sha}",
        "queryId": "asp.codeql.resolve-languages",
        "queryVersion": codeql_version,
        "generatedAt": generated_at,
        "languageId": language_id,
        "providerId": provider_id,
        "projectRoot": project_root,
        "inputHandles": [f"codeql:language:{codeql_language}"],
        "rowCount": row_count,
        "projectRootPolicy": "local-only",
        "sourceSnapshot": {
            "kind": "provider-export",
            "fingerprint": f"sha256:{fingerprint}",
            "generatedFrom": "codeql resolve languages --format=json",
            "fields": {
                "codeqlVersion": codeql_version,
                "languageAvailable": language_available,
            },
        },
        "flowId": f"flow-lite:codeql:availability:{codeql_language}",
        "normalizedRows": [],
        "omissions": [],
        "fields": {
            "codeqlLanguage": codeql_language,
            "codeqlSha": codeql_sha,
            "codeqlVersion": codeql_version,
            "languageAvailable": language_available,
            "languagePackCount": len(language_paths) if isinstance(language_paths, list) else 0,
            "visibleLanguageCount": len(visible_languages),
            "visibleLanguages": visible_languages,
        },
    }
    if language_available:
        evidence["normalizedRows"].append(
            {
                "id": f"codeql-language.{codeql_language}",
                "kind": "source",
                "sourceHandle": f"codeql:language:{codeql_language}",
                "fields": {
                    "languageAvailable": True,
                    "languagePackCount": len(language_paths),
                },
            }
        )
    else:
        evidence["omissions"].append(
            {
                "kind": "backend-unavailable",
                "message": f"CodeQL language pack is not available: {codeql_language}",
                "target": f"codeql:{codeql_language}",
                "fields": {"executionBackend": "codeql"},
            }
        )
    return evidence


def _fingerprint(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()


def _utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


if __name__ == "__main__":
    raise SystemExit(emit_codeql_evidence(sys.argv[1:]))
