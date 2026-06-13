use super::support::{manifest, structural_index, temp_root};
use agent_semantic_client_db::ClientDb;

#[test]
fn invalidating_cache_generation_cascades_structural_index_rows() {
    let root = temp_root("structural-index-cascade");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.replace_structural_index(&structural_index(&root))
        .expect("replace structural index");
    drop(db);

    let invalidated = ClientDb::invalidate_generations(&db_path).expect("invalidate generations");
    let db = ClientDb::open_read_only_existing(&db_path)
        .expect("open db")
        .expect("db exists");
    let summary = db.summary().expect("summary");

    assert_eq!(invalidated, 1);
    assert_eq!(summary.generation_count, 0);
    assert_eq!(summary.structural_index_generation_count, 0);
    assert_eq!(summary.structural_index_owner_count, 0);
    assert_eq!(summary.structural_index_symbol_count, 0);
    assert_eq!(summary.structural_index_dependency_usage_count, 0);
    let _ = std::fs::remove_dir_all(root);
}
