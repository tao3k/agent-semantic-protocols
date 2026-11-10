// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationAdmissionInput;
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationAdmissionReceipt;
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationCapability;
use agent_semantic_context_product::agent_session_delegation_admission::AgentSessionDelegationDecision;

#[test]
fn focused_leaf_denial_is_typed_and_preserves_state() {
    let receipt = AgentSessionDelegationAdmissionReceipt::focused_leaf_denied(
        "project-1",
        "root-1",
        "asp-testing-1",
        "nested-child-1",
        "blake3-256:state",
        vec!["codex-v2:verified-parent".to_owned()],
    )
    .expect("focused leaf receipt");

    assert_eq!(
        receipt.capability,
        AgentSessionDelegationCapability::FocusedLeaf
    );
    assert_eq!(receipt.decision, AgentSessionDelegationDecision::Denied);
    assert_eq!(
        receipt.reason_kind.as_deref(),
        Some("focused-agent-delegation-denied")
    );
    assert_eq!(receipt.pre_state_digest, receipt.post_state_digest);
    assert_eq!(receipt.delegation_generation, None);
}

#[test]
fn denied_receipt_rejects_any_state_mutation() {
    let error = AgentSessionDelegationAdmissionReceipt::new(AgentSessionDelegationAdmissionInput {
        project_id: "project-1".to_owned(),
        root_session_id: "root-1".to_owned(),
        current_session_id: "asp-explorer-1".to_owned(),
        proposed_child_session_id: "nested-child-1".to_owned(),
        capability: AgentSessionDelegationCapability::FocusedLeaf,
        decision: AgentSessionDelegationDecision::Denied,
        reason_kind: Some("focused-agent-delegation-denied".to_owned()),
        pre_state_digest: "blake3-256:before".to_owned(),
        post_state_digest: "blake3-256:after".to_owned(),
        delegation_generation: None,
        evidence_refs: Vec::new(),
    })
    .expect_err("denial cannot mutate control-plane state");

    assert_eq!(error, "denied delegation must preserve control-plane state");
}
