use crate::ClosureDisposition;
use crate::Digest;
use crate::ProtocolId;
use crate::SearchClosureReceipt;
use crate::UncheckedContextProductStateV1;
use crate::ValidationError;

#[test]
fn closure_receipt_digest_excludes_post_transition_digests() {
    let mut receipt: SearchClosureReceipt = serde_json::from_value(serde_json::json!({
        "eventType": "ClosureFinalized",
        "receiptId": "closure-receipt-1",
        "eventId": "closure-event-1",
        "runId": "run-1",
        "sequence": 4,
        "stateRevision": 4,
        "preStateDigest": digest("pre-state"),
        "contextBindingDigest": digest("context"),
        "intentDigest": digest("intent"),
        "routeProgramId": "program-1",
        "programDigest": digest("program"),
        "closedObligations": [{
            "obligationId": "obligation-1",
            "claimClass": "identity",
            "verdict": "supported",
            "proofRefs": ["proof-1"],
            "supportPaths": ["src/lib.rs"],
            "evidenceScope": {
                "scopeDigest": digest("scope"),
                "complete": true,
                "truncated": false,
                "snapshotContinuous": true,
                "providerAdmitted": true
            }
        }],
        "finalizedAtMs": 10,
        "receiptDigest": digest("placeholder")
    }))
    .expect("closure receipt must deserialize");

    let first = receipt.recompute_receipt_digest();
    receipt.receipt_digest = Digest::from_bytes(b"different-placeholder");
    assert_eq!(first, receipt.recompute_receipt_digest());

    let wire = serde_json::to_value(&receipt).expect("closure receipt must serialize");
    assert!(wire.get("eventLogDigest").is_none());
    assert!(wire.get("finalStateDigest").is_none());
}

#[test]
fn search_decision_must_cover_every_required_unresolved_obligation() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/context-product-state.v1.fixtures.json"
    ))
    .expect("fixtures must parse");
    let mut state: UncheckedContextProductStateV1 = serde_json::from_value(
        fixtures["fixtures"]
            .as_array()
            .expect("fixtures array")
            .iter()
            .find(|fixture| fixture["name"] == "valid-open-search-state")
            .expect("open state fixture")["value"]
            .clone(),
    )
    .expect("open state must deserialize");

    let mut second = state.obligations[0].clone();
    second.obligation_id = ProtocolId::parse("obligation-2").expect("valid id");
    state.obligations.push(second);
    state.closure = ClosureDisposition::Open {
        open_obligation_ids: vec![
            ProtocolId::parse("obligation-1").expect("valid id"),
            ProtocolId::parse("obligation-2").expect("valid id"),
        ],
    };
    state.context.binding_digest = state.context.recompute_binding_digest();
    state.frontier.context_binding_digest = state.context.binding_digest.clone();
    state.frontier.frontier_digest = state.frontier.recompute_frontier_digest();
    state.spent_action_ledger_digest = state.recompute_spent_action_ledger_digest();
    state.state_digest = state.recompute_state_digest();

    assert!(matches!(
        state.validate(),
        Err(ValidationError::DecisionObligationSetMismatch)
    ));
}

fn digest(label: &str) -> String {
    Digest::from_bytes(label.as_bytes()).as_str().to_owned()
}
