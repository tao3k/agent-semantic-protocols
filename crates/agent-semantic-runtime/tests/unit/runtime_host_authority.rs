use std::path::Path;

#[test]
fn runtime_host_authority_has_no_legacy_polling_lane() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_dir = manifest_dir.join("src");
    assert!(!source_dir.join("codex_app_server_sessions.rs").exists());
    assert!(
        !source_dir
            .join("codex_app_server_control_plane.rs")
            .exists()
    );

    let forbidden = [
        "CODEX_HOST_RUNTIME_CACHE_TTL_SECONDS",
        "ASP_CODEX_APP_SERVER_TIMEOUT_MS",
        "thread/resume",
        "app-server\", \"--stdio",
    ];
    for entry in std::fs::read_dir(&source_dir).expect("read Runtime source directory") {
        let path = entry.expect("read Runtime source entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read Runtime Rust source");
        for token in forbidden {
            assert!(
                !source.contains(token),
                "Runtime source {} retained legacy Host polling token {token}",
                path.display()
            );
        }
    }
}
