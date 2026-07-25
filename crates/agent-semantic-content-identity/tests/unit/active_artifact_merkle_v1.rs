use super::{
    ActiveArtifactKindV1, ActiveArtifactLeafV1, ActiveAspArtifactReceiptV1,
    ActiveAspArtifactReceiptV1Error,
};
use crate::exact_selector_merkle::blake3_content_digest_v1;

fn leaf(
    logical_path: &str,
    artifact_kind: ActiveArtifactKindV1,
    bytes: &[u8],
) -> ActiveArtifactLeafV1 {
    ActiveArtifactLeafV1::new(
        logical_path,
        format!("/active/{logical_path}"),
        artifact_kind,
        blake3_content_digest_v1(bytes),
        bytes.len() as u64,
        0,
        None,
    )
    .expect("valid active artifact leaf")
}

#[test]
fn receipt_is_sorted_and_binds_every_leaf() {
    let receipt = ActiveAspArtifactReceiptV1::build(
        "asp-runtime",
        vec![
            leaf(
                "state/activation.json",
                ActiveArtifactKindV1::Activation,
                b"activation",
            ),
            leaf(
                "runtime/bin/by-digest/abc/asp",
                ActiveArtifactKindV1::AspBinary,
                b"asp",
            ),
        ],
    )
    .expect("active artifact receipt");
    assert_eq!(receipt.schema_version, "1");
    assert_eq!(receipt.asp_binary_leaf().size_bytes(), 3);
    assert_eq!(receipt.activation_leaf().size_bytes(), 10);

    let mut changed = receipt.clone();
    let original = &changed.leaves[0];
    changed.leaves[0] = ActiveArtifactLeafV1::new(
        original.logical_path(),
        original.materialized_path(),
        original.artifact_kind(),
        original.artifact_digest().clone(),
        original.size_bytes() + 1,
        original.modified_unix_nanos(),
        original.change_time_unix_nanos(),
    )
    .expect("changed active artifact leaf");
    assert_eq!(
        changed.validate(),
        Err(ActiveAspArtifactReceiptV1Error::RootDigestMismatch)
    );
}

#[test]
fn content_root_is_stable_across_materialization_roots() {
    let activation = leaf(
        "state/activation.json",
        ActiveArtifactKindV1::Activation,
        b"activation",
    );
    let binary = leaf(
        "runtime/bin/by-digest/abc/asp",
        ActiveArtifactKindV1::AspBinary,
        b"asp",
    );
    let receipt =
        ActiveAspArtifactReceiptV1::build("asp-runtime", vec![activation.clone(), binary.clone()])
            .expect("canonical receipt");

    let alias = ActiveArtifactLeafV1::new(
        binary.logical_path(),
        "/workspace/.bin/.asp-artifacts/blake3-256/abc/asp",
        binary.artifact_kind(),
        binary.artifact_digest().clone(),
        binary.size_bytes(),
        binary.modified_unix_nanos(),
        binary.change_time_unix_nanos(),
    )
    .expect("aliased active artifact leaf");
    let aliased = ActiveAspArtifactReceiptV1::build("asp-runtime", vec![activation, alias])
        .expect("aliased receipt");

    assert_eq!(receipt.artifact_root_digest, aliased.artifact_root_digest);
    assert_ne!(
        receipt.materialization_root_digest,
        aliased.materialization_root_digest
    );
}

#[test]
fn receipt_rejects_duplicate_or_missing_required_leaves() {
    let binary = leaf(
        "runtime/bin/by-digest/abc/asp",
        ActiveArtifactKindV1::AspBinary,
        b"asp",
    );
    assert!(matches!(
        ActiveAspArtifactReceiptV1::build("asp-runtime", vec![binary]),
        Err(ActiveAspArtifactReceiptV1Error::ActivationLeafCount(0))
    ));
}
