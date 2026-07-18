use super::matches_resident_slot;

#[test]
fn rejects_cross_resident_runtime_observation() {
    assert!(!matches_resident_slot(
        "asp_testing",
        Some("testing-child"),
        "asp_explorer",
        "testing-child",
    ));
}

#[test]
fn rejects_stale_generation_for_same_resident() {
    assert!(!matches_resident_slot(
        "asp_testing",
        Some("current-testing-child"),
        "asp_testing",
        "old-testing-child",
    ));
}

#[test]
fn accepts_matching_resident_and_generation() {
    assert!(matches_resident_slot(
        "release_builder",
        Some("release-child"),
        "release_builder",
        "release-child",
    ));
}
