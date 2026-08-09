namespace ASPProof.ActivationLifecycleAudit

inductive ActivationPhase where
  | inactive
  | activating
  | ready
  | degraded
  | repairing
  | evolving
  | retired
  deriving DecidableEq, Repr

inductive DispatchEvidence where
  | none
  | claimed
  | terminal
  | missingAfterDispatch
  deriving DecidableEq, Repr

structure ActivationState where
  phase : ActivationPhase
  activationReadable : Bool
  registryBound : Bool
  hostReachable : Bool
  transportBound : Bool
  repairToolAvailable : Bool
  emergencyRepairCapability : Bool
  hostAuditComplete : Bool
  subagentStartObserved : Bool
  dispatchAcknowledged : Bool
  dispatchEvidence : DispatchEvidence
  declaredGeneration : Nat
  observedGeneration : Nat
  declaredSchemaVersion : Nat
  runtimeSchemaVersion : Nat
  declaredBinaryDigest : Nat
  observedBinaryDigest : Nat

def DispatchRecoverable : DispatchEvidence → Prop
  | .none => True
  | .claimed => True
  | .terminal => True
  | .missingAfterDispatch => False

def RuntimeIdentityMatches (state : ActivationState) : Prop :=
  state.declaredGeneration = state.observedGeneration ∧
  state.declaredSchemaVersion = state.runtimeSchemaVersion ∧
  state.declaredBinaryDigest = state.observedBinaryDigest

def OperationalReady (state : ActivationState) : Prop :=
  state.phase = .ready ∧
  state.activationReadable = true ∧
  state.registryBound = true ∧
  state.hostReachable = true ∧
  state.transportBound = true ∧
  RuntimeIdentityMatches state ∧
  DispatchRecoverable state.dispatchEvidence

def CanEnterRepair (state : ActivationState) : Prop :=
  state.repairToolAvailable = true ∧
  (state.activationReadable = true ∨
    state.emergencyRepairCapability = true)

def SafeReplacement (state : ActivationState) : Prop :=
  state.hostAuditComplete = true ∧
  state.hostReachable = false ∧
  state.transportBound = false

def EventOnlyRebind (state : ActivationState) : Prop :=
  state.hostReachable = true ∧
  state.subagentStartObserved = true

def AcknowledgementRebind (state : ActivationState) : Prop :=
  state.hostReachable = true ∧
  state.dispatchAcknowledged = true

structure EvolutionPlan where
  expectedGeneration : Nat
  targetGeneration : Nat
  targetSchemaVersion : Nat
  targetBinaryDigest : Nat
  compatibilityWitness : Bool
  rollbackCheckpoint : Bool
  preservesCanonicalIdentity : Bool

def CanApplyEvolution
    (state : ActivationState)
    (plan : EvolutionPlan) : Prop :=
  plan.expectedGeneration = state.observedGeneration ∧
  state.observedGeneration < plan.targetGeneration ∧
  plan.compatibilityWitness = true ∧
  plan.rollbackCheckpoint = true ∧
  plan.preservesCanonicalIdentity = true

def applyEvolution
    (state : ActivationState)
    (plan : EvolutionPlan)
    (_ : CanApplyEvolution state plan) :
    ActivationState :=
  { state with
    phase := .evolving
    declaredGeneration := plan.targetGeneration
    observedGeneration := plan.targetGeneration
    declaredSchemaVersion := plan.targetSchemaVersion
    runtimeSchemaVersion := plan.targetSchemaVersion
    declaredBinaryDigest := plan.targetBinaryDigest
    observedBinaryDigest := plan.targetBinaryDigest
  }

theorem operational_ready_implies_live_bound_matching_runtime
    (state : ActivationState)
    (ready : OperationalReady state) :
    state.activationReadable = true ∧
    state.registryBound = true ∧
    state.hostReachable = true ∧
    state.transportBound = true ∧
    RuntimeIdentityMatches state ∧
    DispatchRecoverable state.dispatchEvidence := by
  exact
    ⟨ready.2.1, ready.2.2.1, ready.2.2.2.1, ready.2.2.2.2.1,
      ready.2.2.2.2.2.1, ready.2.2.2.2.2.2⟩

def cachedReadyWithoutHost : ActivationState where
  phase := .ready
  activationReadable := true
  registryBound := true
  hostReachable := false
  transportBound := true
  repairToolAvailable := true
  emergencyRepairCapability := false
  hostAuditComplete := false
  subagentStartObserved := false
  dispatchAcknowledged := false
  dispatchEvidence := .none
  declaredGeneration := 4
  observedGeneration := 4
  declaredSchemaVersion := 1
  runtimeSchemaVersion := 1
  declaredBinaryDigest := 10
  observedBinaryDigest := 10

theorem cached_ready_without_host_is_not_operational :
    ¬ OperationalReady cachedReadyWithoutHost := by
  intro ready
  exact Bool.noConfusion ready.2.2.2.1

def unreadableActivationWithoutEmergency : ActivationState :=
  { cachedReadyWithoutHost with
    phase := .degraded
    activationReadable := false
    repairToolAvailable := true
    emergencyRepairCapability := false
  }

theorem missing_activation_without_emergency_capability_deadlocks_repair :
    ¬ CanEnterRepair unreadableActivationWithoutEmergency := by
  intro repairable
  rcases repairable.2 with readable | emergency
  · exact Bool.noConfusion readable
  · exact Bool.noConfusion emergency

def hiddenReachableChild : ActivationState :=
  { cachedReadyWithoutHost with
    phase := .degraded
    hostReachable := true
    transportBound := false
    hostAuditComplete := false
  }

theorem hidden_reachable_child_is_not_safe_to_replace :
    ¬ SafeReplacement hiddenReachableChild := by
  intro replaceable
  exact Bool.noConfusion replaceable.2.1

def idleResumableChild : ActivationState :=
  { hiddenReachableChild with
    dispatchAcknowledged := true
    subagentStartObserved := false
  }

theorem idle_resume_can_rebind_by_ack_without_subagent_start :
    AcknowledgementRebind idleResumableChild ∧
    ¬ EventOnlyRebind idleResumableChild := by
  constructor
  · exact ⟨rfl, rfl⟩
  · intro eventOnly
    exact Bool.noConfusion eventOnly.2

def readyWithMissingDispatchReceipt : ActivationState :=
  { cachedReadyWithoutHost with
    hostReachable := true
    dispatchEvidence := .missingAfterDispatch
  }

theorem missing_dispatch_receipt_blocks_operational_ready :
    ¬ OperationalReady readyWithMissingDispatchReceipt := by
  intro ready
  exact ready.2.2.2.2.2.2

def readyWithBinaryDrift : ActivationState :=
  { cachedReadyWithoutHost with
    hostReachable := true
    declaredBinaryDigest := 10
    observedBinaryDigest := 11
  }

theorem binary_drift_blocks_operational_ready :
    ¬ OperationalReady readyWithBinaryDrift := by
  intro ready
  simp [OperationalReady, RuntimeIdentityMatches, readyWithBinaryDrift,
    cachedReadyWithoutHost] at ready

structure ActivationObservation where
  state : ActivationState
  observedAtRevision : Nat
  currentAuthorityRevision : Nat

def TrustedReady (observation : ActivationObservation) : Prop :=
  OperationalReady observation.state ∧
  observation.observedAtRevision =
    observation.currentAuthorityRevision

def fullyBoundReadyState : ActivationState :=
  { cachedReadyWithoutHost with
    hostReachable := true
    transportBound := true
  }

def staleReadyObservation : ActivationObservation where
  state := fullyBoundReadyState
  observedAtRevision := 4
  currentAuthorityRevision := 5

theorem operational_ready_does_not_imply_fresh_trusted_ready :
    OperationalReady staleReadyObservation.state ∧
    ¬ TrustedReady staleReadyObservation := by
  constructor
  · exact
      ⟨rfl, rfl, rfl, rfl, rfl, ⟨rfl, rfl, rfl⟩, trivial⟩
  · intro trusted
    simp [TrustedReady, staleReadyObservation] at trusted

theorem applicable_evolution_advances_generation_and_matches_runtime
    (state : ActivationState)
    (plan : EvolutionPlan)
    (applicable : CanApplyEvolution state plan) :
    state.observedGeneration <
        (applyEvolution state plan applicable).observedGeneration ∧
    RuntimeIdentityMatches (applyEvolution state plan applicable) := by
  constructor
  · exact applicable.2.1
  · exact ⟨rfl, rfl, rfl⟩

theorem generation_regression_cannot_be_applied
    (state : ActivationState)
    (plan : EvolutionPlan)
    (regresses :
      plan.targetGeneration ≤ state.observedGeneration) :
    ¬ CanApplyEvolution state plan := by
  intro applicable
  exact (Nat.not_lt_of_ge regresses) applicable.2.1

theorem evolution_without_rollback_cannot_be_applied
    (state : ActivationState)
    (plan : EvolutionPlan)
    (noRollback : plan.rollbackCheckpoint = false) :
    ¬ CanApplyEvolution state plan := by
  intro applicable
  have rollback := applicable.2.2.2.1
  rw [noRollback] at rollback
  exact Bool.noConfusion rollback

theorem evolution_without_compatibility_witness_cannot_be_applied
    (state : ActivationState)
    (plan : EvolutionPlan)
    (incompatible : plan.compatibilityWitness = false) :
    ¬ CanApplyEvolution state plan := by
  intro applicable
  have compatible := applicable.2.2.1
  rw [incompatible] at compatible
  exact Bool.noConfusion compatible

/-! ## Atomic activation-generation publication -/

structure ActivationGenerationSnapshot where
  generation : Nat
  activationDigest : Nat
  receiptActivationDigest : Nat
  providerGeneration : Nat
  configGeneration : Nat
  complete : Bool
  deriving DecidableEq, Repr

def GenerationConsistent (snapshot : ActivationGenerationSnapshot) : Prop :=
  snapshot.complete = true ∧
  snapshot.activationDigest = snapshot.receiptActivationDigest

def publishGeneration
    (current candidate : ActivationGenerationSnapshot) : ActivationGenerationSnapshot :=
  if candidate.complete &&
      decide (candidate.activationDigest = candidate.receiptActivationDigest)
  then candidate
  else current

theorem incomplete_candidate_cannot_replace_published_generation
    (current candidate : ActivationGenerationSnapshot)
    (incomplete : candidate.complete = false) :
    publishGeneration current candidate = current := by
  simp [publishGeneration, incomplete]

theorem digest_mismatched_candidate_cannot_replace_published_generation
    (current candidate : ActivationGenerationSnapshot)
    (mismatch : candidate.activationDigest ≠ candidate.receiptActivationDigest) :
    publishGeneration current candidate = current := by
  simp [publishGeneration, mismatch]

theorem consistent_candidate_is_the_only_new_visible_generation
    (current candidate : ActivationGenerationSnapshot)
    (complete : candidate.complete = true)
    (consistent : candidate.activationDigest = candidate.receiptActivationDigest) :
    publishGeneration current candidate = candidate := by
  simp [publishGeneration, complete, consistent]

def mixedTwoFileGeneration : ActivationGenerationSnapshot where
  generation := 2
  activationDigest := 22
  receiptActivationDigest := 11
  providerGeneration := 2
  configGeneration := 2
  complete := true

/-- Writing activation and receipt as separate visible files admits a mixed
generation that the atomic publication protocol must reject. -/
theorem two_file_activation_then_receipt_publish_is_not_consistent :
    ¬ GenerationConsistent mixedTwoFileGeneration := by
  simp [GenerationConsistent, mixedTwoFileGeneration]

end ASPProof.ActivationLifecycleAudit
