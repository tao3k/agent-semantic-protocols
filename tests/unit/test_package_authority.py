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
