use super::materialize_source_access_deny_message;
use agent_semantic_hook::{
    DecisionKind, DecisionRoute, DecisionRouteKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision,
    ReasonKind,
};

#[test]
fn configured_message_binds_language_and_appends_executable_provider_route() {
    let mut decision = HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: "codex".to_owned(),
        event: "pre-tool".to_owned(),
        decision: DecisionKind::Deny,
        reason_kind: ReasonKind::StructuredSourceRead,
        language_ids: vec![agent_semantic_config::LanguageId::new("rust")],
        subject: DecisionSubject::default(),
        routes: vec![DecisionRoute {
            language_id: agent_semantic_config::LanguageId::new("rust"),
            provider_id: agent_semantic_config::ProviderId::new("rs-harness"),
            binary: "asp".to_owned(),
            kind: DecisionRouteKind::Owner,
            argv: vec![
                "asp".to_owned(),
                "rust".to_owned(),
                "search".to_owned(),
                "owner".to_owned(),
                "src/a file.rs".to_owned(),
                "items".to_owned(),
                "--workspace".to_owned(),
                ".".to_owned(),
            ],
            stdin_mode: None,
        }],
        message: "Registered {{languageId}} source reads are denied.".to_owned(),
        fields: Default::default(),
    };

    materialize_source_access_deny_message(&mut decision);

    assert!(
        decision
            .message
            .starts_with("Registered rust source reads are denied.")
    );
    assert!(
        decision
            .message
            .contains("ASP route: `asp rust search owner 'src/a file.rs' items --workspace .`")
    );
}
