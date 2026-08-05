use agent_semantic_hook::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind, StdinMode,
};
use std::collections::BTreeMap;

fn decision() -> HookDecision {
    HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_owned(),
        event: "post-tool".to_owned(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: vec!["rust".into()],
        subject: DecisionSubject {
            tool_name: Some("functions.apply_patch".to_owned()),
            command: None,
            paths: vec!["src/lib.rs".to_owned()],
        },
        routes: vec![DecisionRoute {
            language_id: "rust".into(),
            provider_id: "rs-harness".into(),
            binary: "asp".to_owned(),
            kind: DecisionRouteKind::Query,
            argv: vec!["asp".to_owned(), "rust".to_owned(), "query".to_owned()],
            stdin_mode: Some(StdinMode::None),
        }],
        message: "allowed".to_owned(),
        fields: BTreeMap::from([(
            "runtimeGenerationAdmissionStatus".to_owned(),
            "submitted".into(),
        )]),
    }
}

#[test]
fn hook_decision_round_trips_across_the_runtime_server_boundary() {
    let encoded = serde_json::to_string(&decision()).expect("serialize Hook decision");
    let decoded: HookDecision = serde_json::from_str(&encoded).expect("deserialize Hook decision");

    assert_eq!(decoded.schema_id, HOOK_DECISION_SCHEMA_ID);
    assert_eq!(decoded.schema_version, "1");
    assert_eq!(decoded.decision, DecisionKind::Allow);
    assert_eq!(decoded.reason_kind, ReasonKind::None);
    assert_eq!(decoded.subject.paths, ["src/lib.rs"]);
    assert_eq!(
        decoded.fields["runtimeGenerationAdmissionStatus"],
        "submitted"
    );
}

#[test]
fn hook_decision_rejects_runtime_protocol_identity_drift() {
    let mut encoded = serde_json::to_value(decision()).expect("serialize Hook decision");
    encoded["schemaVersion"] = "2".into();

    let error = serde_json::from_value::<HookDecision>(encoded)
        .expect_err("schema version drift must fail closed");
    assert!(error.to_string().contains("expected 1, found 2"));
}

#[test]
fn hook_decision_accepts_a_pathless_runtime_subject() {
    let mut encoded = serde_json::to_value(decision()).expect("serialize Hook decision");
    encoded["subject"]
        .as_object_mut()
        .expect("subject object")
        .remove("paths");

    let decoded = serde_json::from_value::<HookDecision>(encoded)
        .expect("pathless Hook decision remains a valid wire value");
    assert!(decoded.subject.paths.is_empty());
}
