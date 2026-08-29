"""Unit tests for the runtime-server-owned protobuf catalog."""

import hashlib
from pathlib import Path

from asp_schema_manager.catalog import load_proto_contracts


PROTO_RELATIVE_PATH = Path(
    "crates/agent-semantic-runtime-server/proto/asp-provider-stream.proto"
)


def test_load_proto_contracts_returns_stable_owner_receipt(tmp_path: Path) -> None:
    source = b"syntax = \"proto3\";\n"
    proto_path = tmp_path / PROTO_RELATIVE_PATH
    proto_path.parent.mkdir(parents=True)
    proto_path.write_bytes(source)

    contracts, diagnostics = load_proto_contracts(tmp_path)

    assert diagnostics == []
    assert len(contracts) == 1
    contract = contracts[0]
    assert contract.relative_path == PROTO_RELATIVE_PATH.as_posix()
    assert contract.owner_package == "agent-semantic-runtime-server"
    assert contract.consumers == (
        "runtime-server:server-binding",
        "provider-transport:client-binding",
    )
    assert contract.source_digest == f"sha256:{hashlib.sha256(source).hexdigest()}"


def test_load_proto_contracts_does_not_scan_schema_copies(tmp_path: Path) -> None:
    copied = tmp_path / "schemas/proto/asp-provider-stream.proto"
    copied.parent.mkdir(parents=True)
    copied.write_text("copied", encoding="utf-8")

    contracts, diagnostics = load_proto_contracts(tmp_path)

    assert contracts == []
    assert diagnostics[0]["code"] == "missing-proto-contract"
