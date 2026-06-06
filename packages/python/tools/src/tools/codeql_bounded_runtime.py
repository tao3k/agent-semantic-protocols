"""CodeQL runtime helpers for bounded ASLP fixture evidence."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

try:
    from tools.codeql_bounded_cache import codeql_fixture_cache_key
except ModuleNotFoundError:
    from codeql_bounded_cache import codeql_fixture_cache_key


@dataclass(frozen=True, slots=True)
class CodeqlBoundedRun:
    version_payload: dict[str, Any]
    database_info: dict[str, Any]
    qlpacks_payload: dict[str, Any]
    query_text: str
    rows: list[dict[str, Any]]
    database_cache_status: str
    database_cache_dir: str | None


def run_codeql_bounded_fixture(
    *, source_root: Path, query_file: Path, codeql_language: str, cache_dir: Path | None = None
) -> CodeqlBoundedRun:
    version_payload = _run_codeql_json(["version", "--format=json"])
    qlpacks_payload = _run_codeql_json(["resolve", "qlpacks", "--format=json"])
    if cache_dir is not None:
        return _run_cached_fixture(
            source_root=source_root,
            query_file=query_file,
            codeql_language=codeql_language,
            cache_dir=cache_dir,
            version_payload=version_payload,
            qlpacks_payload=qlpacks_payload,
        )
    with tempfile.TemporaryDirectory(prefix="aslp-codeql-") as temp_dir:
        paths = _prepare_fixture_paths(Path(temp_dir), source_root, query_file)
        _create_database(paths.fixture_root, paths.database_root, codeql_language)
        return CodeqlBoundedRun(
            version_payload=version_payload,
            database_info=_run_codeql_json(
                ["resolve", "database", "--format=json", str(paths.database_root)]
            ),
            qlpacks_payload=qlpacks_payload,
            query_text=query_file.read_text(encoding="utf-8"),
            rows=_query_rows(paths.fixture_root, paths.database_root, paths.query_path),
            database_cache_status="disabled",
            database_cache_dir=None,
        )


@dataclass(frozen=True, slots=True)
class _FixturePaths:
    fixture_root: Path
    database_root: Path
    query_path: Path
    bqrs_path: Path


def _run_cached_fixture(
    *,
    source_root: Path,
    query_file: Path,
    codeql_language: str,
    cache_dir: Path,
    version_payload: dict[str, Any],
    qlpacks_payload: dict[str, Any],
) -> CodeqlBoundedRun:
    cache_root = cache_dir / codeql_fixture_cache_key(
        source_root=source_root,
        query_file=query_file,
        codeql_language=codeql_language,
        version_payload=version_payload,
    )
    paths = _cached_fixture_paths(cache_root, source_root, query_file)
    cache_status = "hit" if _database_ready(paths) else "miss"
    if cache_status == "miss":
        if cache_root.exists():
            shutil.rmtree(cache_root)
        paths = _prepare_fixture_paths(cache_root, source_root, query_file)
        _create_database(paths.fixture_root, paths.database_root, codeql_language)
    return CodeqlBoundedRun(
        version_payload=version_payload,
        database_info=_run_codeql_json(["resolve", "database", "--format=json", str(paths.database_root)]),
        qlpacks_payload=qlpacks_payload,
        query_text=query_file.read_text(encoding="utf-8"),
        rows=_query_rows(paths.fixture_root, paths.database_root, paths.query_path),
        database_cache_status=cache_status,
        database_cache_dir=str(cache_dir),
    )


def _prepare_fixture_paths(temp_root: Path, source_root: Path, query_file: Path) -> _FixturePaths:
    fixture_root = temp_root / "source"
    shutil.copytree(
        source_root,
        fixture_root,
        ignore=shutil.ignore_patterns("target", "Cargo.lock"),
    )
    return _FixturePaths(
        fixture_root=fixture_root,
        database_root=temp_root / "codeql-db",
        query_path=fixture_root / query_file.relative_to(source_root),
        bqrs_path=temp_root / "source-file.bqrs",
    )


def _cached_fixture_paths(cache_root: Path, source_root: Path, query_file: Path) -> _FixturePaths:
    fixture_root = cache_root / "source"
    return _FixturePaths(
        fixture_root=fixture_root,
        database_root=cache_root / "codeql-db",
        query_path=fixture_root / query_file.relative_to(source_root),
        bqrs_path=cache_root / "source-file.bqrs",
    )


def _database_ready(paths: _FixturePaths) -> bool:
    return paths.database_root.joinpath("codeql-database.yml").is_file() and paths.query_path.is_file()


def _create_database(fixture_root: Path, database_root: Path, codeql_language: str) -> None:
    _run_codeql(
        [
            "database",
            "create",
            str(database_root),
            "--language",
            codeql_language,
            "--source-root",
            str(fixture_root),
            "--command",
            "cargo check --quiet",
            "--threads",
            "1",
            "--ram",
            "2048",
            "--quiet",
        ],
        cwd=fixture_root,
    )


def _run_fixture_query(
    fixture_root: Path, database_root: Path, query_path: Path, bqrs_path: Path
) -> None:
    _run_codeql(
        [
            "query",
            "run",
            "--database",
            str(database_root),
            "--output",
            str(bqrs_path),
            str(query_path),
            "--threads",
            "1",
            "--ram",
            "2048",
            "--quiet",
        ],
        cwd=fixture_root,
    )


def _query_rows(fixture_root: Path, database_root: Path, query_path: Path) -> list[dict[str, Any]]:
    with tempfile.TemporaryDirectory(prefix="aslp-codeql-bqrs-") as temp_dir:
        bqrs_path = Path(temp_dir) / "source-file.bqrs"
        _run_fixture_query(fixture_root, database_root, query_path, bqrs_path)
        decoded = _run_codeql_json(["bqrs", "decode", "--format=json", str(bqrs_path)])
    return _normalized_rows(decoded, fixture_root)


def _run_codeql(args: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["codeql", *args],
        check=True,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )


def _run_codeql_json(args: list[str]) -> Any:
    completed = _run_codeql(args)
    return json.loads(completed.stdout)


def _normalized_rows(decoded: dict[str, Any], fixture_root: Path) -> list[dict[str, Any]]:
    result_set = decoded.get("#select", {})
    tuples = result_set.get("tuples", []) if isinstance(result_set, dict) else []
    return [_source_row(index, row, fixture_root) for index, row in _valid_rows(tuples)]


def _valid_rows(tuples: Any) -> list[tuple[int, list[Any]]]:
    if not isinstance(tuples, list):
        return []
    return [
        (index, row)
        for index, row in enumerate(tuples, start=1)
        if isinstance(row, list) and row
    ]


def _source_row(index: int, row: list[Any], fixture_root: Path) -> dict[str, Any]:
    relative_path = _relative_query_path(str(row[0]), fixture_root)
    return {
        "id": f"codeql-raw-file.{index}",
        "kind": "source",
        "sourceHandle": f"codeql:file:{relative_path}",
        "fields": {
            "adapterMode": "codeql-raw-dbscheme",
            "executionBackend": "codeql",
            "queryRowPath": relative_path,
            "sourceAuthority": "codeql",
        },
    }


def _relative_query_path(raw_path: str, fixture_root: Path) -> str:
    path = Path(raw_path)
    if not path.is_absolute():
        return raw_path
    try:
        return path.relative_to(fixture_root).as_posix()
    except ValueError:
        return path.name
