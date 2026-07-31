import ASPProof.AgentSessionDispatchTransaction

namespace ASPProof.AgentSessionDispatchRecovery

open ASPProof.AgentSessionDispatchTransaction

structure NegativeReceipt (intent : DispatchIntent) where
  reasonCode : Nat
  deriving DecidableEq, Repr

inductive RecoveryPhase (intent : DispatchIntent) where
  | attempting (remainingRetries : Nat)
  | quarantined
  | delivered (receipt : HostReceipt intent.key intent.payloadDigest)
  | rejected (receipt : NegativeReceipt intent)
  deriving DecidableEq, Repr

inductive AutomaticStep {intent : DispatchIntent} :
    RecoveryPhase intent → RecoveryPhase intent → Prop where
  | retry (remaining : Nat) :
      AutomaticStep (.attempting (remaining + 1)) (.attempting remaining)
  | exhaust :
      AutomaticStep (.attempting 0) .quarantined

inductive EvidenceStep {intent : DispatchIntent} :
    RecoveryPhase intent → RecoveryPhase intent → Prop where
  | acceptAttempting
      (remaining : Nat)
      (receipt : HostReceipt intent.key intent.payloadDigest) :
      EvidenceStep (.attempting remaining) (.delivered receipt)
  | rejectAttempting
      (remaining : Nat)
      (receipt : NegativeReceipt intent) :
      EvidenceStep (.attempting remaining) (.rejected receipt)
  | acceptQuarantined
      (receipt : HostReceipt intent.key intent.payloadDigest) :
      EvidenceStep .quarantined (.delivered receipt)
  | rejectQuarantined
      (receipt : NegativeReceipt intent) :
      EvidenceStep .quarantined (.rejected receipt)

inductive Terminal {intent : DispatchIntent} : RecoveryPhase intent → Prop where
  | delivered (receipt : HostReceipt intent.key intent.payloadDigest) :
      Terminal (.delivered receipt)
  | rejected (receipt : NegativeReceipt intent) : Terminal (.rejected receipt)

inductive MayCloseReservation {intent : DispatchIntent} :
    RecoveryPhase intent → Prop where
  | delivered (receipt : HostReceipt intent.key intent.payloadDigest) :
      MayCloseReservation (.delivered receipt)
  | rejected (receipt : NegativeReceipt intent) :
      MayCloseReservation (.rejected receipt)

inductive MaySupersedeTarget {intent : DispatchIntent} :
    RecoveryPhase intent → Prop where
  | rejected (receipt : NegativeReceipt intent) :
      MaySupersedeTarget (.rejected receipt)

inductive RecoveryPrincipal where
  | agentSessionSupervisor
  | runtimeServer
  | residentSubagent
  | hostRuntime
  deriving DecidableEq, Repr

inductive OwnsRecovery : RecoveryPrincipal → Prop where
  | sessionSupervisor :
      OwnsRecovery .agentSessionSupervisor

inductive OwnsOutcomeAttestation : RecoveryPrincipal → Prop where
  | hostRuntime :
      OwnsOutcomeAttestation .hostRuntime

inductive HealthStage where
  | manifestValid
  | binaryResolved
  | catalogReady
  | invocationAccepted
  | responseDecoded
  | replayVerified
  deriving DecidableEq, Repr

inductive InvocationEvidence : HealthStage → Prop where
  | accepted : InvocationEvidence .invocationAccepted
  | decoded : InvocationEvidence .responseDecoded
  | replayed : InvocationEvidence .replayVerified

inductive CompletedResponseEvidence : HealthStage → Prop where
  | decoded : CompletedResponseEvidence .responseDecoded
  | replayed : CompletedResponseEvidence .replayVerified

inductive ReplayEvidence : HealthStage → Prop where
  | verified : ReplayEvidence .replayVerified

inductive FairSchedule : Prop where
  | available : FairSchedule

inductive CasAvailable : Prop where
  | available : CasAvailable

inductive StableHostAuthority (intent : DispatchIntent) : Prop where
  | attested : StableHostAuthority intent

structure LivenessConditions (intent : DispatchIntent) : Prop where
  fairSchedule : FairSchedule
  casAvailable : CasAvailable
  stableHostAuthority : StableHostAuthority intent

inductive AuthoritativeOutcome (intent : DispatchIntent) where
  | accepted (receipt : HostReceipt intent.key intent.payloadDigest)
  | rejected (receipt : NegativeReceipt intent)

structure ConditionalClosure
    {intent : DispatchIntent}
    (initial final : RecoveryPhase intent) : Prop where
  conditions : LivenessConditions intent
  evidence : EvidenceStep initial final
  terminal : Terminal final

def retryMeasure {intent : DispatchIntent} :
    RecoveryPhase intent → Nat
  | .attempting remaining => remaining + 1
  | .quarantined => 0
  | .delivered _ => 0
  | .rejected _ => 0

structure ControlCost where
  llmRounds : Nat
  recoveryMessages : Nat
  deriving DecidableEq, Repr

def backgroundRecoveryCost : ControlCost :=
  { llmRounds := 0, recoveryMessages := 2 }

def promptLoopRecoveryCost : ControlCost :=
  { llmRounds := 3, recoveryMessages := 5 }

theorem automatic_retry_decreases_measure
    {intent : DispatchIntent}
    {before after : RecoveryPhase intent}
    (step : AutomaticStep before after) :
    retryMeasure after < retryMeasure before := by
  cases step with
  | retry remaining =>
      exact Nat.lt_succ_self (remaining + 1)
  | exhaust =>
      exact Nat.zero_lt_succ 0

theorem zero_budget_enters_quarantine
    (intent : DispatchIntent) :
    AutomaticStep
      (RecoveryPhase.attempting 0 : RecoveryPhase intent)
      RecoveryPhase.quarantined := by
  exact AutomaticStep.exhaust

theorem zero_budget_cannot_retry
    (intent : DispatchIntent) :
    ¬ ∃ remaining,
      AutomaticStep
        (RecoveryPhase.attempting 0 : RecoveryPhase intent)
        (RecoveryPhase.attempting remaining) := by
  intro witness
  rcases witness with ⟨remaining, step⟩
  cases step

theorem quarantined_has_no_automatic_step
    (intent : DispatchIntent) :
    ¬ ∃ next,
      AutomaticStep
        (RecoveryPhase.quarantined : RecoveryPhase intent)
        next := by
  intro witness
  rcases witness with ⟨next, step⟩
  cases step

theorem quarantined_is_not_terminal
    (intent : DispatchIntent) :
    ¬ Terminal (RecoveryPhase.quarantined : RecoveryPhase intent) := by
  intro terminal
  cases terminal

theorem quarantine_cannot_close_reservation
    (intent : DispatchIntent) :
    ¬ MayCloseReservation
      (RecoveryPhase.quarantined : RecoveryPhase intent) := by
  intro closable
  cases closable

theorem quarantine_cannot_retarget
    (intent : DispatchIntent) :
    ¬ MaySupersedeTarget
      (RecoveryPhase.quarantined : RecoveryPhase intent) := by
  intro retarget
  cases retarget

theorem attempting_cannot_close_reservation
    (intent : DispatchIntent)
    (remaining : Nat) :
    ¬ MayCloseReservation
      (RecoveryPhase.attempting remaining : RecoveryPhase intent) := by
  intro closable
  cases closable

theorem timeout_is_not_negative_evidence
    (intent : DispatchIntent)
    (remaining : Nat) :
    ¬ EvidenceStep
      (RecoveryPhase.attempting remaining : RecoveryPhase intent)
      RecoveryPhase.quarantined := by
  intro evidence
  cases evidence

theorem accepted_evidence_resolves_quarantine
    (intent : DispatchIntent)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    EvidenceStep
      (RecoveryPhase.quarantined : RecoveryPhase intent)
      (RecoveryPhase.delivered receipt) := by
  exact EvidenceStep.acceptQuarantined receipt

theorem rejected_evidence_resolves_quarantine
    (intent : DispatchIntent)
    (receipt : NegativeReceipt intent) :
    EvidenceStep
      (RecoveryPhase.quarantined : RecoveryPhase intent)
      (RecoveryPhase.rejected receipt) := by
  exact EvidenceStep.rejectQuarantined receipt

theorem runtime_server_does_not_own_dispatch_recovery :
    ¬ OwnsRecovery RecoveryPrincipal.runtimeServer := by
  intro ownership
  cases ownership

theorem resident_subagent_does_not_own_dispatch_recovery :
    ¬ OwnsRecovery RecoveryPrincipal.residentSubagent := by
  intro ownership
  cases ownership

theorem host_runtime_owns_outcome_attestation :
    OwnsOutcomeAttestation RecoveryPrincipal.hostRuntime := by
  exact OwnsOutcomeAttestation.hostRuntime

theorem runtime_server_does_not_own_outcome_attestation :
    ¬ OwnsOutcomeAttestation RecoveryPrincipal.runtimeServer := by
  intro ownership
  cases ownership

theorem binary_resolution_is_not_invocation_evidence :
    ¬ InvocationEvidence HealthStage.binaryResolved := by
  intro evidence
  cases evidence

theorem catalog_readiness_is_not_invocation_evidence :
    ¬ InvocationEvidence HealthStage.catalogReady := by
  intro evidence
  cases evidence

theorem invocation_acceptance_is_not_completed_response :
    ¬ CompletedResponseEvidence HealthStage.invocationAccepted := by
  intro evidence
  cases evidence

theorem decoded_response_is_not_replay_evidence :
    ¬ ReplayEvidence HealthStage.responseDecoded := by
  intro evidence
  cases evidence

theorem attempting_conditional_liveness
    (intent : DispatchIntent)
    (remaining : Nat)
    (conditions : LivenessConditions intent)
    (outcome : AuthoritativeOutcome intent) :
    ∃ final, ConditionalClosure
      (RecoveryPhase.attempting remaining : RecoveryPhase intent)
      final := by
  cases outcome with
  | accepted receipt =>
      exact ⟨.delivered receipt, {
        conditions := conditions
        evidence := EvidenceStep.acceptAttempting remaining receipt
        terminal := Terminal.delivered receipt
      }⟩
  | rejected receipt =>
      exact ⟨.rejected receipt, {
        conditions := conditions
        evidence := EvidenceStep.rejectAttempting remaining receipt
        terminal := Terminal.rejected receipt
      }⟩

theorem quarantine_conditional_liveness
    (intent : DispatchIntent)
    (conditions : LivenessConditions intent)
    (outcome : AuthoritativeOutcome intent) :
    ∃ final, ConditionalClosure
      (RecoveryPhase.quarantined : RecoveryPhase intent)
      final := by
  cases outcome with
  | accepted receipt =>
      exact ⟨.delivered receipt, {
        conditions := conditions
        evidence := EvidenceStep.acceptQuarantined receipt
        terminal := Terminal.delivered receipt
      }⟩
  | rejected receipt =>
      exact ⟨.rejected receipt, {
        conditions := conditions
        evidence := EvidenceStep.rejectQuarantined receipt
        terminal := Terminal.rejected receipt
      }⟩

theorem background_recovery_reduces_llm_rounds :
    backgroundRecoveryCost.llmRounds <
      promptLoopRecoveryCost.llmRounds := by
  decide

end ASPProof.AgentSessionDispatchRecovery
