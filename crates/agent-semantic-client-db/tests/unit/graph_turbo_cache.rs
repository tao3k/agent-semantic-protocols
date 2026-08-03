use agent_semantic_client_db::graph_turbo_cache::{
    GraphTurboCacheEntry, GraphTurboCacheIdentity, GraphTurboCacheReadStatus,
    GraphTurboProposalStatus, TursoGraphTurboCache,
};
use serde_json::json;

fn identity(snapshot: char) -> GraphTurboCacheIdentity {
    GraphTurboCacheIdentity {
        snapshot_digest: snapshot.to_string().repeat(64),
        workspace_generation_root_digest: "generation:one".into(),
        profile: "owner-query".into(),
        algorithm: "typed-ppr-diverse".into(),
        seed_digest: "b".repeat(64),
        parameter_digest: "c".repeat(64),
    }
}

#[tokio::test]
async fn exact_identity_hits_and_stale_snapshot_misses() {
    let directory = tempfile::tempdir().expect("tempdir");
    let cache = TursoGraphTurboCache::open(&directory.path().join("cache.db"))
        .await
        .expect("open");
    let entry = GraphTurboCacheEntry {
        identity: identity('a'),
        proposal_status: GraphTurboProposalStatus::Candidate,
        result: json!({"frontier": []}),
    };
    cache.put(&entry).await.expect("put");
    let exact = cache.get(&entry.identity).await.expect("get");
    assert_eq!(exact.status, GraphTurboCacheReadStatus::Hit);
    assert_eq!(exact.entry, Some(entry));
    let stale = cache.get(&identity('d')).await.expect("stale");
    assert_eq!(stale.status, GraphTurboCacheReadStatus::Miss);
    assert_eq!(stale.entry, None);

    drop(cache);
    let reopened = TursoGraphTurboCache::open(&directory.path().join("cache.db"))
        .await
        .expect("reopen");
    let restored = reopened.get(&identity('a')).await.expect("restored");
    assert_eq!(restored.status, GraphTurboCacheReadStatus::Hit);
}

#[test]
fn malformed_identity_fails_closed() {
    let mut invalid = identity('a');
    invalid.snapshot_digest = "latest".into();
    assert!(invalid.validate().is_err());
}

#[test]
fn proposal_status_cannot_deserialize_evidence_authority() {
    assert!(serde_json::from_str::<GraphTurboProposalStatus>("\"proved\"").is_err());
    assert!(serde_json::from_str::<GraphTurboProposalStatus>("\"asserted\"").is_err());
}

#[test]
fn resident_receipt_is_candidate_only_at_the_turso_boundary() {
    let candidate = json!({
        "status": "rank-completed",
        "authority": "candidate",
        "result": {"frontier": []}
    });
    let entry = GraphTurboCacheEntry::from_resident_receipt(identity('a'), &candidate)
        .expect("candidate receipt");
    assert_eq!(entry.proposal_status, GraphTurboProposalStatus::Candidate);

    let forged = json!({
        "status": "rank-completed",
        "authority": "proved",
        "result": {"frontier": []}
    });
    assert!(GraphTurboCacheEntry::from_resident_receipt(identity('a'), &forged).is_err());
}
