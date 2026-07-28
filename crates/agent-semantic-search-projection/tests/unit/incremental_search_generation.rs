use serde_json::{Value, json};

use super::validate_incremental_search_generation_v1;

const QUERY_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OWNER_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WORKSPACE_DIGEST: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const GENERATION_DIGEST: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const INVENTORY_DIGEST: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

#[test]
fn complete_cached_query_is_semantically_valid_and_side_effect_free() {
    validate_incremental_search_generation_v1(&complete_cached_packet())
        .expect("complete cached packet");
}

#[test]
fn partial_processed_query_binds_continuation_and_owner_refresh() {
    validate_incremental_search_generation_v1(&partial_processed_packet())
        .expect("partial processed packet");
}

#[test]
fn semantic_validator_rejects_cross_field_drift() {
    for (reason, packet) in [
        (
            "incremental-budget-exceeded",
            mutate(complete_cached_packet(), |packet| {
                packet["completeness"]["dirtyOwnerCount"] = json!(1);
                packet["completeness"]["processedOwnerCount"] = json!(1);
                packet["completeness"]["incrementalBudget"] = json!(0);
                packet["queryOwnerResults"][0]["state"] = json!("processed");
            }),
        ),
        (
            "incremental-counter-exceeded",
            mutate(complete_cached_packet(), |packet| {
                packet["counters"]["sourceByteReads"] = json!(1);
            }),
        ),
        (
            "query-cache-key-drift",
            mutate(complete_cached_packet(), |packet| {
                packet["projections"][0]["queryDigest"] = json!("f".repeat(64));
            }),
        ),
        (
            "capture-item-span-drift",
            mutate(complete_cached_packet(), |packet| {
                packet["projections"][0]["sourceByteEnd"] = json!(40);
            }),
        ),
        (
            "invalid-complete-state",
            mutate(complete_cached_packet(), |packet| {
                packet["inventory"]["state"] = json!("known");
                packet["completeness"]["remainingCountKind"] = json!("known-lower-bound");
            }),
        ),
        (
            "continuation-generation-drift",
            mutate(partial_processed_packet(), |packet| {
                packet["continuation"]["generationRootDigest"] = json!("f".repeat(64));
            }),
        ),
        (
            "capture-count-drift",
            mutate(complete_cached_packet(), |packet| {
                packet["queryOwnerResults"][0]["captureCount"] = json!(2);
            }),
        ),
    ] {
        let error =
            validate_incremental_search_generation_v1(&packet).expect_err("reject semantic drift");
        assert_eq!(error.reason_kind(), reason, "packet={packet}");
    }
}

fn complete_cached_packet() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.incremental-search-generation",
        "schemaVersion": "1",
        "operation": "treesitter-query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerWorkspaceRoot": ".",
        "providerWorkspaceIdentityDigest": WORKSPACE_DIGEST,
        "generationBefore": generation(),
        "generationAfter": generation(),
        "query": {
            "kind": "treesitter",
            "queryDigest": QUERY_DIGEST,
            "captureNames": ["declaration.name"]
        },
        "inventory": {
            "state": "exact",
            "knownOwnerCount": 1,
            "inventoryDigest": INVENTORY_DIGEST
        },
        "completeness": {
            "state": "complete",
            "indexedOwnerCount": 1,
            "dirtyOwnerCount": 0,
            "processedOwnerCount": 0,
            "remainingOwnerCount": 0,
            "remainingCountKind": "exact",
            "incrementalBudget": 4
        },
        "ownerAcquisitions": [owner_acquisition("unchanged")],
        "queryOwnerResults": [query_owner_result("cached", 0)],
        "projections": [capture_projection()],
        "counters": counters(0, 0, 0, 0, 0),
        "continuation": null
    })
}

fn partial_processed_packet() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.incremental-search-generation",
        "schemaVersion": "1",
        "operation": "treesitter-query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerWorkspaceRoot": ".",
        "providerWorkspaceIdentityDigest": WORKSPACE_DIGEST,
        "generationBefore": null,
        "generationAfter": generation(),
        "query": {
            "kind": "treesitter",
            "queryDigest": QUERY_DIGEST,
            "captureNames": ["declaration.name"]
        },
        "inventory": {
            "state": "known",
            "knownOwnerCount": 2,
            "inventoryDigest": INVENTORY_DIGEST
        },
        "completeness": {
            "state": "partial",
            "indexedOwnerCount": 1,
            "dirtyOwnerCount": 2,
            "processedOwnerCount": 1,
            "remainingOwnerCount": 1,
            "remainingCountKind": "known-lower-bound",
            "incrementalBudget": 1
        },
        "ownerAcquisitions": [owner_acquisition("new")],
        "queryOwnerResults": [query_owner_result("processed", 1)],
        "projections": [capture_projection()],
        "counters": counters(1, 1, 1, 1, 1),
        "continuation": {
            "providerWorkspaceIdentityDigest": WORKSPACE_DIGEST,
            "queryDigest": QUERY_DIGEST,
            "generationRootDigest": GENERATION_DIGEST,
            "inventoryState": "known",
            "remainingCountKind": "known-lower-bound",
            "nextOwnerCursor": "src/next.rs"
        }
    })
}

fn generation() -> Value {
    json!({
        "rootDigest": GENERATION_DIGEST,
        "leafCount": 1,
        "ownerCount": 1
    })
}

fn owner_acquisition(decision: &str) -> Value {
    json!({
        "ownerPath": "src/lib.rs",
        "decision": decision,
        "fingerprint": {
            "fileIdentity": "unix:1:2",
            "sizeBytes": 100,
            "modifiedUnixNanos": 1,
            "changeTimeUnixNanos": 2,
            "contentDigest": OWNER_DIGEST
        }
    })
}

fn query_owner_result(state: &str, refresh_count: u64) -> Value {
    json!({
        "ownerPath": "src/lib.rs",
        "ownerContentDigest": OWNER_DIGEST,
        "queryDigest": QUERY_DIGEST,
        "state": state,
        "captureCount": 1,
        "completeOwnerRefreshCount": refresh_count
    })
}

fn capture_projection() -> Value {
    json!({
        "ownerPath": "src/lib.rs",
        "sourceContentDigest": OWNER_DIGEST,
        "queryDigest": QUERY_DIGEST,
        "structuralSelector": "rust://src/lib.rs#item/function/alpha",
        "signature": "pub fn alpha()",
        "itemKind": "function",
        "itemName": "alpha",
        "captureName": "declaration.name",
        "itemSourceByteStart": 0,
        "itemSourceByteEnd": 30,
        "sourceByteStart": 7,
        "sourceByteEnd": 12
    })
}

fn counters(
    source_reads: u64,
    provider_parses: u64,
    cache_writes: u64,
    refreshes: u64,
    owner_writes: u64,
) -> Value {
    json!({
        "metadataReads": 1,
        "sourceByteReads": source_reads,
        "sourceBytesRead": source_reads * 100,
        "providerParses": provider_parses,
        "queryCacheReads": 1,
        "queryCacheWrites": cache_writes,
        "completeOwnerRefreshes": refreshes,
        "ownerIndexWrites": owner_writes,
        "casWrites": 0,
        "merkleLeafWrites": owner_writes,
        "merklePathNodeWrites": owner_writes,
        "fullSourceWalks": 0,
        "fullCasMaterializations": 0,
        "fullMerkleRebuilds": 0,
        "unrelatedProviderCount": 0
    })
}

fn mutate(mut packet: Value, mutation: impl FnOnce(&mut Value)) -> Value {
    mutation(&mut packet);
    packet
}
