// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! State Home retention tests.

use crate::CleanupDisposition;
use crate::CleanupSelection;
use crate::RetainedObject;
use crate::RetentionLease;
use crate::RetentionObjectKind;
use crate::RetentionPlanner;

fn object(id: &str, observed: u64, bytes: u64) -> RetainedObject {
    RetainedObject {
        object_id: id.to_string(),
        kind: RetentionObjectKind::Workspace,
        last_observed_at_ms: observed,
        byte_count: bytes,
    }
}

#[test]
fn active_lease_wins_over_age_and_expired_unleased_state_retires() {
    let plan = RetentionPlanner::new(200, 50)
        .plan(
            vec![object("active", 0, 10), object("expired", 100, 20)],
            &[RetentionLease {
                lease_id: "runtime-active".to_string(),
                object_id: "active".to_string(),
                owner: "runtime".to_string(),
                expires_at_ms: None,
            }],
        )
        .unwrap();

    assert_eq!(plan.retained_count, 1);
    assert_eq!(plan.retired_count, 1);
    assert_eq!(plan.retained_bytes, 10);
    assert_eq!(plan.retired_bytes, 20);
    assert!(matches!(
        plan.entries[0].disposition,
        CleanupDisposition::Keep { .. }
    ));
    assert!(matches!(
        plan.entries[1].disposition,
        CleanupDisposition::Retire { .. }
    ));
}

#[test]
fn planner_is_deterministic_and_rejects_identity_aliasing() {
    let planner = RetentionPlanner::new(100, 10);
    let left = planner
        .plan(vec![object("b", 100, 1), object("a", 100, 1)], &[])
        .unwrap();
    let right = planner
        .plan(vec![object("a", 100, 1), object("b", 100, 1)], &[])
        .unwrap();
    assert_eq!(left, right);
    assert!(
        planner
            .plan(vec![object("same", 0, 1), object("same", 0, 2)], &[])
            .is_err()
    );
}

#[test]
fn exact_cleanup_selection_is_receipted_and_missing_targets_fail_closed() {
    let planner = RetentionPlanner::new(100, 10);
    let selection = CleanupSelection::ObjectId {
        object_id: "workspace-cache:one".to_string(),
    };
    let plan = planner
        .plan_selected(
            vec![object("workspace-cache:one", 0, 7)],
            &[],
            selection.clone(),
        )
        .expect("plan one exact retained object");
    assert_eq!(plan.selection, selection);
    assert_eq!(plan.retired_count, 1);

    let error = planner
        .plan_selected(
            Vec::new(),
            &[],
            CleanupSelection::ObjectId {
                object_id: "workspace-cache:missing".to_string(),
            },
        )
        .expect_err("an exact cleanup target must not silently match nothing");
    assert!(error.contains("state-home-cleanup-selection-no-match"));
}
