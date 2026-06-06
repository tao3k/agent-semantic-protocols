"""Build ASLP CodeQL evidence payloads from bounded fixture runs."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

try:
    from tools.codeql_bounded_runtime import CodeqlBoundedRun
except ModuleNotFoundError:
    from codeql_bounded_runtime import CodeqlBoundedRun


def build_codeql_bounded_evidence(
    *,
    run: CodeqlBoundedRun,
    language_id: str,
    provider_id: str,
    project_root: str,
    source_root: Path,
    codeql_language: str,
    query_file: Path,
    generated_at: str,
) -> dict[str, Any]:
    codeql_version = str(run.version_payload.get("version", "unknown"))
    codeql_sha = str(run.version_payload.get("sha", "unknown"))
    database_languages = [str(language) for language in run.database_info.get("languages", [])]
    rust_library_pack_available = _rust_library_pack_available(run.qlpacks_payload)
    fingerprint = _evidence_fingerprint(
        codeql_language=codeql_language,
        codeql_sha=codeql_sha,
        codeql_version=codeql_version,
        database_languages=database_languages,
        query_text=run.query_text,
        rows=run.rows,
        source_root=source_root,
    )
    evidence = _base_evidence(
        run=run,
        language_id=language_id,
        provider_id=provider_id,
        project_root=project_root,
        source_root=source_root,
        codeql_language=codeql_language,
        query_file=query_file,
        generated_at=generated_at,
        codeql_version=codeql_version,
        codeql_sha=codeql_sha,
        database_languages=database_languages,
        rust_library_pack_available=rust_library_pack_available,
        fingerprint=fingerprint,
    )
    if not run.rows:
        evidence["omissions"].append(_empty_result_omission())
    return evidence


def _base_evidence(
    *,
    run: CodeqlBoundedRun,
    language_id: str,
    provider_id: str,
    project_root: str,
    source_root: Path,
    codeql_language: str,
    query_file: Path,
    generated_at: str,
    codeql_version: str,
    codeql_sha: str,
    database_languages: list[str],
    rust_library_pack_available: bool,
    fingerprint: str,
) -> dict[str, Any]:
    return {
        "schemaId": "agent.semantic-protocols.semantic-codeql-evidence",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "artifactId": "codeql-evidence/bounded/rust-source-file-query.json",
        "databaseFingerprint": f"codeql-db:{fingerprint}",
        "queryId": "asp.codeql.rust.raw-dbscheme.source-file",
        "queryVersion": codeql_version,
        "generatedAt": generated_at,
        "languageId": language_id,
        "providerId": provider_id,
        "projectRoot": project_root,
        "packageName": "aslp_codeql_fixture",
        "inputHandles": [
            f"codeql:fixture:{source_root.as_posix()}",
            f"codeql:query:{query_file.as_posix()}",
        ],
        "rowCount": len(run.rows),
        "projectRootPolicy": "local-only",
        "sourceSnapshot": {
            "kind": "workspace",
            "fingerprint": f"sha256:{fingerprint}",
            "generatedFrom": "codeql database create + codeql query run + bqrs decode",
            "fields": {
                "adapterMode": "codeql-raw-dbscheme",
                "codeqlVersion": codeql_version,
                "databaseCacheStatus": run.database_cache_status,
                "executionBackend": "codeql",
                "rustLibraryPackAvailable": rust_library_pack_available,
            },
        },
        "flowId": "flow-lite:codeql:bounded-fixture:source-file",
        "normalizedRows": run.rows,
        "omissions": [],
        "fields": {
            "adapterMode": "codeql-raw-dbscheme",
            "codeqlLanguage": codeql_language,
            "codeqlSha": codeql_sha,
            "codeqlVersion": codeql_version,
            "databaseCacheEnabled": run.database_cache_status != "disabled",
            "databaseCacheStatus": run.database_cache_status,
            "databaseLanguages": database_languages,
            "executionBackend": "codeql",
            "queryFile": query_file.as_posix(),
            "rawDbschemePredicate": "files",
            "rustLibraryPackAvailable": rust_library_pack_available,
            "sourceAuthority": "codeql",
        },
    }


def _empty_result_omission() -> dict[str, Any]:
    return {
        "kind": "ambiguous",
        "message": "CodeQL fixture query returned no source file rows.",
        "target": "src/lib.rs",
        "fields": {"executionBackend": "codeql"},
    }


def _rust_library_pack_available(qlpacks_payload: dict[str, Any]) -> bool:
    return any(pack in qlpacks_payload for pack in ("codeql/rust-all", "codeql/rust-queries"))


def _evidence_fingerprint(
    *,
    codeql_language: str,
    codeql_sha: str,
    codeql_version: str,
    database_languages: list[str],
    query_text: str,
    rows: list[dict[str, Any]],
    source_root: Path,
) -> str:
    return _fingerprint(
        {
            "codeqlLanguage": codeql_language,
            "codeqlSha": codeql_sha,
            "codeqlVersion": codeql_version,
            "databaseLanguages": database_languages,
            "queryText": query_text,
            "rowHandles": [row.get("sourceHandle") for row in rows],
            "sourceDigest": _source_digest(source_root),
        }
    )


def _source_digest(source_root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(path for path in source_root.rglob("*") if path.is_file()):
        if "target" in path.parts:
            continue
        digest.update(path.relative_to(source_root).as_posix().encode())
        digest.update(b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def _fingerprint(payload: dict[str, Any]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(encoded).hexdigest()
