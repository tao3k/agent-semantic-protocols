use agent_semantic_content_identity::{
    CanonicalItemSelector, ExactSelectorProjectionRecordV1,
    exact_selector_merkle::{ExactProjectionModeV1, blake3_content_digest_v1},
    exact_selector_projection_packet::{
        ExactSelectorProjectionPacketV1Input, ProjectionPacketExecutionCommandDigestV1,
        ProjectionPacketLanguageIdV1, ProjectionPacketOwnerPathV1, ProjectionPacketProviderIdV1,
        ProjectionPacketSemanticRegistryDigestV1, ProjectionPacketStructuralSelectorV1,
        build_exact_selector_projection_packet_v1, derive_parser_identity_digest_v1,
        derive_query_pack_identity_digest_v1,
    },
    workspace_merkle_v1::WorkspacePathMerkleTreeV1,
};

pub(crate) struct ProjectionFixtureInput<'a> {
    pub language_id: &'a str,
    pub provider_id: &'a str,
    pub owner_path: &'a str,
    pub structural_selector: &'a str,
    pub item_kind: &'a str,
    pub item_name: &'a str,
    pub source: &'a [u8],
    pub source_byte_start: u64,
    pub source_byte_end: u64,
}

pub(crate) fn projection_record(
    input: ProjectionFixtureInput<'_>,
) -> ExactSelectorProjectionRecordV1 {
    let source_start =
        usize::try_from(input.source_byte_start).expect("fixture source start fits usize");
    let source_end = usize::try_from(input.source_byte_end).expect("fixture source end fits usize");
    let projection = input
        .source
        .get(source_start..source_end)
        .expect("fixture byte range belongs to source");
    let language_id = ProjectionPacketLanguageIdV1::from(input.language_id);
    let provider_id = ProjectionPacketProviderIdV1::from(input.provider_id);
    let owner_path = ProjectionPacketOwnerPathV1::from(input.owner_path);
    let structural_selector = ProjectionPacketStructuralSelectorV1::from(input.structural_selector);
    let parser_identity_digest = derive_parser_identity_digest_v1(
        &provider_id,
        &ProjectionPacketExecutionCommandDigestV1::from("fixture-execution-command"),
        &ProjectionPacketSemanticRegistryDigestV1::from("fixture-semantic-registry"),
    );
    let query_pack_digest = derive_query_pack_identity_digest_v1(input.language_id.as_bytes());
    let canonical_item_selector = CanonicalItemSelector::parse(input.structural_selector)
        .expect("fixture canonical item selector");
    let tree = WorkspacePathMerkleTreeV1::from_file_digests([(
        input.owner_path.to_owned(),
        blake3_content_digest_v1(input.source),
    )])
    .expect("fixture workspace Merkle tree");
    build_exact_selector_projection_packet_v1(ExactSelectorProjectionPacketV1Input {
        source_byte_start: input.source_byte_start,
        source_byte_end: input.source_byte_end,
        language_id: &language_id,
        provider_id: &provider_id,
        canonical_item_selector,
        parser_identity_digest: &parser_identity_digest,
        query_pack_digest: &query_pack_digest,
        owner_path: &owner_path,
        structural_selector: &structural_selector,
        projection_mode: ExactProjectionModeV1::Code,
        source: input.source,
        normalized_parser_facts: format!(
            "language={};kind={};name={};selector={}",
            input.language_id, input.item_kind, input.item_name, input.structural_selector
        )
        .as_bytes(),
        projection,
    })
    .enrich_projection_record(&tree)
    .expect("fixture exact-selector projection record")
}

pub(crate) fn source_blobs_fixture<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> agent_semantic_client_db::ClientDbSourceIndexSourceBlobs {
    agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        sources.into_iter().map(|(owner_path, bytes)| {
            (
                agent_semantic_client_db::ClientDbSourceIndexPath::new(owner_path),
                bytes.to_vec(),
            )
        }),
    )
}

pub(crate) fn callable_skeleton_projection_fixture(
    owner_path: &str,
    structural_selector: &str,
    symbol: &str,
) -> agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
    let digest = "0".repeat(64);
    let root_selector = serde_json::json!({
        "schemaId": "asp.exact-structural-selector.v1",
        "schemaVersion": "1",
        "languageId": "rust",
        "ownerPath": owner_path,
        "selector": structural_selector,
        "generationIdentityDigest": digest,
        "parserIdentityDigest": "1".repeat(64),
        "queryPackDigest": "2".repeat(64),
        "rootItemSelector": {
            "schemaId": "asp.canonical-item-selector.v1",
            "schemaVersion": "1",
            "languageId": "rust",
            "kind": "function",
            "symbol": symbol,
            "scopes": [],
            "structuralSelector": structural_selector,
        },
        "segments": [],
    });
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schemaId": "agent.semantic-protocols.callable-skeleton-projection",
        "schemaVersion": "1",
        "projectionKind": "callable-skeleton",
        "languageId": "rust",
        "providerId": "rs-harness",
        "rootSelector": root_selector,
        "rootNodeId": "callable:root",
        "callable": {
            "kind": "function",
            "displayName": symbol,
            "signature": symbol,
        },
        "nodes": [{
            "nodeId": "callable:root",
            "kind": "callable",
            "label": symbol,
            "order": 0,
            "queryable": false,
        }],
        "relations": [],
        "cost": {
            "sourceBytes": 0,
            "projectedBytes": 0,
            "omittedBytes": 0,
        },
    }))
    .expect("encode callable skeleton fixture");
    agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
        projection_kind: "callable-skeleton".to_owned(),
        bytes,
    }
}
