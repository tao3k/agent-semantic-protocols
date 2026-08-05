use super::reopened_generation_covers_ready_owner;

#[test]
fn superseding_generation_is_admitted_by_ready_owner_content_identity() {
    assert!(reopened_generation_covers_ready_owner(
        Some("blake3-256:owner-current"),
        Some("blake3-256:owner-current"),
    ));
    assert!(reopened_generation_covers_ready_owner(None, None));
    assert!(!reopened_generation_covers_ready_owner(
        Some("blake3-256:owner-ready"),
        Some("blake3-256:owner-stale"),
    ));
    assert!(!reopened_generation_covers_ready_owner(
        Some("blake3-256:owner-ready"),
        None,
    ));
}
