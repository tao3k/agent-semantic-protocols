use super::apply_verified_child_registration_context;

fn valid_receipt() -> String {
    serde_json::json!({
        "schemaId": "agent.child-session-registration",
        "schemaVersion": 1,
        "generation": 7,
        "lifecycleState": "live",
        "routable": true,
        "registrationAuthority": "project-child-registration-authority",
        "hostCallReceiptDigest": format!("blake3-256:{}", "a".repeat(64)),
        "childSessionId": "child-session",
        "residentId": "asp_explorer",
        "definitionSchemaId": "urn:agent-semantic-protocols:schema:codex-agent-definition",
        "deniedActions": ["edit"],
        "allowedRuleIntents": ["reasoning-search"],
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
        "project-child-registration-authority"
    );
    assert_eq!(payload["registered_agent_name"], "asp_explorer");
    assert_eq!(
        payload["registered_denied_actions"],
        serde_json::json!(["edit"])
    );
    assert_eq!(
        payload["registered_allowed_rule_intents"],
        serde_json::json!(["reasoning-search"])
    );
}

#[test]
fn empty_rule_intent_scope_fails_closed() {
    let mut receipt: serde_json::Value =
        serde_json::from_str(&valid_receipt()).expect("valid receipt fixture");
    receipt["allowedRuleIntents"] = serde_json::json!([]);
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
    });

    let error = apply_verified_child_registration_context(&mut payload, &receipt.to_string())
        .expect_err("empty capability scope must fail closed");

    assert_eq!(error, "child-session-registration-receipt-scope-invalid");
    assert!(payload.get("registration_verified").is_none());
}

#[test]
fn empty_permission_receipt_fails_closed() {
    let mut receipt: serde_json::Value =
        serde_json::from_str(&valid_receipt()).expect("valid receipt fixture");
    receipt["deniedActions"] = serde_json::json!([]);
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "child-session",
        "agent_type": "asp_explorer",
    });

    let error = apply_verified_child_registration_context(&mut payload, &receipt.to_string())
        .expect_err("empty denied permissions must fail closed");

    assert_eq!(
        error,
        "child-session-registration-receipt-permissions-invalid"
    );
    assert!(payload.get("registration_verified").is_none());
}

#[test]
fn receipt_must_bind_the_exact_db_registered_child_identity() {
    let mut payload = serde_json::json!({
        "is_subagent": true,
        "agent_id": "different-child-session",
        "agent_type": "asp_explorer",
    });

    let error = apply_verified_child_registration_context(&mut payload, &valid_receipt())
        .expect_err("mismatched child identity must fail closed");

    assert_eq!(
        error,
        "child-session-registration-receipt-payload-binding-mismatch"
    );
    assert!(payload.get("registration_verified").is_none());
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
