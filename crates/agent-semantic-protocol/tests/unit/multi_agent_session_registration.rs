use super::wait_for_registration_handoff;

#[test]
fn registration_handoff_observes_a_delayed_current_receipt() {
    let mut reads = 0;
    let receipt = wait_for_registration_handoff(
        || {
            reads += 1;
            Ok::<_, String>((reads == 3).then_some("current-child"))
        },
        |value| *value == "current-child",
        4,
        std::time::Duration::ZERO,
    )
    .expect("registration handoff");

    assert_eq!(receipt, Some("current-child"));
    assert_eq!(reads, 3);
}

#[test]
fn registration_handoff_never_accepts_a_stale_child_receipt() {
    let mut reads = 0;
    let receipt = wait_for_registration_handoff(
        || {
            reads += 1;
            Ok::<_, String>(Some(if reads < 3 {
                "previous-child"
            } else {
                "current-child"
            }))
        },
        |value| *value == "current-child",
        3,
        std::time::Duration::ZERO,
    )
    .expect("registration handoff");

    assert_eq!(receipt, Some("current-child"));
    assert_eq!(reads, 3);
}

#[test]
fn registration_handoff_is_bounded_when_receipt_is_missing() {
    let started = std::time::Instant::now();
    let receipt = wait_for_registration_handoff(
        || Ok::<Option<()>, String>(None),
        |_| true,
        5,
        std::time::Duration::from_millis(1),
    )
    .expect("registration handoff");

    assert!(receipt.is_none());
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "missing registration exceeded the bounded handoff window"
    );
}
