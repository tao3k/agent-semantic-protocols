// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::runtime_server_transport_contract_digest;

#[test]
fn runtime_transport_identity_binds_control_workspace_and_provider_planes() {
    let domain = b"agent.semantic-protocols.runtime-server-transport";
    let control = include_bytes!("../../../../../schemas/runtime-server-control.v1.schema.json");
    let data_plane = include_bytes!("../../../../../schemas/workspace-db-owner-ipc.v1.schema.json");
    let performance_observation = include_bytes!(
        "../../../../../schemas/runtime-server-performance-observation.v1.schema.json"
    );
    let performance_ingress_receipt = include_bytes!(
        "../../../../../schemas/runtime-server-performance-ingress-receipt.v1.schema.json"
    );
    let provider_register_request =
        include_bytes!("../../../../../schemas/provider-register-request.schema.json");
    let provider_register_response =
        include_bytes!("../../../../../schemas/provider-register-response.schema.json");
    let asp_client_frame = include_bytes!("../../../../../schemas/asp-client-frame.schema.json");
    let mut expected = blake3::Hasher::new();
    expected.update(domain);
    for (contract_name, contract_bytes) in [
        (b"runtime-server-control".as_slice(), control.as_slice()),
        (b"workspace-db-owner-ipc".as_slice(), data_plane.as_slice()),
        (
            b"runtime-server-performance-observation".as_slice(),
            performance_observation.as_slice(),
        ),
        (
            b"runtime-server-performance-ingress-receipt".as_slice(),
            performance_ingress_receipt.as_slice(),
        ),
        (
            b"provider-register-request".as_slice(),
            provider_register_request.as_slice(),
        ),
        (
            b"provider-register-response".as_slice(),
            provider_register_response.as_slice(),
        ),
        (b"asp-client-frame".as_slice(), asp_client_frame.as_slice()),
    ] {
        expected.update(&(contract_name.len() as u64).to_le_bytes());
        expected.update(contract_name);
        expected.update(&(contract_bytes.len() as u64).to_le_bytes());
        expected.update(contract_bytes);
    }
    let expected = format!("blake3-256:{}", expected.finalize().to_hex());

    assert_eq!(runtime_server_transport_contract_digest(), expected);
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(control).to_hex()),
        "control-only identity would admit an incompatible workspace data plane"
    );
    assert_ne!(
        runtime_server_transport_contract_digest(),
        format!("blake3-256:{}", blake3::hash(data_plane).to_hex()),
        "data-plane-only identity would admit an incompatible control plane"
    );
}
