#[test]
fn spawn_nonce_is_typed_and_process_local() {
    let first = super::spawn_nonce();
    let second = super::spawn_nonce();
    assert!(first.starts_with("blake3-256:"));
    assert_eq!(first.len(), "blake3-256:".len() + 64);
    assert_ne!(first, second);
}

#[tokio::test(flavor = "current_thread")]
async fn live_spawn_receipt_is_the_supervisor_single_flight_authority() {
    let root = std::env::temp_dir().join(format!(
        "asp-global-server-spawn-authority-{}-{}",
        std::process::id(),
        super::spawn_nonce()
    ));
    let server_dir = root.join("runtime").join("server");
    tokio::fs::create_dir_all(&server_dir)
        .await
        .expect("create lifecycle fixture");
    let receipt = super::RuntimeServerSpawnReceipt {
        schema_id: "agent.semantic-protocols.runtime-server-owner-spawn.v1".to_owned(),
        schema_version: "1".to_owned(),
        process_id: std::process::id(),
        nonce: super::spawn_nonce(),
        state_home: root.to_string_lossy().into_owned(),
        runtime_artifact_path: "/fixture/asp".to_owned(),
    };
    tokio::fs::write(
        super::spawn_receipt_path(&root),
        serde_json::to_vec(&receipt).expect("encode spawn receipt"),
    )
    .await
    .expect("publish spawn receipt");

    let outcome = super::ensure_runtime_server(&root, false)
        .await
        .expect("existing spawn receipt is reusable");
    assert!(outcome.is_none(), "must not spawn a competing daemon");

    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove lifecycle fixture");
}
