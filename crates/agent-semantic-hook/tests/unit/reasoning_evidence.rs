use super::{
    ReasoningEvidence, ReasoningEvidenceSource, ReasoningEvidenceVisibility, ReasoningVerdict,
    reduce_reasoning_evidence,
};

fn evidence(
    source: ReasoningEvidenceSource,
    visibility: ReasoningEvidenceVisibility,
    value: Option<&str>,
) -> ReasoningEvidence {
    ReasoningEvidence {
        root_session_id: "root".to_string(),
        child_session_id: "child".to_string(),
        resident_generation: None,
        value: value.map(str::to_string),
        visibility,
        source,
        observed_at: Some(1),
        profile_digest: (source == ReasoningEvidenceSource::TypedRoleProfile)
            .then(|| "profile-digest".to_string()),
    }
}

#[test]
fn direct_runtime_match_preserves_direct_observation() {
    let assessment = reduce_reasoning_evidence(
        "low",
        &[evidence(
            ReasoningEvidenceSource::CodexThreadRuntime,
            ReasoningEvidenceVisibility::Observed,
            Some("low"),
        )],
    );
    assert_eq!(assessment.verdict, ReasoningVerdict::DirectMatch);
    assert_eq!(assessment.observed_reasoning_effort.as_deref(), Some("low"));
    assert_eq!(
        assessment.effective_reasoning_effort.as_deref(),
        Some("low")
    );
}

#[test]
fn omitted_runtime_with_typed_profile_is_attested_but_not_observed() {
    let assessment = reduce_reasoning_evidence(
        "low",
        &[
            evidence(
                ReasoningEvidenceSource::CodexThreadRuntime,
                ReasoningEvidenceVisibility::FieldOmitted,
                None,
            ),
            evidence(
                ReasoningEvidenceSource::TypedRoleProfile,
                ReasoningEvidenceVisibility::Observed,
                Some("low"),
            ),
        ],
    );
    assert_eq!(
        assessment.verdict,
        ReasoningVerdict::ProfileAttestedUnobservable
    );
    assert_eq!(assessment.observed_reasoning_effort, None);
    assert_eq!(
        assessment.effective_reasoning_effort.as_deref(),
        Some("low")
    );
}

#[test]
fn transport_failure_is_not_relabelled_as_unobservable_attestation() {
    let assessment = reduce_reasoning_evidence(
        "low",
        &[
            evidence(
                ReasoningEvidenceSource::CodexThreadRuntime,
                ReasoningEvidenceVisibility::TransportFailed,
                None,
            ),
            evidence(
                ReasoningEvidenceSource::TypedRoleProfile,
                ReasoningEvidenceVisibility::Observed,
                Some("low"),
            ),
        ],
    );
    assert_eq!(assessment.verdict, ReasoningVerdict::TransientlyUnavailable);
}

#[test]
fn direct_mismatch_overrides_profile_attestation() {
    let assessment = reduce_reasoning_evidence(
        "low",
        &[
            evidence(
                ReasoningEvidenceSource::SubagentStart,
                ReasoningEvidenceVisibility::Observed,
                Some("high"),
            ),
            evidence(
                ReasoningEvidenceSource::TypedRoleProfile,
                ReasoningEvidenceVisibility::Observed,
                Some("low"),
            ),
        ],
    );
    assert_eq!(assessment.verdict, ReasoningVerdict::DirectMismatch);
    assert_eq!(
        assessment.observed_reasoning_effort.as_deref(),
        Some("high")
    );
}

#[test]
fn rollout_value_remains_stale_instead_of_becoming_direct_runtime_evidence() {
    let assessment = reduce_reasoning_evidence(
        "low",
        &[evidence(
            ReasoningEvidenceSource::RolloutHeader,
            ReasoningEvidenceVisibility::Observed,
            Some("low"),
        )],
    );
    assert_eq!(assessment.verdict, ReasoningVerdict::StaleEvidence);
    assert_eq!(assessment.observed_reasoning_effort, None);
    assert_eq!(
        assessment.effective_reasoning_effort.as_deref(),
        Some("low")
    );
}

/// Injectable host-evidence fixture. Tests control omission, transport failure,
/// and contradictory direct observations without fabricating Codex events.
struct HostEvidenceAdapterFixture {
    evidence: Vec<ReasoningEvidence>,
}

impl HostEvidenceAdapterFixture {
    fn new(evidence: Vec<ReasoningEvidence>) -> Self {
        Self { evidence }
    }

    fn evidence(&self) -> &[ReasoningEvidence] {
        &self.evidence
    }
}

#[test]
fn conflicting_direct_host_evidence_is_never_profile_attested() {
    let host = HostEvidenceAdapterFixture::new(vec![
        evidence(
            ReasoningEvidenceSource::SubagentStart,
            ReasoningEvidenceVisibility::Observed,
            Some("low"),
        ),
        evidence(
            ReasoningEvidenceSource::CodexThreadRuntime,
            ReasoningEvidenceVisibility::Observed,
            Some("high"),
        ),
        evidence(
            ReasoningEvidenceSource::TypedRoleProfile,
            ReasoningEvidenceVisibility::Observed,
            Some("low"),
        ),
    ]);

    let assessment = reduce_reasoning_evidence("low", host.evidence());
    assert_eq!(assessment.verdict, ReasoningVerdict::ConflictingEvidence);
    assert_eq!(assessment.observed_reasoning_effort, None);
}
