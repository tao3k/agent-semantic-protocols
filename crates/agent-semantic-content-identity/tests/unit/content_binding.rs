use super::ActivationObservation;
use super::AuthorityStamp;
use super::ContentBindingError;
use super::ContentIdentity;
use super::ContentPublicationCommit;
use super::DIGEST_PREFIX;
use super::RuntimeArtifactReference;
use super::SourceGenerationReference;
use super::WorkspaceSnapshotReference;

fn identity(seed: char) -> ContentIdentity {
    let digest = format!("{DIGEST_PREFIX}{}", seed.to_string().repeat(64));
    ContentIdentity {
        runtime_artifact_digest: digest.clone(),
        workspace_snapshot_digest: digest.clone(),
        source_generation_digest: digest.clone(),
        source_index_digest: digest.clone(),
        schema_digest: digest.clone(),
        provider_catalog_digest: digest,
    }
}

fn authority(identity: &ContentIdentity) -> AuthorityStamp {
    AuthorityStamp {
        key_id: "test-authority".to_owned(),
        canonical_digest: identity.digest(),
        signature: "test-signature".to_owned(),
    }
}

#[test]
fn one_identity_has_one_deterministic_commit_digest() {
    let left_identity = identity('a');
    let right_identity = identity('a');
    let left =
        ContentPublicationCommit::linearize(left_identity.clone(), authority(&left_identity))
            .unwrap();
    let right =
        ContentPublicationCommit::linearize(right_identity.clone(), authority(&right_identity))
            .unwrap();
    assert_eq!(left, right);
    assert!(left.validate().is_ok());
}

#[test]
fn same_numeric_label_different_content_is_rejected() {
    let content = identity('a');
    let commit = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    let observation = ActivationObservation {
        publication_nonce: "publication-observation".to_owned(),
        commit_digest: commit.commit_digest.clone(),
    };
    let mut different = identity('b');
    different.schema_digest = identity('c').schema_digest;
    assert!(observation.matches_commit(&commit));
    assert_eq!(
        commit.admit_exact(&different),
        Err(ContentBindingError::ContentMismatch)
    );
    assert!(!observation.is_product_authority());
}

#[test]
fn corrupted_commit_and_non_durable_commit_fail_closed() {
    let identity = identity('d');
    let authority = authority(&identity);
    let mut commit = ContentPublicationCommit::linearize(identity, authority).unwrap();
    commit.commit_digest = format!("{DIGEST_PREFIX}{}", "e".repeat(64));
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::CommitDigestMismatch)
    );
    commit.commit_digest = commit.identity.digest();
    commit.durable = false;
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::NonDurableCommit)
    );
}

#[test]
fn invalid_transaction_fence_fails_closed() {
    let content = identity('e');
    let mut commit =
        ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    commit.expected_digest = Some(format!("{DIGEST_PREFIX}{}", "f".repeat(64)));
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::InvalidCommitFence)
    );
}

#[test]
fn rollback_requires_explicit_authority() {
    let content = identity('f');
    let commit = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    assert_eq!(
        commit.rollback_without_authority(),
        Err(ContentBindingError::RollbackRequiresAuthority)
    );
}

#[test]
fn references_must_share_provider_contract_content() {
    let digest = |seed: char| format!("{DIGEST_PREFIX}{}", seed.to_string().repeat(64));
    let artifact = RuntimeArtifactReference {
        schema_digest: digest('a'),
        artifact_digest: digest('b'),
        provider_contract_digest: digest('c'),
        signer_key_id: "schema-manager".to_owned(),
        signature: "signed".to_owned(),
    };
    let workspace = WorkspaceSnapshotReference {
        workspace_id: "workspace-test".to_owned(),
        workspace_catalog_digest: digest('d'),
        snapshot_digest: digest('e'),
        source_root_digest: digest('f'),
    };
    let source = SourceGenerationReference {
        source_generation_id: "source-test".to_owned(),
        source_snapshot_digest: digest('e'),
        source_index_digest: digest('a'),
        provider_id: "asp-rust".to_owned(),
        provider_contract_digest: digest('0'),
    };
    assert_eq!(
        ContentIdentity::from_references(&artifact, &workspace, &source, digest('b')),
        Err(ContentBindingError::ContentMismatch)
    );
}

#[test]
fn workspace_and_source_snapshot_must_be_the_same_content() {
    let digest = |seed: char| format!("{DIGEST_PREFIX}{}", seed.to_string().repeat(64));
    let artifact = RuntimeArtifactReference {
        schema_digest: digest('a'),
        artifact_digest: digest('b'),
        provider_contract_digest: digest('c'),
        signer_key_id: "schema-manager".to_owned(),
        signature: "signed".to_owned(),
    };
    let workspace = WorkspaceSnapshotReference {
        workspace_id: "workspace-test".to_owned(),
        workspace_catalog_digest: digest('d'),
        snapshot_digest: digest('e'),
        source_root_digest: digest('f'),
    };
    let source = SourceGenerationReference {
        source_generation_id: "source-test".to_owned(),
        source_snapshot_digest: digest('0'),
        source_index_digest: digest('a'),
        provider_id: "asp-rust".to_owned(),
        provider_contract_digest: digest('c'),
    };
    assert_eq!(
        ContentIdentity::from_references(&artifact, &workspace, &source, digest('b')),
        Err(ContentBindingError::ContentMismatch)
    );
}
