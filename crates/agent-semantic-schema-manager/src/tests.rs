use std::fs;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;

use super::{BUNDLE_RECEIPT_FILE, SchemaManager, verify_bundle_receipt};

fn write_json(path: &Path, value: &serde_json::Value) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(path, serde_json::to_vec_pretty(value).expect("encode fixture"))
        .expect("write fixture");
}

fn fixture() -> (TempDir, SchemaManager) {
    let root = TempDir::new().expect("temp workspace");
    write_json(
        &root.path().join("schemas/root.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/root.schema.json",
            "$ref": "dependency.schema.json"
        }),
    );
    write_json(
        &root.path().join("schemas/dependency.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/dependency.schema.json",
            "type": "object"
        }),
    );
    write_json(
        &root.path().join("schemas/old.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/old.schema.json",
            "type": "string"
        }),
    );
    let provider_schema_root = root.path().join("languages/fixture/schemas");
    write_json(
        &provider_schema_root.join("private.schema.json"),
        &json!({"type": "object"}),
    );
    write_json(
        &provider_schema_root.join("unmanaged.schema.json"),
        &json!({"type": "boolean"}),
    );
    let registry_path = root.path().join("profiles.json");
    write_registry(&registry_path, &["root.schema.json", "old.schema.json"]);
    let manager = SchemaManager::with_registry(root.path(), &registry_path);
    (root, manager)
}

fn write_registry(path: &Path, roots: &[&str]) {
    write_json(
        path,
        &json!({
            "$schema": "language-schema-profile-registry.schema.json",
            "schemaId": "agent.semantic-protocols.language-schema-profile-registry",
            "schemaVersion": "1",
            "rootSets": {"contract": roots},
            "profiles": [{
                "languageId": "fixture",
                "packageRoot": "languages/fixture",
                "rootSets": ["contract"],
                "roots": [],
                "providerOwned": ["private.schema.json"]
            }]
        }),
    );
}

#[tokio::test]
async fn materialize_resolves_closure_and_verify_is_read_only() {
    let (root, manager) = fixture();
    let reports = manager.materialize(&[]).await.expect("materialize");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].schema_count, 3);
    let schema_root = root.path().join("languages/fixture/schemas");
    assert!(schema_root.join("root.schema.json").is_file());
    assert!(schema_root.join("dependency.schema.json").is_file());
    assert!(schema_root.join(BUNDLE_RECEIPT_FILE).is_file());

    let verified = manager.verify(&[]).await.expect("verify");
    assert_eq!(verified[0].changed_count, 0);
    assert_eq!(verified[0].bundle_digest, reports[0].bundle_digest);
    let portable = verify_bundle_receipt(schema_root.join(BUNDLE_RECEIPT_FILE))
        .await
        .expect("portable receipt verification");
    assert_eq!(portable.language_id, "fixture");
}

#[tokio::test]
async fn manager_never_removes_provider_owned_or_unmanaged_schemas() {
    let (root, manager) = fixture();
    manager.materialize(&[]).await.expect("initial materialize");
    write_registry(&root.path().join("profiles.json"), &["root.schema.json"]);

    let reports = manager.materialize(&[]).await.expect("second materialize");
    let schema_root = root.path().join("languages/fixture/schemas");
    assert_eq!(reports[0].removed_count, 1);
    assert!(!schema_root.join("old.schema.json").exists());
    assert!(schema_root.join("private.schema.json").is_file());
    assert!(schema_root.join("unmanaged.schema.json").is_file());
}

#[tokio::test]
async fn verify_fails_closed_on_materialized_schema_drift() {
    let (root, manager) = fixture();
    manager.materialize(&[]).await.expect("materialize");
    fs::write(
        root.path().join("languages/fixture/schemas/root.schema.json"),
        b"{}",
    )
    .expect("drift materialized schema");

    let error = manager.verify(&[]).await.expect_err("drift must fail");
    assert!(error.contains("materialized schema digest drift"), "{error}");
}
