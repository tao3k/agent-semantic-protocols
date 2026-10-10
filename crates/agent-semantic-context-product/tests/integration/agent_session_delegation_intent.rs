// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationCapability;
use agent_semantic_context_product::agent_session_delegation_intent::AGENT_SESSION_DELEGATION_INTENT_SCHEMA_ID;
use agent_semantic_context_product::agent_session_delegation_intent::AGENT_SESSION_DELEGATION_INTENT_SCHEMA_VERSION;
use agent_semantic_context_product::agent_session_delegation_intent::AgentSessionDelegationIntent;

fn intent() -> AgentSessionDelegationIntent {
    AgentSessionDelegationIntent {
        schema_id: AGENT_SESSION_DELEGATION_INTENT_SCHEMA_ID.to_owned(),
        schema_version: AGENT_SESSION_DELEGATION_INTENT_SCHEMA_VERSION.to_owned(),
        event_id: "event-1".to_owned(),
        project_id: "project-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        current_session_id: "asp-testing-1".to_owned(),
        proposed_child_session_id: "nested-1".to_owned(),
        proposed_child_resident_name: "explorer".to_owned(),
        proposed_child_capability: AgentSessionDelegationCapability::Standard,
        evidence_refs: vec!["payload:blake3-256:abc".to_owned()],
        observed_at_ms: 1,
    }
}

#[test]
fn authority_current_intent_has_no_caller_selected_generation() {
    let intent = intent();
    intent.validate().expect("valid delegation intent");
    let value = serde_json::to_value(intent).expect("serialize delegation intent");
    assert_eq!(
        value["schemaId"],
        serde_json::Value::String(AGENT_SESSION_DELEGATION_INTENT_SCHEMA_ID.to_owned())
    );
    assert_eq!(
        value["schemaVersion"],
        serde_json::Value::String("1".to_owned())
    );
    assert!(value.get("expectedGeneration").is_none());
}

#[test]
fn intent_rejects_duplicate_or_empty_evidence() {
    let mut duplicate = intent();
    duplicate
        .evidence_refs
        .push(duplicate.evidence_refs[0].clone());
    assert_eq!(
        duplicate.validate().expect_err("duplicate evidence denied"),
        "delegation intent evidenceRefs must be unique"
    );

    let mut empty = intent();
    empty.evidence_refs = vec![String::new()];
    assert_eq!(
        empty.validate().expect_err("empty evidence denied"),
        "delegation intent evidenceRefs must be non-empty strings"
    );
}
