use crate::runtime_candidate_state::CandidateState;

#[test]
fn supersession_policy_is_explicit() {
    assert!(CandidateState::parse("accepted").may_be_superseded());
    assert!(!CandidateState::parse("promoting").may_be_superseded());
    assert!(CandidateState::parse("promoting").is_promoting());
}
