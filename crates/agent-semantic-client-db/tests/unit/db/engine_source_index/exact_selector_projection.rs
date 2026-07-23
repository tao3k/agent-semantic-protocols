use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_content_identity::canonical_item_identity::{
    CanonicalItemIdentityV1, CanonicalItemSelectorV1,
};
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
use agent_semantic_content_identity::exact_selector_merkle::{
    ExactProjectionModeV1, blake3_content_digest_v1, canonical_content_digest_v1,
};
use agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

#[test]
fn turso_round_trip_returns_only_a_validated_merkle_projection() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "asp-exact-selector-merkle-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create exact-selector test directory");
    let owner_path = "src/lib.rs";
    let selector = "rust://src/lib.rs#item/function/example";
    let canonical_item_selector = CanonicalItemSelectorV1::new(
        CanonicalItemIdentityV1::new("rust", "function", "example"),
        selector,
    );
    let source = b"fn example() {}\n";
    let source_blob_digest = blake3_content_digest_v1(source);
    let parser_identity_digest = canonical_content_digest_v1(b"parser", &[b"rs-harness"]);
    let query_pack_digest = canonical_content_digest_v1(b"query-pack", &[b"rust"]);
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([(
        owner_path.to_string(),
        source_blob_digest.clone(),
    )])
    .expect("workspace Merkle tree");
    let packet = build_exact_selector_projection_packet_v1(
        "rust",
        "rs-harness",
        canonical_item_selector,
        &parser_identity_digest,
        &query_pack_digest,
        owner_path,
        selector,
        ExactProjectionModeV1::Code,
        source,
        br#"{"kind":"fn","name":"example"}"#,
        source,
    );
    let record = packet
        .enrich_projection_record(&tree)
        .expect("enrich exact-selector packet");
    let key = ExactSelectorMerkleLookupKeyV1 {
        language_id: "rust",
        workspace_root_digest: tree.root_digest(),
        owner_path,
        owner_subtree_digest: tree
            .owner_subtree_digest(owner_path)
            .expect("owner subtree digest"),
        source_blob_digest: &source_blob_digest,
        parser_identity_digest: &parser_identity_digest,
        query_pack_digest: &query_pack_digest,
        structural_selector: selector,
        projection_mode: ExactProjectionModeV1::Code,
    };

    ClientDbEngine::persist_exact_selector_projection_v1_from_client_dir(&root, &key, &record)
        .expect("persist exact-selector projection");
    let validated =
        ClientDbEngine::lookup_exact_selector_projection_v1_from_client_dir(&root, &key)
            .expect("lookup exact-selector projection")
            .expect("persisted exact-selector projection");
    let hit = validated
        .validate_warm_hit(&key)
        .expect("validated warm hit");
    assert_eq!(hit.projection_payload, source);
    assert_eq!(hit.side_effects.parser_process_count, 0);
    assert_eq!(hit.side_effects.content_store_write_count, 0);
    assert_eq!(hit.side_effects.turso_write_count, 0);
    assert_eq!(hit.side_effects.manifest_write_count, 0);

    std::fs::remove_dir_all(root).expect("remove exact-selector test directory");
}
