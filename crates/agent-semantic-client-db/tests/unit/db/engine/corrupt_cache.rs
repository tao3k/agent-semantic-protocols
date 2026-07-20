#[test]
fn invalidate_rebuilds_a_corrupt_derived_turso_cache() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-client-db-corrupt-cache-{}-{nonce}",
        std::process::id()
    ));
    let client_dir = root.join("client");
    let project_root = root.join("project");
    std::fs::create_dir_all(&client_dir).expect("create client directory");
    std::fs::create_dir_all(&project_root).expect("create project directory");

    let db_path = client_dir.join("client.turso");
    std::fs::write(&db_path, b"not-a-valid-derived-turso-cache")
        .expect("write corrupt derived cache");
    let mut sidecar_paths = Vec::new();
    for suffix in ["-wal", "-shm", "-tshm"] {
        let mut sidecar = db_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let sidecar = std::path::PathBuf::from(sidecar);
        std::fs::write(&sidecar, b"corrupt-sidecar").expect("write corrupt cache sidecar");
        sidecar_paths.push(sidecar);
    }

    let invalidated = agent_semantic_client_db::ClientDbEngine::invalidate_generations_for_project_from_client_dir(
        &client_dir,
        &project_root,
    )
    .expect("invalidate should rebuild a corrupt derived cache");
    assert_eq!(invalidated, 0);
    assert!(db_path.exists(), "derived cache should be recreated");
    for sidecar in &sidecar_paths {
        match std::fs::read(sidecar) {
            Ok(bytes) => assert_ne!(
                bytes, b"corrupt-sidecar",
                "a recreated sidecar must not retain corrupt input bytes"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("failed to inspect `{}`: {error}", sidecar.display()),
        }
    }

    let second = agent_semantic_client_db::ClientDbEngine::invalidate_generations_for_project_from_client_dir(
        &client_dir,
        &project_root,
    )
    .expect("recreated derived cache should remain usable");
    assert_eq!(second, 0);

    let _ = std::fs::remove_dir_all(root);
}
