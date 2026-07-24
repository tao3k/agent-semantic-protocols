use crate::{
    DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION, DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID,
    DerivedArtifactAuthorityState, DerivedSourceArtifactEvidence, DerivedSourceArtifactKind,
    SourceSnapshotEvidence, SourceSnapshotKind,
};

fn source_snapshot() -> SourceSnapshotEvidence {
    SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        1,
        "b".repeat(64),
    )
}

#[test]
fn current_evidence_resolves_the_snapshot_derived_digest() {
    let digest = "c".repeat(64);
    let snapshot = source_snapshot();
    let evidence = DerivedSourceArtifactEvidence::current(
        DerivedSourceArtifactKind::EvidenceGraph,
        &digest,
        snapshot.clone(),
    );

    assert_eq!(
        evidence.schema_id,
        DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID
    );
    assert_eq!(
        evidence.cache_disposition,
        DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION
    );
    assert_eq!(
        evidence.authority_state,
        DerivedArtifactAuthorityState::Current
    );
    assert_eq!(evidence.expected_artifact_digest, digest);
    assert_eq!(
        evidence.resolved_artifact_digest.as_deref(),
        Some(digest.as_str())
    );
    assert_eq!(evidence.source_snapshot, snapshot);
}

#[test]
fn parse_artifact_v1_key_invalidates_when_provider_digest_changes() {
    let initial_snapshot = source_snapshot();
    let mut changed_provider_snapshot = initial_snapshot.clone();
    changed_provider_snapshot.provider_digest = "c".repeat(64);

    let initial_key = crate::hash_derived_artifact_key(crate::DerivedArtifactKeyInput {
        artifact_kind: "rust-parse-artifact",
        schema_id: "asp.rust.parse-artifact.v1",
        snapshot_root: &initial_snapshot.root_digest,
        provider_digest: &initial_snapshot.provider_digest,
        parameters: &[],
    });
    let changed_provider_key = crate::hash_derived_artifact_key(crate::DerivedArtifactKeyInput {
        artifact_kind: "rust-parse-artifact",
        schema_id: "asp.rust.parse-artifact.v1",
        snapshot_root: &changed_provider_snapshot.root_digest,
        provider_digest: &changed_provider_snapshot.provider_digest,
        parameters: &[],
    });

    assert_eq!(
        initial_snapshot.root_digest, changed_provider_snapshot.root_digest,
        "this regression must isolate provider identity from source content"
    );
    assert_ne!(initial_key.value, changed_provider_key.value);
}

#[test]
fn missing_and_stale_evidence_never_resolve_a_cached_digest() {
    let digest = "d".repeat(64);
    for evidence in [
        DerivedSourceArtifactEvidence::missing(
            DerivedSourceArtifactKind::SourceIndex,
            &digest,
            source_snapshot(),
        ),
        DerivedSourceArtifactEvidence::stale(
            DerivedSourceArtifactKind::CompactGraph,
            &digest,
            source_snapshot(),
        ),
    ] {
        assert_eq!(evidence.expected_artifact_digest, digest);
        assert!(evidence.resolved_artifact_digest.is_none());
    }
}

#[test]
fn unresolved_evidence_omits_resolved_artifact_digest_from_json() {
    let evidence = DerivedSourceArtifactEvidence::missing(
        DerivedSourceArtifactKind::ParserArtifact,
        "e".repeat(64),
        source_snapshot(),
    );
    let json = serde_json::to_value(evidence).expect("evidence should serialize");

    assert_eq!(json["schemaId"], DERIVED_SOURCE_ARTIFACT_EVIDENCE_SCHEMA_ID);
    assert_eq!(json["artifactKind"], "parser-artifact");
    assert_eq!(json["authorityState"], "missing");
    assert_eq!(
        json["cacheDisposition"],
        DERIVED_SOURCE_ARTIFACT_CACHE_DISPOSITION
    );
    assert!(json.get("resolvedArtifactDigest").is_none());
}
