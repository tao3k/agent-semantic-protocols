// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{SessionValidationReport, normalized_metadata, sandbox_verification_status};

#[test]
fn sandbox_drift_is_explicit_without_becoming_a_ready_gate() {
    assert_eq!(
        sandbox_verification_status(Some("read-only"), Some("danger-full-access")),
        "host-inherited-drift-warning"
    );
    assert_eq!(
        sandbox_verification_status(Some("read-only"), Some("read-only")),
        "matched"
    );
}

#[test]
fn normalized_metadata_projects_machine_readable_warning_only_sandbox_drift() {
    let validation = SessionValidationReport::new(
        "warning".to_string().into(),
        "sandbox drift is warning-only",
    )
    .with_roles(
        Some("asp_explorer".to_string()),
        Some("asp_explorer".to_string()),
    )
    .with_models(
        Some("gpt-5.4-mini".to_string()),
        Some("gpt-5.4-mini".to_string()),
    )
    .with_reasoning_efforts(Some("low".to_string()), Some("low".to_string()))
    .with_sandboxes(
        Some("read-only".to_string()),
        Some("danger-full-access".to_string()),
    );

    let metadata = normalized_metadata(None, &validation).expect("normalized metadata");
    let metadata: serde_json::Value = serde_json::from_str(&metadata).expect("metadata JSON");
    let projected = &metadata["validation"];
    assert_eq!(
        projected["sandboxVerificationStatus"],
        "host-inherited-drift-warning"
    );
    assert_eq!(projected["sandboxPolicy"], "warning-only-host-inherited");
    assert_eq!(projected["sandboxAffectsReady"], false);
}
