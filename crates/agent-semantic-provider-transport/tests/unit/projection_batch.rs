use super::{
    MAX_PROVIDER_PROJECTION_BATCH_OWNERS, MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES,
    MAX_PROVIDER_PROJECTION_SINGLE_OWNER_SOURCE_BYTES, PROJECTION_BATCH_RESPONSE_SCHEMA_ID,
    ProviderProjectedItem, ProviderProjectedItemIdentity, ProviderProjectedOwner,
    ProviderProjectionBatchRequest, ProviderProjectionBatchResponse, ProviderProjectionOwner,
    provider_projection_batch_ranges, provider_projection_batch_ranges_with_auxiliary_bytes,
    validate_response,
};

fn request() -> ProviderProjectionBatchRequest {
    ProviderProjectionBatchRequest {
        language_id: "rust".to_string(),
        provider_id: "asp-rust".to_string(),
        workspace_identity: "workspace-projection-fixture".to_string(),
        generation_root_digest: "generation-a".to_string(),
        parser_identity_digest: "parser-a".to_string(),
        query_pack_digest: "query-pack-a".to_string(),
        base_generation_root_digest: Some("generation-base".to_string()),
        owners: vec![ProviderProjectionOwner {
            owner_path: "src/lib.rs".to_string(),
            source_leaf_digest: "leaf-a".to_string(),
            source_bytes: b"pub fn exact() {}\n".to_vec(),
        }],
        auxiliary_owners: Vec::new(),
    }
}

#[test]
fn structured_request_carries_exact_owner_source() {
    let request = request();
    let payload: serde_json::Value =
        serde_json::from_slice(&request.encode().expect("encode request"))
            .expect("structured request");
    assert_eq!(payload["schemaVersion"], "1");
    assert_eq!(payload["workspaceIdentity"], request.workspace_identity);
    assert_eq!(payload["owners"][0]["sourceEncoding"], "utf8");
    assert_eq!(payload["owners"][0]["sourceText"], "pub fn exact() {}\n");
    assert!(payload["owners"][0].get("sourceBytesBase64").is_none());
    assert!(payload.get("auxiliaryOwners").is_none());
    assert!(payload.get("transport").is_none());
}

#[test]
fn structured_request_carries_auxiliary_context_without_response_coverage() {
    let mut request = request();
    request.auxiliary_owners.push(ProviderProjectionOwner {
        owner_path: "Cargo.toml".to_string(),
        source_leaf_digest: "config-a".to_string(),
        source_bytes: b"[package]\nname = \"fixture\"\n".to_vec(),
    });
    let payload: serde_json::Value = serde_json::from_slice(
        &request
            .encode()
            .expect("encode request with auxiliary owner"),
    )
    .expect("structured request");
    assert_eq!(payload["auxiliaryOwners"][0]["ownerPath"], "Cargo.toml");

    request.auxiliary_owners[0].owner_path = "src/lib.rs".to_string();
    assert!(request.encode().is_err());
}

#[test]
fn structured_request_preserves_non_utf8_owner_bytes() {
    let mut request = request();
    request.owners[0].source_bytes = vec![b'(', b'"', 0xed, 0xa0, 0x80, b'"', b')'];

    let payload: serde_json::Value =
        serde_json::from_slice(&request.encode().expect("encode non-UTF-8 request"))
            .expect("structured request");
    assert_eq!(payload["owners"][0]["sourceEncoding"], "base64");
    assert_eq!(payload["owners"][0]["sourceBytesBase64"], "KCLtoIAiKQ==");
    assert!(payload["owners"][0].get("sourceText").is_none());
}

#[test]
fn response_validation_rejects_generation_or_owner_drift() {
    let request = request();
    let mut response = ProviderProjectionBatchResponse {
        schema_id: PROJECTION_BATCH_RESPONSE_SCHEMA_ID.to_string(),
        schema_version: "1".to_string(),
        language_id: "rust".to_string(),
        provider_id: "asp-rust".to_string(),
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
                    schema_id: "agent.semantic-protocols.canonical-language-item-identity"
                        .to_string(),
                    schema_version: "1".to_string(),
                    language_id: "rust".to_string(),
                    kind: "function".to_string(),
                    symbol: "exact".to_string(),
                    scopes: Vec::new(),
                },
                projections: Vec::new(),
            }],
            relations: Vec::new(),
        }],
    };
    validate_response(&request, &response).expect("matching response");

    let mut delimiter_request = request.clone();
    delimiter_request.owners[0].owner_path = "src/hash#.rs".to_string();
    let mut delimiter_response = response.clone();
    delimiter_response.owners[0].owner_path = "src/hash#.rs".to_string();
    delimiter_response.owners[0].items[0].owner_id = "owner:src/hash#.rs".to_string();
    delimiter_response.owners[0].items[0].selector =
        "rust://src/hash%23.rs#item/function/exact".to_string();
    validate_response(&delimiter_request, &delimiter_response)
        .expect("canonical encoded owner path must match decoded response owner");
    delimiter_response.owners[0].items[0].selector =
        "rust://src/hash#.rs#item/function/exact".to_string();
    assert!(validate_response(&delimiter_request, &delimiter_response).is_err());

    response.owners[0].source_leaf_digest = "leaf-drift".to_string();
    assert!(validate_response(&request, &response).is_err());
}

#[test]
fn workspace_pressure_is_split_into_bounded_wire_frames() {
    let owner_sizes = vec![64 * 1024; 294];
    let ranges = provider_projection_batch_ranges(&owner_sizes);

    assert_eq!(ranges.len(), 49);
    assert_eq!(ranges.first().expect("first frame"), &(0..6));
    assert_eq!(ranges.last().expect("last frame"), &(288..294));
    for range in ranges {
        assert!(range.len() <= MAX_PROVIDER_PROJECTION_BATCH_OWNERS);
        assert!(
            owner_sizes[range].iter().copied().sum::<usize>()
                <= MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES
        );
    }
}

#[test]
fn source_byte_pressure_splits_before_owner_count_limit() {
    let owner_sizes = vec![128 * 1024; 12];
    let ranges = provider_projection_batch_ranges(&owner_sizes);

    assert_eq!(ranges, vec![0..3, 3..6, 6..9, 9..12]);
}

#[test]
fn auxiliary_context_bytes_are_reserved_in_every_frame() {
    let owner_sizes = vec![128 * 1024; 4];
    let ranges = provider_projection_batch_ranges_with_auxiliary_bytes(&owner_sizes, 128 * 1024);

    assert_eq!(ranges, vec![0..2, 2..4]);
}

#[test]
fn owner_larger_than_the_source_budget_is_isolated_for_wire_validation() {
    let owner_sizes = vec![MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES + 1, 1];
    let ranges = provider_projection_batch_ranges(&owner_sizes);

    assert_eq!(ranges, vec![0..1, 1..2]);
}

#[test]
fn indivisible_large_owner_is_admitted_only_as_a_single_owner_frame() {
    let mut request = request();
    request.owners[0].source_bytes = vec![b'x'; MAX_PROVIDER_PROJECTION_BATCH_SOURCE_BYTES + 1];
    request.encode().expect("isolated large owner frame");

    request.owners.push(ProviderProjectionOwner {
        owner_path: "src/second.rs".to_string(),
        source_leaf_digest: "leaf-second".to_string(),
        source_bytes: vec![b'y'],
    });
    assert!(request.encode().is_err());
}

#[test]
fn indivisible_owner_still_has_a_transport_safe_upper_bound() {
    let mut request = request();
    request.owners[0].source_bytes =
        vec![b'x'; MAX_PROVIDER_PROJECTION_SINGLE_OWNER_SOURCE_BYTES + 1];

    assert!(request.encode().is_err());
}
