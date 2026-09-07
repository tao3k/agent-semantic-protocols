// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::WorkspaceRuntimeSelectorRead;

#[test]
fn provider_projection_has_a_generation_independent_v1_wire_shape() {
    let receipt = WorkspaceRuntimeSelectorRead::ProviderProjection {
        owner_content_digest: "blake3-256:owner".to_owned(),
        resolved_selector: "rust://src/lib.rs#item/function/live".to_owned(),
        bytes: b"fn live() {}".to_vec(),
    };

    let encoded = serde_json::to_value(&receipt).expect("provider projection must serialize");
    assert_eq!(encoded["state"], "provider-projection");
    assert_eq!(encoded["ownerContentDigest"], "blake3-256:owner");
    assert_eq!(
        encoded["resolvedSelector"],
        "rust://src/lib.rs#item/function/live"
    );
    assert!(encoded.get("generationDigest").is_none());
    assert!(encoded.get("rootDigest").is_none());

    let decoded: WorkspaceRuntimeSelectorRead =
        serde_json::from_value(encoded).expect("provider projection must deserialize");
    assert_eq!(decoded, receipt);
}
