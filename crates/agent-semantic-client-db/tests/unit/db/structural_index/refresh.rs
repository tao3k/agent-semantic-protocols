use super::support::{
    add_cached_helper_file, lookup, manifest, manifest_with_generations, path_strings,
    structural_index, structural_index_with_generation, temp_root,
};
use agent_semantic_client_core::ClientCacheFileHash;
use agent_semantic_client_db::ClientDb;

#[test]
fn structural_index_refresh_plan_is_asp_owned_from_file_hashes() {
    let root = temp_root("structural-index-refresh-plan");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut initial_import = structural_index(&root);

    db.import_manifest(&manifest).expect("import manifest");
    let first_plan = db
        .plan_structural_index_refresh(&initial_import)
        .expect("first refresh plan");
    assert!(first_plan.unchanged_paths.is_empty());
    assert_eq!(path_strings(&first_plan.changed_paths), vec!["src/lib.rs"]);
    assert!(first_plan.deleted_paths.is_empty());

    initial_import.file_hashes.push(ClientCacheFileHash {
        path: "src/old.rs".to_string(),
        sha256: "1".repeat(64),
        byte_len: 0,
        mtime_ms: 0,
    });
    db.replace_structural_index(&initial_import)
        .expect("replace structural index");

    let unchanged_plan = db
        .plan_structural_index_refresh(&initial_import)
        .expect("unchanged refresh plan");
    assert_eq!(
        path_strings(&unchanged_plan.unchanged_paths),
        vec!["src/lib.rs", "src/old.rs"]
    );
    assert!(unchanged_plan.changed_paths.is_empty());
    assert!(unchanged_plan.deleted_paths.is_empty());

    let mut changed_import = structural_index(&root);
    changed_import.file_hashes[0].sha256 = "2".repeat(64);
    let changed_plan = db
        .plan_structural_index_refresh(&changed_import)
        .expect("changed refresh plan");
    assert!(changed_plan.unchanged_paths.is_empty());
    assert_eq!(
        path_strings(&changed_plan.changed_paths),
        vec!["src/lib.rs"]
    );
    assert_eq!(
        path_strings(&changed_plan.deleted_paths),
        vec!["src/old.rs"]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn structural_index_refresh_apply_reuses_unchanged_rows_inside_asp() {
    let root = temp_root("structural-index-refresh-apply");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest_with_generations(&root, &["rust-main-1", "rust-main-2", "rust-main-3"]);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut initial_import = structural_index_with_generation(&root, "rust-main-1");

    db.import_manifest(&manifest).expect("import manifest");
    add_cached_helper_file(&mut initial_import);
    db.replace_structural_index(&initial_import)
        .expect("replace structural index");

    let mut refresh_import = structural_index_with_generation(&root, "rust-main-2");
    refresh_import.file_hashes[0].sha256 = "2".repeat(64);
    refresh_import.file_hashes.push(ClientCacheFileHash {
        path: "src/unchanged.rs".to_string(),
        sha256: "1".repeat(64),
        byte_len: 0,
        mtime_ms: 0,
    });
    let stats = db
        .apply_structural_index_refresh(&refresh_import)
        .expect("apply structural refresh");
    let copied_symbols = db
        .lookup_structural_symbols(&lookup(&root, "cached_helper"))
        .expect("lookup copied symbol");

    assert_eq!(stats.owner_count, 2);
    assert_eq!(stats.symbol_count, 2);
    assert_eq!(stats.dependency_usage_count, 2);
    assert_eq!(copied_symbols.len(), 1);
    assert_eq!(copied_symbols[0].owner_path.as_str(), "src/unchanged.rs");

    let mut delete_import = structural_index_with_generation(&root, "rust-main-3");
    delete_import.file_hashes[0].sha256 = "3".repeat(64);
    let stats = db
        .apply_structural_index_refresh(&delete_import)
        .expect("apply structural delete refresh");
    let deleted_symbols = db
        .lookup_structural_symbols(&lookup(&root, "cached_helper"))
        .expect("lookup deleted symbol");

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.symbol_count, 1);
    assert_eq!(stats.dependency_usage_count, 1);
    assert!(deleted_symbols.is_empty(), "{deleted_symbols:?}");
    let _ = std::fs::remove_dir_all(root);
}
