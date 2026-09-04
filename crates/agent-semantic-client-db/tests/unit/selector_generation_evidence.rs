use std::path::PathBuf;

use agent_semantic_client_core::ProviderId;
use agent_semantic_client_db::ClientDbSourceIndexPath;
use agent_semantic_client_db::ClientDbSourceIndexQueryKey;
use agent_semantic_client_db::ClientDbSourceIndexScopeFile;
use agent_semantic_client_db::ClientDbSourceIndexSelector;
use agent_semantic_client_db::ClientDbSourceIndexSelectorId;
use agent_semantic_client_db::ClientDbSourceIndexSelectorKind;
use agent_semantic_client_db::ClientDbSourceIndexSelectorSymbol;
use agent_semantic_client_db::ClientDbSourceIndexSource;
use agent_semantic_client_db::ClientDbSourceIndexSourceBlobs;
use agent_semantic_client_db::source_index_file_hashes;

fn selector(selector_id: &str) -> ClientDbSourceIndexSelector {
    let (item_kind, source): (&str, &[u8]) = if selector_id.contains("#item/method/") {
        ("method", b"impl Owner { fn target(&self) {} }\n")
    } else {
        ("function", b"pub fn target() {}\n")
    };
    ClientDbSourceIndexSelector {
        owner_path: "src/lib.rs".into(),
        provider_id: ProviderId::from("asp-rust"),
        selector_id: ClientDbSourceIndexSelectorId::from(selector_id),
        symbol: Some(ClientDbSourceIndexSelectorSymbol::from("target")),
        kind: Some(ClientDbSourceIndexSelectorKind::from(item_kind)),
        source: ClientDbSourceIndexSource::from("parser"),
        query_keys: vec![ClientDbSourceIndexQueryKey::from("target")],
        derived_projections: Vec::new(),
        projection_record: crate::projection_fixture::projection_record(
            crate::projection_fixture::ProjectionFixtureInput {
                language_id: "rust",
                provider_id: "parser",
                owner_path: "src/lib.rs",
                structural_selector: selector_id,
                item_kind,
                item_name: "target",
                source,
                source_byte_start: 0,
                source_byte_end: source.len() as u64,
            },
        ),
    }
}

fn selector_generation_hash(selectors: Vec<ClientDbSourceIndexSelector>) -> String {
    let fixture = crate::test_support::TestDir::new("selector-generation-evidence");
    let root = fixture.path().join("project");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let bytes = b"pub fn target() {}\n".to_vec();
    std::fs::write(root.join("src/lib.rs"), &bytes).unwrap();
    let files = [ClientDbSourceIndexScopeFile {
        relations: Vec::new(),
        path: PathBuf::from("src/lib.rs"),
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
        projection_coverage:
            agent_semantic_client_db::ClientDbSourceIndexProjectionCoverage::Complete,
        projection_diagnostic: None,
        selector_receipts: selectors,
    }];
    let source_blobs = ClientDbSourceIndexSourceBlobs::from_normalized(vec![(
        ClientDbSourceIndexPath::new("src/lib.rs"),
        bytes,
    )]);
    let hash =
        source_index_file_hashes(&root, &files, &source_blobs, "provider-registry-v1", ["."])
            .unwrap()
            .into_iter()
            .find(|entry| entry.path == "@scope/selector-generation/src/lib.rs")
            .expect("selector generation evidence hash")
            .sha256;
    hash
}

#[test]
fn selector_generation_hash_binds_typed_selector_identity() {
    let first = selector_generation_hash(vec![selector("rust://src/lib.rs#item/function/target")]);
    let second = selector_generation_hash(vec![selector(
        "rust://src/lib.rs#item/method/target/scope/implementation-owner/type/Owner",
    )]);
    assert_ne!(first, second);
}

#[test]
fn selector_generation_hash_is_order_independent() {
    let first = selector_generation_hash(vec![
        selector("rust://src/lib.rs#item/function/a"),
        selector("rust://src/lib.rs#item/function/b"),
    ]);
    let second = selector_generation_hash(vec![
        selector("rust://src/lib.rs#item/function/b"),
        selector("rust://src/lib.rs#item/function/a"),
    ]);
    assert_eq!(first, second);
}
