"""Guard the central provider bidi-stream contract without requiring protoc."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]
PROTO = ROOT / "crates/agent-semantic-runtime-server/proto/asp-provider-stream.proto"
sys.path.insert(0, str(ROOT / "packages/python/asp_schema_manager/src"))


def test_provider_stream_contract_is_single_unversioned_bidi_service() -> None:
    text = PROTO.read_text(encoding="utf-8")
    assert 'package asp.provider.stream;' in text
    assert 'service ProviderSession {' in text
    assert len(re.findall(r"^\s*service\s+", text, re.MULTILINE)) == 1
    assert 'rpc Session(stream ProviderStreamEnvelope)' in text
    assert 'returns (stream ProviderStreamEnvelope);' in text
    assert 'package asp.provider.stream.v1' not in text
    assert 'schema_version = 2;' in text
    assert 'bytes payload = 12;' in text


def test_provider_stream_envelope_has_stable_field_tags() -> None:
    text = PROTO.read_text(encoding="utf-8")
    expected = {
        "schema_id": 1,
        "schema_version": 2,
        "session_id": 3,
        "request_id": 4,
        "workspace_identity": 5,
        "generation_digest": 6,
        "provider_id": 7,
        "language_id": 8,
        "sequence": 9,
        "kind": 10,
        "payload_schema_id": 11,
        "payload": 12,
    }
    for field, tag in expected.items():
        assert re.search(rf"\b{field}\s*=\s*{tag};", text)


def test_schema_manager_discovers_proto_as_central_contract() -> None:
    from asp_schema_manager.catalog import load_proto_contracts

    contracts, diagnostics = load_proto_contracts(ROOT)
    assert diagnostics == []
    assert [contract.relative_path for contract in contracts] == [
        "crates/agent-semantic-runtime-server/proto/asp-provider-stream.proto"
    ]
    assert contracts[0].owner_package == "agent-semantic-runtime-server"
    assert contracts[0].consumers == (
        "runtime-server:server-binding", "provider-transport:client-binding"
    )
    assert contracts[0].source_digest
    assert not (ROOT / "schemas/proto/asp-provider-stream.proto").exists()
