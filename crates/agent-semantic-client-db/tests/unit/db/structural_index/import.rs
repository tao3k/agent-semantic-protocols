use super::support::{lookup, manifest, raw_source_like_columns, structural_index, temp_root};
use agent_semantic_client_db::{ClientDb, ClientDbStructuralName};

#[test]
fn structural_index_imports_queryable_rows_without_source_text() {
    let root = temp_root("structural-index");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let stats = db
        .replace_structural_index(&structural_index(&root))
        .expect("replace structural index");
    let summary = db.summary().expect("summary");
    let symbols = db
        .lookup_structural_symbols(&lookup(&root, "parse_config"))
        .expect("lookup symbols");
    let dependencies = db
        .lookup_structural_dependency_usages(&lookup(&root, "serde_json::from_str"))
        .expect("lookup dependencies");
    let source_text_columns = raw_source_like_columns(&db_path);

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.symbol_count, 1);
    assert_eq!(stats.dependency_usage_count, 1);
    assert_eq!(summary.structural_index_generation_count, 1);
    assert_eq!(summary.structural_index_owner_count, 1);
    assert_eq!(summary.structural_index_symbol_count, 1);
    assert_eq!(summary.structural_index_dependency_usage_count, 1);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name.as_str(), "parse_config");
    assert_eq!(symbols[0].owner_path.as_str(), "src/lib.rs");
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].package_name.as_str(), "serde_json");
    assert_eq!(
        dependencies[0]
            .api_name
            .as_ref()
            .map(ClientDbStructuralName::as_str),
        Some("from_str")
    );
    assert!(source_text_columns.is_empty(), "{source_text_columns:?}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn structural_index_requires_file_hash_evidence() {
    let root = temp_root("structural-index-no-hash");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut import = structural_index(&root);

    db.import_manifest(&manifest).expect("import manifest");
    import.file_hashes.clear();
    let error = db
        .replace_structural_index(&import)
        .expect_err("reject missing hashes");

    assert!(error.contains("file hash evidence"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}
