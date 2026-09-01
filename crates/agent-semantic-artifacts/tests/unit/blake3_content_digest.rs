use super::Blake3ContentDigest;

const REAL_DIGEST: &str =
    "blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1";

#[test]
fn precise_production_digest_roundtrips_through_display_and_serde() {
    let digest = Blake3ContentDigest::parse(REAL_DIGEST).expect("parse production digest");
    assert_eq!(digest.to_string(), REAL_DIGEST);
    let encoded = serde_json::to_string(&digest).expect("encode digest");
    let decoded: Blake3ContentDigest = serde_json::from_str(&encoded).expect("decode digest");
    assert_eq!(decoded, digest);
    assert_eq!(
        decoded.content_digest().as_str(),
        &REAL_DIGEST["blake3-256:".len()..]
    );
}

#[test]
fn malformed_digest_shapes_are_rejected() {
    for invalid in [
        "9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        "blake3-256:9313893f",
        "blake3-256:9313893F2985088DC3E5B14FDFD4877B0ED37C8DC8D5EF60BD6531AB5678ABE1",
        "blake3-256:blake3-256:9313893f2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
        "blake3-256:9313893g2985088dc3e5b14fdfd4877b0ed37c8dc8d5ef60bd6531ab5678abe1",
    ] {
        assert!(
            Blake3ContentDigest::parse(invalid).is_err(),
            "accepted {invalid}"
        );
    }
}
