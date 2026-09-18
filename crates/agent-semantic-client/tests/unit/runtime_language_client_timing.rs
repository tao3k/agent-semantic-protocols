use super::build_playbook_client_timing_witness;

#[test]
fn playbook_witness_preserves_real_phase_order_and_request_binding() {
    let witness = build_playbook_client_timing_witness(
        "asp-client-session-1",
        "dispatch-request-1",
        13,
        21,
        34,
    )
    .expect("valid Playbook client timing witness");

    assert_eq!(
        witness
            .phases
            .each_ref()
            .map(|phase| (phase.name.as_str(), phase.elapsed_micros)),
        [
            ("launcher", 13),
            ("client-frame-encode", 21),
            ("ipc-connect", 34),
        ]
    );
    witness
        .admit_for_request("asp-client-session-1", "dispatch-request-1")
        .expect("same request binding");
    assert_eq!(
        witness
            .admit_for_request("asp-client-session-1", "dispatch-request-2")
            .expect_err("foreign request")
            .reason_kind(),
        "runtime-search-client-timing-identity-mismatch"
    );
}
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later
