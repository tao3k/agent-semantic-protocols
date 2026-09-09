use super::cleanup_invalid_runtime_server_endpoint;

#[tokio::test]
async fn invalid_endpoint_cleanup_uses_its_content_bound_status_identity() {
    let fixture = tempfile::tempdir().expect("invalid endpoint cleanup fixture");
    let state_home = fixture.path().join("state");
    tokio::fs::create_dir_all(&state_home)
        .await
        .expect("create State Home");
    let runtime_base = super::super::endpoint_identity::runtime_server_runtime_base(&state_home)
        .expect("derive Runtime serving root");
    tokio::fs::create_dir_all(&runtime_base)
        .await
        .expect("create Runtime serving root");
    let endpoint_path = super::super::endpoint_identity::runtime_server_endpoint_path(&state_home)
        .expect("derive endpoint path");
    let owner_epoch = 7_u64;
    let binding_token = "binding-current";
    let runtime_binary_digest = format!("blake3-256:{}", "a".repeat(64));
    let status_identity =
        blake3::hash(format!("{owner_epoch}\0{binding_token}\0{runtime_binary_digest}").as_bytes())
            .to_hex();
    let status_path = runtime_base.join(format!("status-{}.memory", &status_identity[..16]));
    let unrelated_status_path = runtime_base.join("status-unrelated.memory");
    tokio::fs::write(&status_path, b"stale status")
        .await
        .expect("write content-bound status");
    tokio::fs::write(&unrelated_status_path, b"unrelated status")
        .await
        .expect("write unrelated status");
    tokio::fs::write(
        &endpoint_path,
        serde_json::to_vec(&serde_json::json!({
            "ownerEpoch": owner_epoch,
            "bindingToken": binding_token,
            "runtimeBinaryIdentity": { "value": runtime_binary_digest },
            "invalid": true
        }))
        .expect("encode invalid endpoint"),
    )
    .await
    .expect("write invalid endpoint");

    cleanup_invalid_runtime_server_endpoint(&state_home)
        .await
        .expect("clean invalid endpoint");

    assert!(
        !endpoint_path.exists(),
        "invalid endpoint receipt is removed"
    );
    assert!(
        !status_path.exists(),
        "the content-bound stale status memory is removed"
    );
    assert!(
        unrelated_status_path.exists(),
        "cleanup cannot remove another identity's status memory"
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
