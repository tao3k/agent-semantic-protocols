use super::generation_identity_matches;

#[test]
fn exact_projection_requires_one_generation_identity() {
    let generation_a =
        "blake3-256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let generation_b =
        "blake3-256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    assert!(generation_identity_matches(
        7,
        generation_a,
        7,
        generation_a
    ));
    assert!(!generation_identity_matches(
        7,
        generation_a,
        8,
        generation_a
    ));
    assert!(!generation_identity_matches(
        7,
        generation_a,
        7,
        generation_b
    ));
}
