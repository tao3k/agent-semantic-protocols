use super::apply_verified_child_registration_context;

fn valid_receipt() -> String {
    serde_json::json!({
        "generation": 7,
        "lifecycleState": "live",
        "routable": true,
        "registrationAuthority": "codex-host-call-receipt-v1",
        "hostCallReceiptDigest": format!("blake3-256:{}", "a".repeat(64)),
    })
    .to_string()
}

#[test]
fn verified_host_receipt_projects_capability_context() {
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
    });

    apply_verified_child_registration_context(&mut payload, &valid_receipt())
        .expect("project verified registration context");

    assert_eq!(payload["registration_verified"], true);
    assert_eq!(payload["registration_generation"], 7);
    assert_eq!(
        payload["registration_authority"],
        "codex-host-call-receipt-v1"
    );
}

#[test]
fn non_routable_receipt_never_projects_capability_context() {
    let mut receipt: serde_json::Value =
        serde_json::from_str(&valid_receipt()).expect("valid receipt fixture");
    receipt["routable"] = serde_json::Value::Bool(false);
    let mut payload = serde_json::json!({"is_subagent": true});

    let error = apply_verified_child_registration_context(&mut payload, &receipt.to_string())
        .expect_err("non-routable receipt must fail closed");

    assert_eq!(error, "child-session-registration-receipt-not-routable");
    assert!(payload.get("registration_verified").is_none());
}
