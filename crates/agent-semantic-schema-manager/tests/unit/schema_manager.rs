use std::fs;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;

use super::{BUNDLE_RECEIPT_FILE, SchemaManager, verify_bundle_receipt};

fn write_json(path: &Path, value: &serde_json::Value) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("encode fixture"),
    )
    .expect("write fixture");
}

fn fixture() -> (TempDir, SchemaManager) {
    let root = TempDir::new().expect("temp workspace");
    write_json(
        &root.path().join("schemas/root.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/root.schema.json",
            "title": "Fixture Root",
            "$ref": "dependency.schema.json"
        }),
    );
    write_json(
        &root.path().join("schemas/dependency.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/dependency.schema.json",
            "title": "Fixture Dependency",
            "type": "object"
        }),
    );
    write_json(
        &root.path().join("schemas/old.schema.json"),
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/old.schema.json",
            "title": "Fixture Old Contract",
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
            "families": [{
                "familyId": "asp.schema-family.fixture",
                "owner": "fixture",
                "rationale": "Owns all fixture schemas used by Schema Manager tests.",
                "priority": 100,
                "namespace": {"filenamePrefixes": [""]}
            }],
            "referenceDecisions": [],
            "wireArtifacts": {},
            "rootSets": {"contract": roots},
            "profiles": [{
                "languageId": "fixture",
                "packageRoot": "languages/fixture",
                "bundleRoot": "languages/fixture/schemas",
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
async fn resolve_bundles_returns_canonical_bytes_without_materializing_package_files() {
    let (root, manager) = fixture();
    let schema_root = root.path().join("languages/fixture/schemas");

    let bundles = manager.resolve_bundles(&[]).await.expect("resolve bundles");

    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].language_id, "fixture");
    assert_eq!(bundles[0].root_set_ids, ["contract"]);
    assert_eq!(bundles[0].schemas.len(), 3);
    assert!(
        bundles[0]
            .schemas
            .iter()
            .all(|schema| !schema.bytes.is_empty())
    );
    assert!(!schema_root.join("root.schema.json").exists());
    assert!(!schema_root.join("dependency.schema.json").exists());
    assert!(!schema_root.join(BUNDLE_RECEIPT_FILE).exists());
}

#[tokio::test]
async fn materialize_replaces_a_stale_package_copy_from_the_canonical_root() {
    let (root, manager) = fixture();
    manager.materialize(&[]).await.expect("initial materialize");

    let canonical_path = root.path().join("schemas/root.schema.json");
    let package_path = root
        .path()
        .join("languages/fixture/schemas/root.schema.json");
    write_json(
        &canonical_path,
        &json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://example.invalid/root.schema.json",
            "title": "Updated Canonical Root",
            "$ref": "dependency.schema.json"
        }),
    );

    let reports = manager.materialize(&[]).await.expect("refresh materialize");
    assert_eq!(reports[0].changed_count, 3);
    assert_eq!(
        fs::read(&package_path).expect("read package projection"),
        fs::read(&canonical_path).expect("read canonical root")
    );
    manager.verify(&[]).await.expect("verify refreshed bundle");
}

#[tokio::test]
async fn verify_rejects_legacy_bundle_receipt_shape() {
    let (root, manager) = fixture();
    manager.materialize(&[]).await.expect("materialize");
    let receipt_path = root
        .path()
        .join("languages/fixture/schemas")
        .join(BUNDLE_RECEIPT_FILE);
    write_json(
        &receipt_path,
        &json!({
            "schemaId": "agent.semantic-protocols.language-schema-bundle-receipt",
            "schemaVersion": "1",
            "languageId": "fixture",
            "profileDigest": "legacy",
            "bundleDigest": "legacy",
            "schemas": []
        }),
    );

    let error = manager
        .verify(&[])
        .await
        .expect_err("legacy receipt must fail closed");
    assert!(error.contains("decode schema bundle receipt"), "{error}");
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
        root.path()
            .join("languages/fixture/schemas/root.schema.json"),
        b"{}",
    )
    .expect("drift materialized schema");

    let error = manager.verify(&[]).await.expect_err("drift must fail");
    assert!(
        error.contains("materialized schema digest drift"),
        "{error}"
    );
}

#[tokio::test]
async fn publishes_portable_client_bundle_outside_language_package() {
    let (root, manager) = fixture();
    let output = root.path().join("downstream-client/schemas");

    let report = manager
        .publish_client_bundle("fixture".to_owned(), &output)
        .await
        .expect("publish portable client bundle");

    assert_eq!(report.schema_count, 3);
    assert!(output.join("root.schema.json").is_file());
    assert!(output.join("dependency.schema.json").is_file());
    assert!(!output.join("private.schema.json").exists());
    let receipt = verify_bundle_receipt(output.join(BUNDLE_RECEIPT_FILE))
        .await
        .expect("verify downstream receipt");
    assert_eq!(receipt.language_id, "fixture");
    assert_eq!(receipt.bundle_digest, report.bundle_digest);
}

#[tokio::test]
async fn registry_fails_closed_when_any_canonical_schema_has_no_responsibility() {
    let (root, manager) = fixture();
    let registry_path = root.path().join("profiles.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&registry_path).expect("read registry"))
            .expect("decode registry");
    registry["families"][0]["namespace"]["filenamePrefixes"] = json!(["root."]);
    write_json(&registry_path, &registry);

    let error = manager
        .verify(&[])
        .await
        .expect_err("unowned canonical schema must fail");
    assert!(
        error.contains("schema responsibility is undeclared"),
        "{error}"
    );
}

#[tokio::test]
async fn registry_fails_closed_on_ambiguous_schema_responsibility() {
    let (root, manager) = fixture();
    let registry_path = root.path().join("profiles.json");
    let mut registry: serde_json::Value =
        serde_json::from_slice(&fs::read(&registry_path).expect("read registry"))
            .expect("decode registry");
    registry["families"]
        .as_array_mut()
        .expect("families")
        .push(json!({
            "familyId": "asp.schema-family.fixture-shadow",
            "owner": "fixture-shadow",
            "rationale": "Deliberately conflicts with the fixture family.",
            "priority": 100,
            "namespace": {"filenamePrefixes": [""]}
        }));
    write_json(&registry_path, &registry);

    let error = manager
        .verify(&[])
        .await
        .expect_err("ambiguous responsibility must fail");
    assert!(
        error.contains("schema responsibility is ambiguous"),
        "{error}"
    );
}

#[tokio::test]
async fn registry_fails_closed_on_duplicate_canonical_schema_id() {
    let (root, manager) = fixture();
    let old_path = root.path().join("schemas/old.schema.json");
    let mut old: serde_json::Value =
        serde_json::from_slice(&fs::read(&old_path).expect("read old schema"))
            .expect("decode old schema");
    old["$id"] = json!("https://example.invalid/root.schema.json");
    write_json(&old_path, &old);

    let error = manager
        .verify(&[])
        .await
        .expect_err("duplicate schema id must fail");
    assert!(error.contains("duplicate canonical schema $id"), "{error}");
}

#[tokio::test]
async fn canonical_registry_verifies_every_registered_language_bundle() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let reports = SchemaManager::new(workspace)
        .verify(&[])
        .await
        .expect("verify canonical language bundles");
    let language_ids = reports
        .into_iter()
        .map(|report| report.language_id)
        .collect::<Vec<_>>();

    assert_eq!(
        language_ids,
        [
            "rust",
            "typescript",
            "python",
            "julia",
            "gerbil-scheme",
            "org",
            "md",
        ]
    );
}

#[test]
fn canonical_client_profile_publishes_the_shared_schema_bundle_route() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let registry: serde_json::Value = serde_json::from_slice(
        &fs::read(workspace.join("schemas/language-schema-profiles.json"))
            .expect("read canonical schema profile registry"),
    )
    .expect("decode canonical schema profile registry");
    let client_roots = registry["rootSets"]["client-protocol"]
        .as_array()
        .expect("client-protocol root set")
        .iter()
        .map(|value| value.as_str().expect("schema root name"))
        .collect::<Vec<_>>();

    assert!(client_roots.contains(&"asp-client-schema-bundle-request.schema.json"));
    assert!(client_roots.contains(&"asp-client-schema-bundle-response.schema.json"));
    assert!(client_roots.contains(&"semantic-agent-search-playbook-receipt.v1.schema.json"));
    assert!(client_roots.contains(&"large-search-playbook-performance-receipt.v1.schema.json"));
    assert!(
        registry["profiles"]
            .as_array()
            .expect("registered profiles")
            .iter()
            .all(|profile| profile.get("profileId").is_none()),
        "canonical profiles are bound by languageId and rootSets, not an invented profileId"
    );
    assert!(
        registry["profiles"]
            .as_array()
            .expect("registered profiles")
            .iter()
            .all(|profile| profile["rootSets"]
                .as_array()
                .is_some_and(|root_sets| root_sets.iter().any(|root| root == "client-protocol"))),
        "every registered language profile must consume the shared client-protocol root set"
    );
}

#[test]
fn every_language_profile_receives_the_resident_graph_contract_declaratively() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let registry: serde_json::Value = serde_json::from_slice(
        &fs::read(workspace.join("schemas/language-schema-profiles.json"))
            .expect("read canonical schema profile registry"),
    )
    .expect("decode canonical schema profile registry");
    let reasoning_roots = registry["rootSets"]["agent-reasoning"]
        .as_array()
        .expect("agent-reasoning root set");
    for schema in [
        "semantic-graph-resident-evaluation-request.v1.schema.json",
        "semantic-graph-resident-evaluation-result.v1.schema.json",
        "python-generation-graph-performance-receipt.v1.schema.json",
    ] {
        assert!(
            reasoning_roots.iter().any(|entry| entry == schema),
            "missing {schema}"
        );
    }
    assert!(
        registry["profiles"]
            .as_array()
            .expect("registered profiles")
            .iter()
            .all(|profile| profile["rootSets"]
                .as_array()
                .is_some_and(|roots| roots.iter().any(|root| root == "agent-reasoning"))),
        "every language must consume the shared resident graph contract"
    );
}

#[test]
fn canonical_client_protocol_wire_artifact_has_one_schema_manager_authority() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let canonical = SchemaManager::new(workspace)
        .canonical_wire_artifact_path("asp-client-protocol-v1")
        .expect("resolve canonical ASP Client Protocol protobuf");

    assert_eq!(
        canonical,
        workspace.join("schemas/asp-client-protocol.v1.proto")
    );
    assert!(canonical.is_file());
    assert!(
        !workspace
            .join("crates/agent-semantic-client-server/proto/asp-client-protocol.proto")
            .exists(),
        "transport package must not retain a private protobuf authority"
    );
}
#[test]
fn repository_schema_family_registry_never_names_a_missing_schema() {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let profiles = crate::SchemaManager::new(&workspace)
        .registered_language_profiles()
        .expect("the canonical schema family registry must be complete");
    assert!(!profiles.is_empty());
}
