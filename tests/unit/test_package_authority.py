"""Current package authority and dependency-DAG contract."""

from pathlib import Path
import tomllib


ROOT = Path(__file__).resolve().parents[2]
BATCH_GATES = {"runtime-server-package-extraction": "pending"}


def package(name: str) -> dict:
    path = next(ROOT.glob(f"crates/{name}/Cargo.toml"))
    return tomllib.loads(path.read_text())


def deps(name: str) -> set[str]:
    return set(package(name).get("dependencies", {}))


def test_transport_and_client_authorities_have_no_db_cycle() -> None:
    assert "agent-semantic-client-db" not in deps("agent-semantic-client-server")
    assert "agent-semantic-client-protocol" in deps("agent-semantic-client-server")
    assert "agent-semantic-client-server" in deps("agent-semantic-client")
    assert (ROOT / "crates/agent-semantic-client/src/runtime_language_client.rs").is_file()
    assert not (ROOT / "crates/agent-semantic-protocol/src/command/runtime_http_client.rs").exists()


def test_runtime_server_extraction_is_explicit_batch_gate() -> None:
    runtime_server_manifest = ROOT / "crates/agent-semantic-runtime-server/Cargo.toml"
    if runtime_server_manifest.exists():
        manifest = tomllib.loads(runtime_server_manifest.read_text())
        assert "agent-semantic-protocol" not in manifest.get("dependencies", {})
    else:
        assert BATCH_GATES["runtime-server-package-extraction"] == "pending"


def test_protocol_is_not_declared_as_client_server_runtime_owner() -> None:
    protocol_sources = list((ROOT / "crates/agent-semantic-protocol/src").rglob("*.rs"))
    assert not any(path.name == "runtime_http_client.rs" for path in protocol_sources)
    assert not (ROOT / "crates/agent-semantic-protocol/src/server/runtime_server_endpoint_io.rs").exists()


def test_source_index_owner_is_db_package_not_client_package() -> None:
    client_source_index = ROOT / "crates/agent-semantic-client/src/source_index"
    db_server_source_index = ROOT / "crates/agent-semantic-client-db/src/server_source_index"
    client_manifest = package("agent-semantic-client")
    db_manifest = package("agent-semantic-client-db")
    assert not client_source_index.exists()
    assert (db_server_source_index / "generation.rs").is_file()
    assert (db_server_source_index / "projection.rs").is_file()
    assert (db_server_source_index / "provider_envelope.rs").is_file()
    assert "agent-semantic-client-db" in client_manifest["dependencies"]
    assert "agent-semantic-client-server" in db_manifest["dependencies"]
    assert "ASP Server-owned Turso/Merkle DB Engine" in db_manifest["description"]
    assert "pub mod server_source_index;" in (
        ROOT / "crates/agent-semantic-client-db/src/lib.rs"
    ).read_text()
    client_sources = list((ROOT / "crates/agent-semantic-client/src").rglob("*.rs"))
    assert not any(
        "agent_semantic_client::source_index" in path.read_text() for path in client_sources
    )


def test_python_graphs_has_only_server_managed_runtime_entrypoint() -> None:
    graphs_root = ROOT / "packages/python/asp_python_graphs/src/asp_python_graphs"
    assert not (graphs_root / "__main__.py").exists()
    assert (graphs_root / "service_cli.py").is_file()
    service_cli = (graphs_root / "service_cli.py").read_text()
    assert "serve" in service_cli
