use agent_semantic_artifacts as identity;

use identity::{
    ARTIFACT_IDENTITY_SCHEMA_ID, ARTIFACT_IDENTITY_SCHEMA_VERSION, ArtifactChildRef,
    ArtifactGeneration, ArtifactHash, ArtifactIdentityDocument, ArtifactJson, ArtifactKind,
    ArtifactLeafInput, ArtifactNodeInput, ArtifactRepoId, ArtifactRootInput, ArtifactRootRef,
    ArtifactScopeId, ArtifactWorkspaceId, EDGE_DOMAIN_V1, HASH_ALGORITHM_BLAKE3, hash_leaf,
    hash_node, hash_normalized_json, hash_root,
};
use serde_json::json;
use std::time::Duration;
use std::time::Instant;

#[test]
fn normalized_json_hash_ignores_object_key_order() {
    let left = json!({"schemaVersion": "1", "schemaId": "x", "items": [2, 1]});
    let right = json!({"items": [2, 1], "schemaId": "x", "schemaVersion": "1"});

    assert_eq!(
        hash_normalized_json(
            &ArtifactJson::from_serializable(&left).expect("left JSON must serialize"),
        ),
        hash_normalized_json(
            &ArtifactJson::from_serializable(&right).expect("right JSON must serialize"),
        )
    );
}

#[test]
fn node_hash_ignores_child_insertion_order() {
    let a = ArtifactHash::blake3(b"a");
    let b = ArtifactHash::blake3(b"b");
    let left = ArtifactNodeInput {
        kind: ArtifactKind::new("compactGraph"),
        schema_id: "semantic-graph-turbo-artifact-events".to_string(),
        schema_version: "1".to_string(),
        producer_hash: None,
        payload_hash: Some(a.clone()),
        metadata_hash: None,
        children: vec![
            ArtifactChildRef::new("source", "b", b.clone(), 1),
            ArtifactChildRef::new("source", "a", a.clone(), 0),
        ],
    };
    let right = ArtifactNodeInput {
        children: vec![
            ArtifactChildRef::new("source", "a", a, 0),
            ArtifactChildRef::new("source", "b", b, 1),
        ],
        ..left.clone()
    };

    assert_eq!(hash_node(&left), hash_node(&right));
}

#[test]
fn root_hash_is_workspace_scoped() {
    let node_hash = ArtifactHash::blake3(b"node");
    let first = ArtifactRootInput {
        repo_id: ArtifactRepoId::new("repo"),
        workspace_id: ArtifactWorkspaceId::new("workspace-a"),
        scope_id: ArtifactScopeId::new("default"),
        generation: ArtifactGeneration::new("g1"),
        root_kind: ArtifactKind::new("sourceSnapshot"),
        node_hash: node_hash.clone(),
    };
    let second = ArtifactRootInput {
        workspace_id: ArtifactWorkspaceId::new("workspace-b"),
        ..first.clone()
    };

    assert_ne!(hash_root(&first), hash_root(&second));
}

#[test]
fn root_hash_is_deterministic_for_same_inputs() {
    let input = ArtifactRootInput {
        repo_id: ArtifactRepoId::new("repo"),
        workspace_id: ArtifactWorkspaceId::new("workspace-a"),
        scope_id: ArtifactScopeId::new("default"),
        generation: ArtifactGeneration::new("g1"),
        root_kind: ArtifactKind::new("sourceSnapshot"),
        node_hash: ArtifactHash::blake3(b"node"),
    };

    assert_eq!(hash_root(&input), hash_root(&input));
}

#[test]
fn provider_manifest_drift_changes_node_and_root_hashes() {
    let base = ArtifactNodeInput {
        kind: ArtifactKind::new("providerOutput"),
        schema_id: "semantic-provider-output".to_string(),
        schema_version: "1".to_string(),
        producer_hash: Some(ArtifactHash::blake3(b"rust-harness-manifest-a")),
        payload_hash: Some(ArtifactHash::blake3(b"owner-items-payload")),
        metadata_hash: Some(ArtifactHash::blake3(b"provider-metadata")),
        children: Vec::new(),
    };
    let changed_provider = ArtifactNodeInput {
        producer_hash: Some(ArtifactHash::blake3(b"rust-harness-manifest-b")),
        ..base.clone()
    };
    let base_node_hash = hash_node(&base);
    let changed_node_hash = hash_node(&changed_provider);

    assert_ne!(base_node_hash, changed_node_hash);

    let base_root = ArtifactRootInput {
        repo_id: ArtifactRepoId::new("repo"),
        workspace_id: ArtifactWorkspaceId::new("workspace-a"),
        scope_id: ArtifactScopeId::new("default"),
        generation: ArtifactGeneration::new("g1"),
        root_kind: ArtifactKind::new("providerOutput"),
        node_hash: base_node_hash,
    };
    let changed_root = ArtifactRootInput {
        node_hash: changed_node_hash,
        ..base_root.clone()
    };

    assert_ne!(hash_root(&base_root), hash_root(&changed_root));
}

#[test]
fn dynamic_overlay_generation_is_session_scoped() {
    let node_hash = ArtifactHash::blake3(b"dirty-overlay-node");
    let session_a = ArtifactRootInput {
        repo_id: ArtifactRepoId::new("repo"),
        workspace_id: ArtifactWorkspaceId::new("workspace-a"),
        scope_id: ArtifactScopeId::new("session-a"),
        generation: ArtifactGeneration::new("overlay-1"),
        root_kind: ArtifactKind::new("dynamicOverlay"),
        node_hash: node_hash.clone(),
    };
    let session_b = ArtifactRootInput {
        scope_id: ArtifactScopeId::new("session-b"),
        ..session_a.clone()
    };
    let next_generation = ArtifactRootInput {
        generation: ArtifactGeneration::new("overlay-2"),
        ..session_a.clone()
    };

    assert_ne!(hash_root(&session_a), hash_root(&session_b));
    assert_ne!(hash_root(&session_a), hash_root(&next_generation));
}

#[test]
fn leaf_domain_separates_codec_and_media_type() {
    let json_leaf = hash_leaf(ArtifactLeafInput {
        codec: "json",
        media_type: "application/json",
        payload: br#"{"a":1}"#,
    });
    let text_leaf = hash_leaf(ArtifactLeafInput {
        codec: "text",
        media_type: "text/plain",
        payload: br#"{"a":1}"#,
    });

    assert_ne!(json_leaf, text_leaf);
}

#[test]
fn large_payload_leaf_hash_stays_subsecond() {
    let payload = vec![b'x'; 1024 * 1024];
    let started = Instant::now();
    let hash = hash_leaf(ArtifactLeafInput {
        codec: "bytes",
        media_type: "application/octet-stream",
        payload: &payload,
    });
    let elapsed = started.elapsed();

    assert_eq!(hash.algorithm, "blake3");
    assert_eq!(hash.value.len(), 64);
    assert!(
        elapsed < Duration::from_secs(1),
        "1MiB artifact hashing should stay below 1s, elapsed={elapsed:?}"
    );
}

#[test]
fn identity_document_serializes_schema_compatible_root_refs() {
    let node_hash = ArtifactHash::blake3(b"node");
    let root = ArtifactRootRef::from_input(
        ArtifactRootInput {
            repo_id: ArtifactRepoId::new("repo_123"),
            workspace_id: ArtifactWorkspaceId::new("workspace_456"),
            scope_id: ArtifactScopeId::new("default"),
            generation: ArtifactGeneration::new("g1"),
            root_kind: ArtifactKind::new("searchReceipt"),
            node_hash,
        },
        Some(ArtifactHash::blake3(b"producer")),
        Some(ArtifactHash::blake3(b"schema")),
        Some(ArtifactHash::blake3(b"content")),
    );
    let document = ArtifactIdentityDocument::new(vec![root]);
    let json = serde_json::to_value(document).expect("serialize artifact identity document");

    assert_eq!(json["schemaId"], ARTIFACT_IDENTITY_SCHEMA_ID);
    assert_eq!(json["schemaVersion"], ARTIFACT_IDENTITY_SCHEMA_VERSION);
    assert_eq!(json["hashAlgorithm"], HASH_ALGORITHM_BLAKE3);
    assert_eq!(EDGE_DOMAIN_V1, "asp.edge.v1");
    assert_eq!(json["roots"][0]["repoId"], "repo_123");
    assert_eq!(json["roots"][0]["workspaceId"], "workspace_456");
    assert_eq!(json["roots"][0]["scopeId"], "default");
    assert_eq!(json["roots"][0]["rootKind"], "searchReceipt");
    assert_eq!(
        json["roots"][0]["rootHash"]["algorithm"],
        HASH_ALGORITHM_BLAKE3
    );
    assert_eq!(
        json["roots"][0]["rootHash"]["value"]
            .as_str()
            .expect("root hash string")
            .len(),
        64
    );
}
