#[tokio::test(flavor = "current_thread")]
async fn hook_ensure_preserves_operator_stop_without_spawning() {
    let root = std::env::temp_dir().join(format!(
        "asp-global-server-operator-stop-{}-{}",
        std::process::id(),
        super::spawn_nonce()
    ));
    super::mark_runtime_server_operator_stopped(&root)
        .await
        .expect("publish operator stop");

    super::ensure_runtime_server_for_hook(&root).expect("operator stop is a Hook no-op");

    assert!(!super::spawn_receipt_path(&root).exists());
    tokio::fs::remove_dir_all(root)
        .await
        .expect("remove lifecycle fixture");
}

#[test]
fn spawn_nonce_is_typed_and_process_local() {
    let first = super::spawn_nonce();
    let second = super::spawn_nonce();
    assert!(first.starts_with("blake3-256:"));
    assert_eq!(first.len(), "blake3-256:".len() + 64);
    assert_ne!(first, second);
}
