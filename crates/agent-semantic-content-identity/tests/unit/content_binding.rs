// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
fn authority_stamp_is_part_of_publication_commit_identity() {
    let content = identity('a');
    let first = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    let second = ContentPublicationCommit::linearize(
        content.clone(),
        AuthorityStamp {
            key_id: "other-authority".to_owned(),
            canonical_digest: content.digest(),
            signature: "other-signature".to_owned(),
        },
    )
    .unwrap();
    assert_ne!(first.commit_digest, second.commit_digest);
}

#[test]
fn predecessor_fence_is_part_of_publication_commit_identity() {
    let content = identity('a');
    let first = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    let successor = ContentPublicationCommit::linearize_with_expected(
        content.clone(),
        authority(&content),
        Some(&first.commit_digest),
    )
    .unwrap();
    assert_ne!(first.commit_digest, successor.commit_digest);
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
    let different_authority = authority(&different);
    let different_binding = super::ContentBinding::new(different, different_authority).unwrap();
    assert!(observation.matches_commit(&commit));
    assert_eq!(
        commit.admit_exact(&different_binding),
        Err(ContentBindingError::ContentMismatch)
    );
    assert!(!observation.is_product_authority());
}

#[test]
fn corrupted_commit_and_schema_identity_fail_closed() {
    let identity = identity('d');
    let authority = authority(&identity);
    let mut commit = ContentPublicationCommit::linearize(identity, authority).unwrap();
    commit.commit_digest = format!("{DIGEST_PREFIX}{}", "e".repeat(64));
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::CommitDigestMismatch)
    );
    commit = ContentPublicationCommit::linearize(
        commit.identity().clone(),
        commit.authority_stamp().clone(),
    )
    .unwrap();
    commit.schema_version = "2".to_owned();
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::InvalidCommitFence)
    );
}

#[test]
fn invalid_transaction_fence_fails_closed() {
    let content = identity('e');
    let mut commit =
        ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    commit.predecessor_commit_digest = Some(format!("{DIGEST_PREFIX}{}", "f".repeat(64)));
    assert_eq!(
        commit.validate(),
        Err(ContentBindingError::CommitDigestMismatch)
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
