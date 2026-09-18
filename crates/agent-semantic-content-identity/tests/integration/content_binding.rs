// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentBindingError;
use agent_semantic_content_identity::content_binding::ContentIdentity;
use agent_semantic_content_identity::content_binding::ContentPublicationCommit;

fn identity() -> ContentIdentity {
    let digest = "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    ContentIdentity {
        runtime_artifact_digest: digest.to_owned(),
        workspace_snapshot_digest: digest.to_owned(),
        source_generation_digest: digest.to_owned(),
        source_index_digest: digest.to_owned(),
        schema_digest: digest.to_owned(),
        provider_catalog_digest: digest.to_owned(),
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
fn exact_admission_accepts_only_the_complete_identity() {
    let content = identity();
    let commit = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    assert!(commit.predecessor_commit_digest.is_none());
    assert!(commit.admit_exact(&commit.content_binding).is_ok());

    let mut changed = identity();
    changed.provider_catalog_digest =
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
    let changed_authority = authority(&changed);
    let changed_binding = ContentBinding::new(changed, changed_authority).unwrap();
    assert_eq!(
        commit.admit_exact(&changed_binding),
        Err(ContentBindingError::ContentMismatch)
    );
}

#[test]
fn schema_version_is_in_the_binding_not_the_rust_namespace() {
    let content = identity();
    let binding = ContentBinding::new(content.clone(), authority(&content)).unwrap();
    assert_eq!(binding.schema_version, "1");
    assert!(binding.validate().is_ok());
    let json = serde_json::to_value(&binding).unwrap();
    assert_eq!(json["schemaId"], "asp.content-binding");
    assert_eq!(json["schemaVersion"], "1");
    assert!(json.get("runtimeArtifactDigest").is_some());
    assert!(json.get("runtime_artifact_digest").is_none());
    assert_eq!(json["authorityStamp"]["canonicalDigest"], content.digest());
    assert!(json.get("contentBindingVersion").is_none());
}

#[test]
fn schema_version_tampering_is_rejected_before_publication() {
    let content = identity();
    let mut binding = ContentBinding::new(content.clone(), authority(&content)).unwrap();
    binding.schema_version = "2".to_owned();
    assert_eq!(
        binding.validate(),
        Err(ContentBindingError::InvalidCommitFence)
    );
}

#[test]
fn publication_commit_uses_the_authority_and_fence_schema_shape() {
    let content = identity();
    let commit = ContentPublicationCommit::linearize(content.clone(), authority(&content)).unwrap();
    let json = serde_json::to_value(&commit).unwrap();
    assert_eq!(
        json["schemaId"],
        "agent.semantic-protocols.content-publication-commit"
    );
    assert_eq!(json["schemaVersion"], "1");
    assert_eq!(
        json["contentBinding"]["authorityStamp"]["keyId"],
        "test-authority"
    );
    assert!(json["predecessorCommitDigest"].is_null());
    assert!(json.get("durable").is_none());
    assert!(json.get("identity").is_none());
}
