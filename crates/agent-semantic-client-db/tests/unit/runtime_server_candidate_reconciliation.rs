use agent_semantic_client_db::runtime_server_candidate_reconciliation::{CandidateReconciliationOutcome, PreviousCandidatePromotionEnvelope, validate_promotion_envelope};
use std::path::Path;

#[test]
fn outcome_is_versioned_in_data_not_type_name() {
    let outcome = CandidateReconciliationOutcome::failed("candidate-proof-mismatch", "invalid proof");
    assert_eq!(outcome.schema_version, "1");
    assert_eq!(outcome.reason_kind, "candidate-proof-mismatch");
}

fn envelope() -> PreviousCandidatePromotionEnvelope {
    PreviousCandidatePromotionEnvelope { desired_identity: "identity".into(), executable_path: "/bin/asp".into(), nonce: "nonce".into(), process_id: 42, schema_version: "1".into(), started_at: 1, state: "promoting".into(), state_home: "/tmp/state".into() }
}

#[test]
fn promotion_envelope_accepts_complete_proof() { assert!(validate_promotion_envelope(&envelope(), Path::new("/tmp/state")).is_ok()); }

#[test]
fn promotion_envelope_rejects_wrong_schema_state_home_identity_and_pid() {
    for mutate in [
        |value: &mut PreviousCandidatePromotionEnvelope| value.schema_version = "2".into(),
        |value| value.state = "accepted".into(),
        |value| value.state_home = "/tmp/other".into(),
        |value| value.desired_identity.clear(),
        |value| value.process_id = 0,
    ] {
        let mut value = envelope(); mutate(&mut value);
        assert_eq!(validate_promotion_envelope(&value, Path::new("/tmp/state")), Err("candidate-proof-mismatch"));
    }
}
