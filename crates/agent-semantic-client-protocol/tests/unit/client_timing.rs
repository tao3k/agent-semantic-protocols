use super::RuntimeSearchClientTimingWitness;

#[test]
fn witness_is_ordered_and_bound_only_to_session_and_request() {
    let witness = RuntimeSearchClientTimingWitness::new("session-1", "request-1", [1, 2, 3])
        .expect("canonical witness");
    assert_eq!(witness.phases[1].name, "client-frame-encode");
    witness
        .admit_for_request("session-1", "request-1")
        .expect("matching Host correlation");
    let error = witness
        .admit_for_request("session-1", "request-2")
        .expect_err("foreign request must fail closed");
    assert_eq!(
        error.reason_kind(),
        "runtime-search-client-timing-identity-mismatch"
    );
}

#[test]
fn reordered_client_phase_is_rejected() {
    let mut witness = RuntimeSearchClientTimingWitness::new("session-1", "request-1", [1, 2, 3])
        .expect("canonical witness");
    witness.phases.swap(0, 1);
    assert_eq!(
        witness
            .validate()
            .expect_err("reordered phase")
            .reason_kind(),
        "runtime-search-client-timing-phase-order"
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
