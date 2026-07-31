import ASPProof.AgentSessionHostRegistryQuarantineAdjudication

namespace ASPProof.AgentSessionHostRegistryAdjudicationDecisionCertificate

open ASPProof.AgentSessionHostRegistryCrashSafeMaterializationRecovery
open ASPProof.AgentSessionHostRegistryQuarantineAdjudication

structure TerminalProvenance where
  collectorDigest : Nat
  replayExecutorDigest : Nat
  terminal : TerminalCapture
  sourceSnapshotDigest : Nat
  acquisitionDigest : Nat
  provenanceRoot : Nat
  replayed : Bool
  deriving DecidableEq, Repr

def ProvenanceBindsTerminal
    (expected : TerminalCapture)
    (provenance : TerminalProvenance) : Prop :=
  provenance.terminal = expected ∧
  provenance.replayed = true ∧
  provenance.replayExecutorDigest ≠ provenance.collectorDigest

def IndependentProvenancePair
    (accepted conflicting : TerminalProvenance) : Prop :=
  accepted.collectorDigest ≠ conflicting.collectorDigest ∧
  accepted.provenanceRoot ≠ conflicting.provenanceRoot

inductive EvidenceJudgment where
  | valid
  | invalid
  deriving DecidableEq, Repr

inductive DecisionEntailed :
    AdjudicationDecision → EvidenceJudgment → EvidenceJudgment → Prop where
  | upholdAccepted : DecisionEntailed .upholdAccepted .valid .invalid
  | replaceWithConflicting : DecisionEntailed .replaceWithConflicting .invalid .valid
  | rejectBoth : DecisionEntailed .rejectBoth .invalid .invalid

structure DecisionCertificatePayload where
  quarantineDigest : Nat
  acceptedProvenanceRoot : Nat
  conflictingProvenanceRoot : Nat
  decision : AdjudicationDecision
  acceptedJudgment : EvidenceJudgment
  conflictingJudgment : EvidenceJudgment
  entailmentDigest : Nat
  authorityDigest : Nat
  adjudicatorDigest : Nat
  generation : Nat
  deriving DecidableEq, Repr

structure DecisionCertificate where
  payload : DecisionCertificatePayload
  certificateDigest : Nat
  deriving DecidableEq, Repr

abbrev CertificateDigest := DecisionCertificatePayload → Nat

def ValidDecisionCertificate
    (quarantine : QuarantineCase)
    (accepted conflicting : TerminalProvenance)
    (digestPayload : CertificateDigest)
    (certificate : DecisionCertificate) : Prop :=
  ProvenanceBindsTerminal quarantine.accepted accepted ∧
  ProvenanceBindsTerminal quarantine.conflicting conflicting ∧
  IndependentProvenancePair accepted conflicting ∧
  certificate.payload.quarantineDigest = quarantine.quarantineDigest ∧
  certificate.payload.acceptedProvenanceRoot = accepted.provenanceRoot ∧
  certificate.payload.conflictingProvenanceRoot = conflicting.provenanceRoot ∧
  DecisionEntailed certificate.payload.decision
    certificate.payload.acceptedJudgment
    certificate.payload.conflictingJudgment ∧
  certificate.payload.generation = quarantine.key.generation + 1 ∧
  certificate.certificateDigest = digestPayload certificate.payload

inductive CertificateStatus where
  | active
  | revoked (revocationDigest : Nat)
  deriving DecidableEq, Repr

def AdmittedForNewDecision (status : CertificateStatus) : Prop :=
  status = .active

inductive CertificateLifecycleStep :
    Nat → CertificateStatus → CertificateStatus → Prop where
  | revoke
      (certificateDigest revocationDigest : Nat) :
      CertificateLifecycleStep certificateDigest .active (.revoked revocationDigest)

structure AppealCertificate where
  priorCertificateDigest : Nat
  appealAuthorityDigest : Nat
  successor : DecisionCertificate
  deriving DecidableEq, Repr

def AppealSupersedes
    (prior : DecisionCertificate)
    (appeal : AppealCertificate) : Prop :=
  appeal.priorCertificateDigest = prior.certificateDigest ∧
  appeal.successor.payload.generation = prior.payload.generation + 1 ∧
  appeal.successor.certificateDigest ≠ prior.certificateDigest

theorem provenance_binding_binds_terminal
    {expected : TerminalCapture}
    {provenance : TerminalProvenance}
    (bound : ProvenanceBindsTerminal expected provenance) :
    provenance.terminal = expected := by
  exact bound.1

theorem provenance_binding_requires_replay
    {expected : TerminalCapture}
    {provenance : TerminalProvenance}
    (bound : ProvenanceBindsTerminal expected provenance) :
    provenance.replayed = true := by
  exact bound.2.1

theorem provenance_replay_is_independent
    {expected : TerminalCapture}
    {provenance : TerminalProvenance}
    (bound : ProvenanceBindsTerminal expected provenance) :
    provenance.replayExecutorDigest ≠ provenance.collectorDigest := by
  exact bound.2.2

theorem nonreplayed_provenance_cannot_bind
    (expected : TerminalCapture)
    (provenance : TerminalProvenance)
    (notReplayed : provenance.replayed = false) :
    ¬ ProvenanceBindsTerminal expected provenance := by
  intro bound
  exact Bool.false_ne_true (notReplayed.symm.trans bound.2.1)

theorem self_replayed_provenance_cannot_bind
    (expected : TerminalCapture)
    (provenance : TerminalProvenance)
    (same : provenance.replayExecutorDigest = provenance.collectorDigest) :
    ¬ ProvenanceBindsTerminal expected provenance := by
  intro bound
  exact bound.2.2 same

theorem same_collector_is_not_independent_pair
    (accepted conflicting : TerminalProvenance)
    (same : accepted.collectorDigest = conflicting.collectorDigest) :
    ¬ IndependentProvenancePair accepted conflicting := by
  intro independent
  exact independent.1 same

theorem same_root_is_not_independent_pair
    (accepted conflicting : TerminalProvenance)
    (same : accepted.provenanceRoot = conflicting.provenanceRoot) :
    ¬ IndependentProvenancePair accepted conflicting := by
  intro independent
  exact independent.2 same

theorem uphold_decision_requires_asymmetric_judgments
    {accepted conflicting : EvidenceJudgment}
    (entailed : DecisionEntailed .upholdAccepted accepted conflicting) :
    accepted = .valid ∧ conflicting = .invalid := by
  cases entailed
  exact ⟨rfl, rfl⟩

theorem replace_decision_requires_asymmetric_judgments
    {accepted conflicting : EvidenceJudgment}
    (entailed : DecisionEntailed .replaceWithConflicting accepted conflicting) :
    accepted = .invalid ∧ conflicting = .valid := by
  cases entailed
  exact ⟨rfl, rfl⟩

theorem both_valid_evidence_has_no_decision_entailment
    (decision : AdjudicationDecision) :
    ¬ DecisionEntailed decision .valid .valid := by
  intro entailed
  cases entailed

theorem valid_certificate_binds_both_provenance_roots
    {quarantine : QuarantineCase}
    {accepted conflicting : TerminalProvenance}
    {digestPayload : CertificateDigest}
    {certificate : DecisionCertificate}
    (valid : ValidDecisionCertificate quarantine accepted conflicting
      digestPayload certificate) :
    certificate.payload.acceptedProvenanceRoot = accepted.provenanceRoot ∧
    certificate.payload.conflictingProvenanceRoot = conflicting.provenanceRoot := by
  exact ⟨valid.2.2.2.2.1, valid.2.2.2.2.2.1⟩

theorem valid_certificate_carries_entailment
    {quarantine : QuarantineCase}
    {accepted conflicting : TerminalProvenance}
    {digestPayload : CertificateDigest}
    {certificate : DecisionCertificate}
    (valid : ValidDecisionCertificate quarantine accepted conflicting
      digestPayload certificate) :
    DecisionEntailed certificate.payload.decision
      certificate.payload.acceptedJudgment
      certificate.payload.conflictingJudgment := by
  exact valid.2.2.2.2.2.2.1

theorem valid_certificate_advances_generation
    {quarantine : QuarantineCase}
    {accepted conflicting : TerminalProvenance}
    {digestPayload : CertificateDigest}
    {certificate : DecisionCertificate}
    (valid : ValidDecisionCertificate quarantine accepted conflicting
      digestPayload certificate) :
    certificate.payload.generation = quarantine.key.generation + 1 := by
  exact valid.2.2.2.2.2.2.2.1

theorem valid_certificate_seal_matches_payload
    {quarantine : QuarantineCase}
    {accepted conflicting : TerminalProvenance}
    {digestPayload : CertificateDigest}
    {certificate : DecisionCertificate}
    (valid : ValidDecisionCertificate quarantine accepted conflicting
      digestPayload certificate) :
    certificate.certificateDigest = digestPayload certificate.payload := by
  exact valid.2.2.2.2.2.2.2.2

theorem wrong_certificate_seal_is_rejected
    {quarantine : QuarantineCase}
    {accepted conflicting : TerminalProvenance}
    {digestPayload : CertificateDigest}
    {certificate : DecisionCertificate}
    (wrong : certificate.certificateDigest ≠ digestPayload certificate.payload) :
    ¬ ValidDecisionCertificate quarantine accepted conflicting
      digestPayload certificate := by
  intro valid
  exact wrong valid.2.2.2.2.2.2.2.2

theorem revoked_certificate_cannot_authorize_new_decision
    (revocationDigest : Nat) :
    ¬ AdmittedForNewDecision (.revoked revocationDigest) := by
  intro admitted
  cases admitted

theorem revocation_is_append_only_lifecycle_step
    (certificateDigest revocationDigest : Nat) :
    CertificateLifecycleStep certificateDigest .active (.revoked revocationDigest) := by
  exact CertificateLifecycleStep.revoke certificateDigest revocationDigest

theorem revoked_status_cannot_transition_back_to_active
    (certificateDigest revocationDigest : Nat) :
    ¬ CertificateLifecycleStep certificateDigest (.revoked revocationDigest) .active := by
  intro transition
  cases transition

theorem valid_appeal_binds_prior_certificate
    {prior : DecisionCertificate}
    {appeal : AppealCertificate}
    (valid : AppealSupersedes prior appeal) :
    appeal.priorCertificateDigest = prior.certificateDigest := by
  exact valid.1

theorem valid_appeal_advances_certificate_generation
    {prior : DecisionCertificate}
    {appeal : AppealCertificate}
    (valid : AppealSupersedes prior appeal) :
    appeal.successor.payload.generation = prior.payload.generation + 1 := by
  exact valid.2.1

theorem appeal_cannot_restore_quarantined_generation
    {prior : DecisionCertificate}
    {appeal : AppealCertificate}
    {quarantinedGeneration : Nat}
    (priorAdvanced : prior.payload.generation = quarantinedGeneration + 1)
    (valid : AppealSupersedes prior appeal) :
    appeal.successor.payload.generation ≠ quarantinedGeneration := by
  intro equal
  have successor : appeal.successor.payload.generation =
      (quarantinedGeneration + 1) + 1 := by
    exact valid.2.1.trans (congrArg (fun n => n + 1) priorAdvanced)
  have greater : quarantinedGeneration < appeal.successor.payload.generation := by
    rw [successor]
    exact Nat.lt_trans
      (Nat.lt_succ_self quarantinedGeneration)
      (Nat.lt_succ_self (quarantinedGeneration + 1))
  exact (Nat.ne_of_gt greater) equal

end ASPProof.AgentSessionHostRegistryAdjudicationDecisionCertificate
