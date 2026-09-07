// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::CanonicalItemSelector;
use agent_semantic_content_identity::ExactSelectorProjectionRecordV1;
use agent_semantic_content_identity::exact_selector_merkle::ExactProjectionModeV1;
use agent_semantic_content_identity::exact_selector_merkle::blake3_content_digest_v1;
use agent_semantic_content_identity::exact_selector_projection_packet::ExactSelectorProjectionPacketV1Input;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketExecutionCommandDigestV1;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketLanguageIdV1;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketOwnerPathV1;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketProviderIdV1;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketSemanticRegistryDigestV1;
use agent_semantic_content_identity::exact_selector_projection_packet::ProjectionPacketStructuralSelectorV1;
use agent_semantic_content_identity::exact_selector_projection_packet::build_exact_selector_projection_packet_v1;
use agent_semantic_content_identity::exact_selector_projection_packet::derive_parser_identity_digest_v1;
use agent_semantic_content_identity::exact_selector_projection_packet::derive_query_pack_identity_digest_v1;
use agent_semantic_content_identity::workspace_merkle_v1::WorkspacePathMerkleTreeV1;

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
    structural_selector: &str,
    symbol: &str,
) -> agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
    let payload = serde_json::json!({
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
    });
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"asp.projection-evidence-context.v1\0");
    for component in [
        "rust",
        "asp-rust",
        &"0".repeat(64),
        &"1".repeat(64),
        &"2".repeat(64),
    ] {
        hasher.update(&(component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    let evidence_context_ref = format!("blake3-256:{}", hasher.finalize().to_hex());
    let envelope = agent_semantic_content_identity::semantic_projection::SemanticProjection::new(
        agent_semantic_content_identity::semantic_projection::SemanticProjectionInput {
            projection_kind: "callable-skeleton".into(),
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
            root_selector: structural_selector.into(),
            evidence_context_ref: evidence_context_ref.clone().into(),
            payload_schema_id: "agent.semantic-protocols.callable-skeleton".into(),
            payload,
        },
    )
    .expect("build semantic projection fixture");
    let mut envelope = serde_json::to_value(envelope).expect("encode semantic projection value");
    let typed_payload: agent_semantic_content_identity::callable_skeleton_projection::CallableSkeletonPayload =
        serde_json::from_value(envelope["payload"].clone()).expect("decode typed fixture payload");
    let typed_bytes = serde_json::to_vec(&typed_payload).expect("encode typed fixture payload");
    envelope["payloadDigest"] =
        format!("blake3-256:{}", blake3::hash(&typed_bytes).to_hex()).into();
    let bytes = serde_json::to_vec(&envelope).expect("encode callable skeleton fixture");
    agent_semantic_client_db::runtime_server_workspace::WorkspaceDerivedProjectionSnapshot {
        projection_kind: agent_semantic_client_db::runtime_server_workspace::ExactProjectionKind::CallableSkeleton,
        bytes,
        evidence_context: Some(
            agent_semantic_content_identity::projection_evidence_context::ProjectionEvidenceContext {
                schema_id: "agent.semantic-protocols.projection-evidence-context".to_owned(),
                schema_version: "1".to_owned(),
                evidence_context_ref: evidence_context_ref.into(),
                language_id: "rust".into(),
                provider_id: "asp-rust".into(),
                generation_identity_digest: "0".repeat(64),
                parser_identity_digest: "1".repeat(64),
                query_pack_digest: "2".repeat(64),
            },
        ),
    }
}
