from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]

ACTIVE_SURFACES = [
    REPO_ROOT / "crates" / "agent-semantic-client-db" / "src",
    REPO_ROOT / "crates" / "agent-semantic-client" / "src",
    REPO_ROOT / "crates" / "agent-semantic-client-core" / "src",
    REPO_ROOT / "crates" / "agent-semantic-runtime" / "src" / "state_core.rs",
    REPO_ROOT / "crates" / "agent-semantic-protocol" / "src" / "state_cli.rs",
    REPO_ROOT / "schemas" / "semantic-db-engine-report.v1.schema.json",
    REPO_ROOT / "schemas" / "semantic-db-engine-manifest.v1.schema.json",
    REPO_ROOT / "schemas" / "agent-semantic-client-receipt.v1.schema.json",
    REPO_ROOT / "schemas" / "semantic-state-locate-report.v1.schema.json",
    REPO_ROOT / "schemas" / "semantic-state-locate-report.v2.schema.json",
]

RETIRED_BACKEND_TOKENS = [
    "rusqlite",
    "sqlite-v1",
    "sqlite",
    "futureBackend",
    "futureBackendReport",
    "future_backend",
    "future_backend_report",
    "turso-backend",
]


def _active_files():
    for surface in ACTIVE_SURFACES:
        if surface.is_file():
            yield surface
        else:
            yield from (path for path in surface.rglob("*") if path.is_file())


def test_active_db_engine_surfaces_do_not_expose_retired_backend_tokens() -> None:
    violations = []
    for path in _active_files():
        text = path.read_text(errors="ignore")
        for token in RETIRED_BACKEND_TOKENS:
            if token.lower() in text.lower():
                violations.append(f"{path.relative_to(REPO_ROOT)} contains {token!r}")

    assert not violations, "\n".join(violations)
