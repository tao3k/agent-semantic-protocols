use super::{
    PROJECTION_BATCH_RESPONSE_SCHEMA_ID, PROJECTION_BATCH_TRANSPORT, ProjectionBatchHeader,
    ProviderProjectedItem, ProviderProjectedItemIdentity, ProviderProjectedOwner,
    ProviderProjectionBatchRequest, ProviderProjectionBatchResponse, ProviderProjectionOwner,
    validate_response,
};

fn request() -> ProviderProjectionBatchRequest {
    ProviderProjectionBatchRequest {
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        workspace_identity: "workspace-projection-fixture".to_string(),
        generation_root_digest: "generation-a".to_string(),
        base_generation_root_digest: Some("generation-base".to_string()),
        owners: vec![ProviderProjectionOwner {
            owner_path: "src/lib.rs".to_string(),
            source_leaf_digest: "leaf-a".to_string(),
            source_bytes: b"pub fn exact() {}\n".to_vec(),
        }],
    }
}

#[test]
fn framed_request_carries_exact_owner_bytes() {
    let request = request();
    let frame = request.encode().expect("encode request");
    let header_length = u32::from_be_bytes(frame[..4].try_into().expect("header length"));
    let header_end = 4 + header_length as usize;
    let header: ProjectionBatchHeader =
        serde_json::from_slice(&frame[4..header_end]).expect("decode header");
    assert_eq!(header.transport, PROJECTION_BATCH_TRANSPORT);
    assert_eq!(header.workspace_identity, request.workspace_identity);
    assert_eq!(
        header.owners[0].byte_length,
        request.owners[0].source_bytes.len()
    );
    assert_eq!(&frame[header_end..], request.owners[0].source_bytes);
}

#[test]
fn response_validation_rejects_generation_or_owner_drift() {
    let request = request();
    let mut response = ProviderProjectionBatchResponse {
        schema_id: PROJECTION_BATCH_RESPONSE_SCHEMA_ID.to_string(),
        schema_version: "1".to_string(),
        language_id: "rust".to_string(),
        provider_id: "rs-harness".to_string(),
        generation_root_digest: "generation-a".to_string(),
        owners: vec![ProviderProjectedOwner {
            owner_path: "src/lib.rs".to_string(),
            source_leaf_digest: "leaf-a".to_string(),
            items: vec![ProviderProjectedItem {
                item_id: "item:function:exact".to_string(),
                owner_id: "owner:src/lib.rs".to_string(),
                kind: "function".to_string(),
                name: "exact".to_string(),
                selector: "rust://src/lib.rs#item/function/exact".to_string(),
                source_byte_start: 0,
                source_byte_end: request.owners[0].source_bytes.len(),
                identity: ProviderProjectedItemIdentity {
                    schema_id: "asp.canonical-language-item-identity.v1".to_string(),
                    schema_version: "1".to_string(),
                    language_id: "rust".to_string(),
                    kind: "function".to_string(),
                    symbol: "exact".to_string(),
                    scopes: Vec::new(),
                },
            }],
            relations: Vec::new(),
        }],
    };
    validate_response(&request, &response).expect("matching response");
    response.owners[0].source_leaf_digest = "leaf-drift".to_string();
    assert!(validate_response(&request, &response).is_err());
}
