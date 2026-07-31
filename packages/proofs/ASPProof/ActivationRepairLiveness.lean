import ASPProof.ActivationRepairTransaction

namespace ASPProof.ActivationRepairLiveness

/-- Observable milestones of one successful repair attempt. -/
inductive RepairStage where
  | needsRepair
  | capabilityIssued
  | candidateCommitted
  | publicationEnqueued
  | candidatePublished
  | hostObserved
  | dispatchReconciled
  | trustedReady
  | rollbackClosed
  | retired
  deriving Repr, DecidableEq

/-- Environmental progress assumptions. These are deliberately data, not
hidden axioms of the repair safety model. -/
structure ProgressGuarantees where
  authorityAvailable : Bool
  capabilityStableUntilCommit : Bool
  winningWriterScheduled : Bool
  durableOutboxAvailable : Bool
  publicationDeliveryFair : Bool
  hostObservationLeaseAvailable : Bool
  dispatchReconciliationFair : Bool
  freshReadinessEvidenceAvailable : Bool
  rollbackClockAdvances : Bool
  retirementStoreAvailable : Bool
  deriving Repr, DecidableEq

def AllGuarantees (guarantees : ProgressGuarantees) : Prop :=
  guarantees.authorityAvailable = true ∧
  guarantees.capabilityStableUntilCommit = true ∧
  guarantees.winningWriterScheduled = true ∧
  guarantees.durableOutboxAvailable = true ∧
  guarantees.publicationDeliveryFair = true ∧
  guarantees.hostObservationLeaseAvailable = true ∧
  guarantees.dispatchReconciliationFair = true ∧
  guarantees.freshReadinessEvidenceAvailable = true ∧
  guarantees.rollbackClockAdvances = true ∧
  guarantees.retirementStoreAvailable = true

/-- One externally justified progress transition. -/
inductive Step (guarantees : ProgressGuarantees) : RepairStage → RepairStage → Prop
  | issueCapability
      (available : guarantees.authorityAvailable = true) :
      Step guarantees .needsRepair .capabilityIssued
  | commitCandidate
      (stable : guarantees.capabilityStableUntilCommit = true)
      (scheduled : guarantees.winningWriterScheduled = true) :
      Step guarantees .capabilityIssued .candidateCommitted
  | enqueuePublication
      (durable : guarantees.durableOutboxAvailable = true) :
      Step guarantees .candidateCommitted .publicationEnqueued
  | deliverPublication
      (fair : guarantees.publicationDeliveryFair = true) :
      Step guarantees .publicationEnqueued .candidatePublished
  | observeHost
      (lease : guarantees.hostObservationLeaseAvailable = true) :
      Step guarantees .candidatePublished .hostObserved
  | reconcileDispatch
      (fair : guarantees.dispatchReconciliationFair = true) :
      Step guarantees .hostObserved .dispatchReconciled
  | deriveTrustedReady
      (fresh : guarantees.freshReadinessEvidenceAvailable = true) :
      Step guarantees .dispatchReconciled .trustedReady
  | closeRollback
      (advances : guarantees.rollbackClockAdvances = true) :
      Step guarantees .trustedReady .rollbackClosed
  | retireGeneration
      (available : guarantees.retirementStoreAvailable = true) :
      Step guarantees .rollbackClosed .retired

/-- Transitive closure used for conditional liveness statements. -/
inductive Reachable (guarantees : ProgressGuarantees) :
    RepairStage → RepairStage → Prop
  | refl (stage) : Reachable guarantees stage stage
  | tail
      (step : Step guarantees before after)
      (rest : Reachable guarantees after target) :
      Reachable guarantees before target

/-- Remaining externally justified milestones. -/
def remainingSteps : RepairStage → Nat
  | .needsRepair => 9
  | .capabilityIssued => 8
  | .candidateCommitted => 7
  | .publicationEnqueued => 6
  | .candidatePublished => 5
  | .hostObserved => 4
  | .dispatchReconciled => 3
  | .trustedReady => 2
  | .rollbackClosed => 1
  | .retired => 0

def allAvailable : ProgressGuarantees :=
  { authorityAvailable := true
    capabilityStableUntilCommit := true
    winningWriterScheduled := true
    durableOutboxAvailable := true
    publicationDeliveryFair := true
    hostObservationLeaseAvailable := true
    dispatchReconciliationFair := true
    freshReadinessEvidenceAvailable := true
    rollbackClockAdvances := true
    retirementStoreAvailable := true }

def noAuthority : ProgressGuarantees :=
  { allAvailable with authorityAvailable := false }

def unstableCapability : ProgressGuarantees :=
  { allAvailable with capabilityStableUntilCommit := false }

def unscheduledWriter : ProgressGuarantees :=
  { allAvailable with winningWriterScheduled := false }

def unavailableOutbox : ProgressGuarantees :=
  { allAvailable with durableOutboxAvailable := false }

def unfairPublication : ProgressGuarantees :=
  { allAvailable with publicationDeliveryFair := false }

def unavailableHostLease : ProgressGuarantees :=
  { allAvailable with hostObservationLeaseAvailable := false }

def unreconciledDispatch : ProgressGuarantees :=
  { allAvailable with dispatchReconciliationFair := false }

def unavailableFreshEvidence : ProgressGuarantees :=
  { allAvailable with freshReadinessEvidenceAvailable := false }

def frozenRollbackClock : ProgressGuarantees :=
  { allAvailable with rollbackClockAdvances := false }

def unavailableRetirementStore : ProgressGuarantees :=
  { allAvailable with retirementStoreAvailable := false }

def CanAdvance (guarantees : ProgressGuarantees) (stage : RepairStage) : Prop :=
  ∃ next, Step guarantees stage next

theorem all_available_satisfies_guarantees :
    AllGuarantees allAvailable := by
  exact ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

theorem complete_repair_is_reachable
    (guarantees : ProgressGuarantees)
    (all : AllGuarantees guarantees) :
    Reachable guarantees .needsRepair .retired := by
  rcases all with
    ⟨authority, stable, scheduled, outbox, publication, host, dispatch,
      freshness, rollback, retirement⟩
  exact .tail (.issueCapability authority)
    (.tail (.commitCandidate stable scheduled)
      (.tail (.enqueuePublication outbox)
        (.tail (.deliverPublication publication)
          (.tail (.observeHost host)
            (.tail (.reconcileDispatch dispatch)
              (.tail (.deriveTrustedReady freshness)
                (.tail (.closeRollback rollback)
                  (.tail (.retireGeneration retirement) (.refl .retired)))))))))

theorem every_step_decreases_remaining
    (step : Step guarantees before after) :
    remainingSteps after < remainingSteps before := by
  cases step <;> decide

theorem initial_progress_requires_authority
    (advance : CanAdvance guarantees .needsRepair) :
    guarantees.authorityAvailable = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem capability_progress_requires_stability
    (advance : CanAdvance guarantees .capabilityIssued) :
    guarantees.capabilityStableUntilCommit = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem capability_progress_requires_scheduling
    (advance : CanAdvance guarantees .capabilityIssued) :
    guarantees.winningWriterScheduled = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem committed_progress_requires_durable_outbox
    (advance : CanAdvance guarantees .candidateCommitted) :
    guarantees.durableOutboxAvailable = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem enqueued_progress_requires_delivery_fairness
    (advance : CanAdvance guarantees .publicationEnqueued) :
    guarantees.publicationDeliveryFair = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem published_progress_requires_host_lease
    (advance : CanAdvance guarantees .candidatePublished) :
    guarantees.hostObservationLeaseAvailable = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem host_progress_requires_dispatch_reconciliation
    (advance : CanAdvance guarantees .hostObserved) :
    guarantees.dispatchReconciliationFair = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem reconciled_progress_requires_fresh_evidence
    (advance : CanAdvance guarantees .dispatchReconciled) :
    guarantees.freshReadinessEvidenceAvailable = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem ready_progress_requires_rollback_clock
    (advance : CanAdvance guarantees .trustedReady) :
    guarantees.rollbackClockAdvances = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem closed_rollback_requires_retirement_store
    (advance : CanAdvance guarantees .rollbackClosed) :
    guarantees.retirementStoreAvailable = true := by
  rcases advance with ⟨next, step⟩
  cases step
  assumption

theorem missing_authority_blocks_initial_progress :
    ¬ CanAdvance noAuthority .needsRepair := by
  intro advance
  have available := initial_progress_requires_authority advance
  contradiction

theorem unstable_capability_blocks_commit_progress :
    ¬ CanAdvance unstableCapability .capabilityIssued := by
  intro advance
  have stable := capability_progress_requires_stability advance
  contradiction

theorem unscheduled_writer_blocks_commit_progress :
    ¬ CanAdvance unscheduledWriter .capabilityIssued := by
  intro advance
  have scheduled := capability_progress_requires_scheduling advance
  contradiction

theorem unavailable_outbox_blocks_publication_enqueue :
    ¬ CanAdvance unavailableOutbox .candidateCommitted := by
  intro advance
  have durable := committed_progress_requires_durable_outbox advance
  contradiction

theorem unfair_delivery_blocks_publication :
    ¬ CanAdvance unfairPublication .publicationEnqueued := by
  intro advance
  have fair := enqueued_progress_requires_delivery_fairness advance
  contradiction

theorem missing_host_lease_blocks_observation :
    ¬ CanAdvance unavailableHostLease .candidatePublished := by
  intro advance
  have lease := published_progress_requires_host_lease advance
  contradiction

theorem missing_dispatch_fairness_blocks_reconciliation :
    ¬ CanAdvance unreconciledDispatch .hostObserved := by
  intro advance
  have fair := host_progress_requires_dispatch_reconciliation advance
  contradiction

theorem missing_fresh_evidence_blocks_readiness :
    ¬ CanAdvance unavailableFreshEvidence .dispatchReconciled := by
  intro advance
  have fresh := reconciled_progress_requires_fresh_evidence advance
  contradiction

theorem frozen_clock_blocks_retirement_window :
    ¬ CanAdvance frozenRollbackClock .trustedReady := by
  intro advance
  have advances := ready_progress_requires_rollback_clock advance
  contradiction

theorem missing_retirement_store_blocks_retirement :
    ¬ CanAdvance unavailableRetirementStore .rollbackClosed := by
  intro advance
  have available := closed_rollback_requires_retirement_store advance
  contradiction

end ASPProof.ActivationRepairLiveness
