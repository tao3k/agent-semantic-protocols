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
    ActiveArtifactLeafV1 {
        logical_path: logical_path.to_string(),
        materialized_path: format!("/active/{logical_path}"),
        artifact_kind,
        artifact_digest: blake3_content_digest_v1(bytes),
        size_bytes: bytes.len() as u64,
        modified_unix_nanos: 0,
        change_time_unix_nanos: None,
    }
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
    assert_eq!(receipt.asp_binary_leaf().size_bytes, 3);
    assert_eq!(receipt.activation_leaf().size_bytes, 10);

    let mut changed = receipt.clone();
    changed.leaves[0].size_bytes += 1;
    assert_eq!(
        changed.validate(),
        Err(ActiveAspArtifactReceiptV1Error::RootDigestMismatch)
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
