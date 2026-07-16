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
    let validation = SessionValidationReport {
        status: "warning".to_string(),
        reason: "sandbox drift is warning-only".to_string(),
        config_path: None,
        rollout_path: None,
        expected_root_session_id: None,
        actual_root_session_id: None,
        expected_parent_thread_id: None,
        actual_parent_thread_id: None,
        expected_agent_path: None,
        actual_agent_path: None,
        expected_role: Some("asp_explorer".to_string()),
        actual_role: Some("asp_explorer".to_string()),
        expected_model: Some("gpt-5.4-mini".to_string()),
        actual_model: Some("gpt-5.4-mini".to_string()),
        expected_reasoning_effort: Some("low".to_string()),
        actual_reasoning_effort: Some("low".to_string()),
        expected_sandbox: Some("read-only".to_string()),
        actual_sandbox: Some("danger-full-access".to_string()),
    };

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
