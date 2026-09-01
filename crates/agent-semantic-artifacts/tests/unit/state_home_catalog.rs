//! Pure State Home catalog contract tests.

use crate::{
    CatalogGeneration, CatalogObservation, ProjectBinding, RetainedObject, RetentionLease,
    RetentionObjectKind, admit_state_home_catalog_batch,
};

fn observation(object_id: &str, lease_object_id: &str) -> CatalogObservation {
    let root = std::path::Path::new("/workspace");
    CatalogObservation {
        binding: ProjectBinding::resolve(None, "repo", root).unwrap(),
        object: RetainedObject {
            object_id: object_id.to_string(),
            kind: RetentionObjectKind::Artifact,
            last_observed_at_ms: 10,
            byte_count: 1,
        },
        leases: vec![RetentionLease {
            lease_id: "active".to_string(),
            object_id: lease_object_id.to_string(),
            owner: "runtime".to_string(),
            expires_at_ms: None,
        }],
        observed_at_ms: 10,
    }
}

#[test]
fn nonempty_batch_advances_exactly_one_generation() {
    let observations = [
        observation("artifact:first", "artifact:first"),
        observation("artifact:second", "artifact:second"),
    ];

    let receipt = admit_state_home_catalog_batch(CatalogGeneration::new(41), &observations)
        .expect("admit valid catalog batch");

    assert_eq!(receipt.generation.get(), 42);
    assert_eq!(receipt.observation_count, 2);
}

#[test]
fn empty_batch_preserves_generation() {
    let receipt = admit_state_home_catalog_batch(CatalogGeneration::new(7), &[])
        .expect("admit empty catalog batch");

    assert_eq!(receipt.generation.get(), 7);
    assert_eq!(receipt.observation_count, 0);
}

#[test]
fn mismatched_lease_is_rejected_before_generation_advance() {
    let error = admit_state_home_catalog_batch(
        CatalogGeneration::new(11),
        &[observation("artifact:first", "artifact:different")],
    )
    .expect_err("reject mismatched catalog lease");

    assert!(error.contains("targets a different object"));
}

#[test]
fn empty_object_identity_is_rejected() {
    let error = admit_state_home_catalog_batch(CatalogGeneration::new(11), &[observation("", "")])
        .expect_err("reject empty catalog object identity");

    assert!(error.contains("object id must not be empty"));
}

#[test]
fn generation_overflow_fails_closed() {
    let error = admit_state_home_catalog_batch(
        CatalogGeneration::new(u64::MAX),
        &[observation("artifact:first", "artifact:first")],
    )
    .expect_err("reject catalog generation overflow");

    assert_eq!(error, "State Home catalog generation overflow");
}
