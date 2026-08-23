use agent_semantic_client_db::ClientDbEngine;
use agent_semantic_content_identity::canonical_item_identity::{
    CanonicalItemIdentity, CanonicalItemSelector,
};
use agent_semantic_content_identity::exact_selector_cache::ExactSelectorMerkleLookupKeyV1;
use agent_semantic_content_identity::exact_selector_merkle::{
    ExactProjectionModeV1, blake3_content_digest_v1, canonical_content_digest,
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
    let canonical_item_selector = CanonicalItemSelector::new(
        CanonicalItemIdentity::new("rust", "function", "example"),
        selector,
    );
    let source = b"fn example() {}\n";
    let source_blob_digest = blake3_content_digest_v1(source);
    let parser_identity_digest = canonical_content_digest(b"parser", &[b"rs-harness"]);
    let query_pack_digest = canonical_content_digest(b"query-pack", &[b"rust"]);
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([(
        owner_path.to_string(),
        source_blob_digest.clone(),
    )])
    .expect("workspace Merkle tree");
    let packet = build_exact_selector_projection_packet_v1(
        agent_semantic_content_identity::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input {
            language_id: &agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketLanguageIdV1::from(
                "rust",
            ),
            provider_id: &agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketProviderIdV1::from(
                "rs-harness",
            ),
            canonical_item_selector,
            parser_identity_digest: &parser_identity_digest,
            query_pack_digest: &query_pack_digest,
            owner_path: &agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketOwnerPathV1::from(
                owner_path,
            ),
            structural_selector: &agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1::from(
                selector,
            ),
            projection_mode: ExactProjectionModeV1::Code,
            source_byte_start: 0,
            source_byte_end: source.len() as u64,
            source,
            normalized_parser_facts: br#"{"kind":"fn","name":"example"}"#,
            projection: source,
        },
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

#[test]
fn source_index_relation_publication_omits_unqueryable_owner_and_keeps_bound_relation() {
    use agent_semantic_content_identity::provider_projection_relation::{
        PROVIDER_RELATION_ITEM_ENDPOINT_KIND, PROVIDER_RELATION_OWNER_ENDPOINT_KIND,
        ProviderProjectedRelation, ProviderProjectedRelationEndpoint,
    };
    let path = "benches/query_search_microbench.rs";
    let source = b"fn bench() {}\n";
    let endpoint = |kind: &str, id: &str| ProviderProjectedRelationEndpoint {
        kind: kind.to_owned(),
        id: id.to_owned(),
    };
    let relation = || ProviderProjectedRelation {
        from: endpoint(
            PROVIDER_RELATION_OWNER_ENDPOINT_KIND,
            &format!("owner:{path}"),
        ),
        kind: "references".to_owned(),
        to: endpoint(PROVIDER_RELATION_ITEM_ENDPOINT_KIND, "selector:target"),
    };
    let request = |selectors| super::ClientDbSourceIndexImportRequest {
        source_blobs: Default::default(),
        generation_id: super::CacheGenerationId::from("relation-publication"),
        project_root: super::temp_root("relation-publication"),
        schema_id: super::SemanticSchemaId::from(super::CLIENT_DB_SOURCE_INDEX_SCHEMA_ID),
        schema_version: super::SemanticSchemaVersion::from(
            super::CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION,
        ),
        selector_source: super::ClientDbSourceIndexSource::from(
            super::CLIENT_DB_SOURCE_INDEX_PROVIDER_ID,
        ),
        file_hashes: vec![super::ClientCacheFileHash {
            path: path.to_owned(),
            sha256: "0".repeat(64),
            byte_len: source.len() as u64,
            mtime_ms: 0,
        }],
        files: vec![super::ClientDbSourceIndexImportFile {
            relative_path: path.to_owned(),
            language_id: super::LanguageId::from("rust"),
            provider_id: super::ProviderId::from("rs-harness"),
            text: String::from_utf8_lossy(source).into_owned(),
            selectors,
            relations: vec![relation()],
        }],
    };
    let omitted = crate::source_index_fixture::build_fixture_source_index_import(request(vec![]))
        .expect("owner materializes");
    assert_eq!(omitted.owners.len(), 1);
    assert!(
        omitted
            .source_blobs
            .get(&super::ClientDbSourceIndexPath::from(path))
            .is_some()
    );
    assert!(omitted.relations.is_empty());
    let selector = super::rust_selector_fixture(
        path,
        "rust://benches/query_search_microbench.rs#item/function/bench",
        "selector:target",
        source,
    );
    let retained =
        crate::source_index_fixture::build_fixture_source_index_import(request(vec![selector]))
            .expect("bound relation materializes");
    assert_eq!(retained.relations.len(), 1);
}
