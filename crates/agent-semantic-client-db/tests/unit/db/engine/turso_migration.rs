#[test]
fn active_turso_0_7_migration_preserves_non_db_artifacts_and_is_idempotent() {
    use agent_semantic_client_db::engine::ClientDbTurso07ActiveMigration;

    let root = temp_root("active-turso-0-7-migration");
    let workspace = root.join("workspace");
    let client_dir = workspace.join("live/client");
    std::fs::create_dir_all(&workspace).expect("create migration workspace");
    let manifest = ClientCacheManifest {
        schema_id: "agent.semantic-protocols.client-cache-manifest".into(),
        schema_version: "1".into(),
        protocol_id: "agent.semantic-protocols.client-cache".into(),
        protocol_version: "1".into(),
        cache_root: agent_semantic_client_core::ClientCachePath::from_path(&client_dir),
        generations: Vec::new(),
    };
    let mut writer =
        ClientDbEngine::open_write_session_client_dir(&client_dir).expect("create source Turso DB");
    writer
        .import_manifest(&manifest)
        .expect("seed source cache manifest");
    drop(writer);

    let db_path = ClientDbEngine::turso_path_for_client_dir(&client_dir);
    let search_db_path = client_dir.join("search-projection.turso");
    std::fs::remove_file(client_dir.join("facts.turso.format.v1.json"))
        .expect("remove source facts receipt");
    std::fs::remove_file(client_dir.join("search-projection.turso.format.v1.json"))
        .expect("remove source search receipt");
    let source_db_bytes = std::fs::read(&db_path).expect("snapshot source facts DB");
    let source_search_bytes =
        std::fs::read(&search_db_path).expect("snapshot source search DB");
    let sentinel_path = client_dir.join("source-blob-cas");
    std::fs::create_dir(&sentinel_path).expect("create non-DB sentinel dir");
    std::fs::write(sentinel_path.join("sentinel"), b"preserve")
        .expect("write non-DB sentinel");

    let migrated = ClientDbEngine::migrate_active_project_client_dir_to_turso_0_7(&client_dir)
        .expect("migrate active client dir");
    let ClientDbTurso07ActiveMigration::Migrated {
        report,
        rollback_client_dir,
    } = migrated
    else {
        panic!("unreceipted active DB must be migrated");
    };
    assert_eq!(report.target_client_dir, client_dir);
    assert_eq!(
        std::fs::read(sentinel_path.join("sentinel")).expect("read preserved sentinel"),
        b"preserve"
    );
    assert_eq!(
        std::fs::read(rollback_client_dir.join("facts.turso"))
            .expect("read rollback facts DB"),
        source_db_bytes
    );
    assert_eq!(
        std::fs::read(rollback_client_dir.join("search-projection.turso"))
            .expect("read rollback search DB"),
        source_search_bytes
    );
    assert!(report.format_receipt_path.is_file());
    assert!(report.search_projection_format_receipt_path.is_file());
    assert!(report.migration_receipt_path.is_file());
    assert!(
        ClientDbEngine::open_read_session_client_dir(&client_dir)
            .expect("open migrated DB")
            .is_some()
    );

    let second = ClientDbEngine::migrate_active_project_client_dir_to_turso_0_7(&client_dir)
        .expect("repeat migration");
    assert!(matches!(
        second,
        ClientDbTurso07ActiveMigration::AlreadyCurrent { .. }
    ));

    std::fs::remove_dir_all(root).expect("remove migration fixture");
}
