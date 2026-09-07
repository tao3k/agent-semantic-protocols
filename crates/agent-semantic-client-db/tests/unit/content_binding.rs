// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_client_db::runtime_server_workspace::content_binding::ContentPublicationLedger;
use agent_semantic_content_identity::content_binding::AuthorityStamp;
use agent_semantic_content_identity::content_binding::ContentBinding;
use agent_semantic_content_identity::content_binding::ContentBindingError;
use agent_semantic_content_identity::content_binding::ContentIdentity;

fn identity(seed: char) -> ContentIdentity {
    let digest = format!("blake3-256:{}", seed.to_string().repeat(64));
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
fn the_server_has_one_active_content_commit() {
    let mut ledger = ContentPublicationLedger::default();
    let first_identity = identity('a');
    let first = ledger
        .publish(None, first_identity.clone(), authority(&first_identity))
        .unwrap()
        .clone();
    assert!(first.predecessor_commit_digest.is_none());
    let second_identity = identity('b');
    let second = ledger
        .publish(
            Some(first.commit_digest.as_str()),
            second_identity.clone(),
            authority(&second_identity),
        )
        .unwrap()
        .clone();
    assert_ne!(first.commit_digest, second.commit_digest);
    assert_eq!(
        second.predecessor_commit_digest.as_deref(),
        Some(first.commit_digest.as_str())
    );
    assert_eq!(ledger.active().unwrap(), &second);
}

#[test]
fn stale_expected_digest_cannot_replace_active_content() {
    let mut ledger = ContentPublicationLedger::default();
    let first_identity = identity('c');
    let first = ledger
        .publish(None, first_identity.clone(), authority(&first_identity))
        .unwrap()
        .clone();
    let replacement_identity = identity('d');
    assert_eq!(
        ledger.publish(
            Some("blake3-256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"),
            replacement_identity.clone(),
            authority(&replacement_identity),
        ),
        Err(ContentBindingError::ContentMismatch)
    );
    assert_eq!(ledger.active().unwrap(), &first);
}

#[test]
fn rollback_without_authority_is_rejected() {
    let mut ledger = ContentPublicationLedger::default();
    let content = identity('e');
    ledger
        .publish(None, content.clone(), authority(&content))
        .unwrap();
    assert_eq!(
        ledger.clear_without_authority(),
        Err(ContentBindingError::RollbackRequiresAuthority)
    );
    assert!(ledger.active().is_some());
}

#[test]
fn publication_point_validates_the_schema_shaped_binding() {
    let mut ledger = ContentPublicationLedger::default();
    let content = identity('f');
    let binding = ContentBinding::new(content.clone(), authority(&content)).unwrap();
    assert!(ledger.publish_binding(None, binding).is_ok());
    assert_eq!(ledger.admit_exact(&content).unwrap().identity(), &content);
}

#[test]
fn admission_rejects_a_tampered_schema_binding() {
    let mut ledger = ContentPublicationLedger::default();
    let content = identity('g');
    let mut binding = ContentBinding::new(content.clone(), authority(&content)).unwrap();
    ledger.publish_binding(None, binding.clone()).unwrap();
    binding.schema_version = "2".to_owned();
    assert_eq!(
        ledger.admit_binding(&binding),
        Err(ContentBindingError::InvalidCommitFence)
    );
    assert_eq!(ledger.admit_exact(&content).unwrap().identity(), &content);
}

#[test]
fn admission_rejects_a_forged_authority_stamp_for_equal_content() {
    let mut ledger = ContentPublicationLedger::default();
    let content = identity('8');
    let canonical = ContentBinding::new(content.clone(), authority(&content)).unwrap();
    ledger.publish_binding(None, canonical).unwrap();

    let forged = ContentBinding::new(
        content.clone(),
        AuthorityStamp {
            key_id: "foreign-authority".to_owned(),
            canonical_digest: content.digest(),
            signature: "foreign-signature".to_owned(),
        },
    )
    .expect("the forged stamp is structurally valid but not independently admitted");

    assert_eq!(
        ledger.admit_binding(&forged),
        Err(ContentBindingError::ContentMismatch)
    );
}
