use super::support::{
    add_cached_helper_file, lookup, manifest, manifest_with_generations, structural_index_packet,
    structural_index_packet_with_generation, structural_index_with_generation, temp_root,
};
use agent_semantic_client_db::{ClientDb, ClientDbStructuralLocator};
use serde_json::json;

#[test]
fn semantic_structural_index_packet_imports_provider_rows() {
    let root = temp_root("structural-index-packet");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let packet = serde_json::to_vec(&structural_index_packet(&root)).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    let stats = db
        .import_semantic_structural_index_packet(&manifest.generations[0], &packet)
        .expect("import structural packet");
    let symbols = db
        .lookup_structural_symbols(&lookup(&root, "crate::parse_config"))
        .expect("lookup qualified symbol");
    let dependencies = db
        .lookup_structural_dependency_usages(&lookup(&root, "serde_json::from_str"))
        .expect("lookup dependencies");

    assert_eq!(stats.owner_count, 1);
    assert_eq!(stats.symbol_count, 1);
    assert_eq!(stats.dependency_usage_count, 1);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name.as_str(), "parse_config");
    assert_eq!(
        dependencies[0]
            .source_locator
            .as_ref()
            .map(ClientDbStructuralLocator::as_str),
        Some("src/lib.rs:8:8")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_structural_index_refresh_packet_uses_asp_incremental_apply() {
    let root = temp_root("structural-index-refresh-packet");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest_with_generations(&root, &["rust-main-1", "rust-main-2"]);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");
    let mut initial_import = structural_index_with_generation(&root, "rust-main-1");
    let mut packet = structural_index_packet_with_generation(&root, "rust-main-2");

    add_cached_helper_file(&mut initial_import);
    db.import_manifest(&manifest).expect("import manifest");
    db.replace_structural_index(&initial_import)
        .expect("replace structural index");

    packet["fileHashes"] = json!([
        {
            "path": "src/lib.rs",
            "sha256": "2222222222222222222222222222222222222222222222222222222222222222",
            "byteLen": 0,
            "mtimeMs": 0,
            "source": "provider"
        },
        {
            "path": "src/unchanged.rs",
            "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "byteLen": 0,
            "mtimeMs": 0,
            "source": "provider"
        }
    ]);
    let packet = serde_json::to_vec(&packet).expect("packet bytes");
    let stats = db
        .import_semantic_structural_index_refresh_packet(&manifest.generations[1], &packet)
        .expect("import structural refresh packet");
    let copied_symbols = db
        .lookup_structural_symbols(&lookup(&root, "cached_helper"))
        .expect("lookup copied symbol");

    assert_eq!(stats.owner_count, 2);
    assert_eq!(stats.symbol_count, 2);
    assert_eq!(stats.dependency_usage_count, 2);
    assert_eq!(copied_symbols.len(), 1);
    assert_eq!(copied_symbols[0].owner_path.as_str(), "src/unchanged.rs");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_structural_index_packet_rejects_raw_source_row_fields() {
    let root = temp_root("structural-index-raw-source-row");
    let db_path = root.join("client.sqlite3");
    let manifest = manifest(&root);
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    for (rows_field, raw_source_field) in [
        ("owners", "content"),
        ("symbols", "code"),
        ("dependencyUsages", "snippet"),
    ] {
        let mut packet = structural_index_packet(&root);
        packet
            .get_mut(rows_field)
            .and_then(serde_json::Value::as_array_mut)
            .and_then(|rows| rows.first_mut())
            .and_then(serde_json::Value::as_object_mut)
            .expect("row object")
            .insert(
                raw_source_field.to_string(),
                json!("pub fn cached_source_body() {}"),
            );
        let packet = serde_json::to_vec(&packet).expect("packet bytes");
        let error = db
            .import_semantic_structural_index_packet(&manifest.generations[0], &packet)
            .expect_err("reject raw source row field");

        assert!(error.contains("raw source field"), "{error}");
        assert!(error.contains(raw_source_field), "{error}");
    }
    let _ = std::fs::remove_dir_all(root);
}
