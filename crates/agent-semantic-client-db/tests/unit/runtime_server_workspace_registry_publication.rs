static NEXT_TEST_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[tokio::test(flavor = "current_thread")]
async fn prepared_workspace_scope_waits_for_writer_lane_readiness() {
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        NEXT_TEST_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    );
    let root = std::env::temp_dir().join(format!("asp-writer-ready-{nonce}"));
    let project_root = root.join("project");
    let registry =
        crate::runtime_server_workspace::RuntimeServerWorkspaceRegistry::new(root.clone())
            .expect("create runtime workspace registry");

    let prepared = registry
        .prepare_resident_workspace_scope("workspace-writer-ready", &project_root)
        .await
        .expect("prepare resident workspace scope");

    assert_eq!(prepared, project_root);
    registry
        .shutdown()
        .await
        .expect("shutdown prepared workspace registry");
    let _ = tokio::fs::remove_dir_all(root).await;
}
