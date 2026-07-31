import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

structure AdmissionState where
  admittedCount : Nat
  roundCapacity : Nat

structure RegistrationState where
  processedCount : Nat
  processingCapacity : Nat

def AdmissionAuthorized (state : AdmissionState) : Prop :=
  state.admittedCount < state.roundCapacity

def admitOne (state : AdmissionState) : AdmissionState := {
  admittedCount := state.admittedCount + 1
  roundCapacity := state.roundCapacity
}

def PrincipalQuotaAuthorized
    (principalAdmittedCount principalQuota : Nat) : Prop :=
  principalAdmittedCount < principalQuota

def RegistrationProcessingAuthorized
    (state : RegistrationState) : Prop :=
  state.processedCount < state.processingCapacity

def processOneRegistration
    (state : RegistrationState) : RegistrationState := {
  processedCount := state.processedCount + 1
  processingCapacity := state.processingCapacity
}

def ComparisonCount : Nat → Nat
  | 0 => 0
  | Nat.succ remainingCandidates =>
      remainingCandidates

def RoundProofCost
    (admittedCount fixedCost perComparisonCost : Nat) : Nat :=
  fixedCost +
    ComparisonCount admittedCount * perComparisonCost

def RecoveryProofCost
    (attempts admittedCount fixedCost
      perComparisonCost : Nat) : Nat :=
  attempts *
    RoundProofCost
      admittedCount fixedCost perComparisonCost

def RoundSearchCost
    (processedCount admittedCount fixedCost
      perRegistrationCost perComparisonCost : Nat) : Nat :=
  fixedCost +
    processedCount * perRegistrationCost +
    ComparisonCount admittedCount * perComparisonCost

def RecoverySearchCost
    (attempts processedCount admittedCount fixedCost
      perRegistrationCost perComparisonCost : Nat) : Nat :=
  attempts *
    RoundSearchCost
      processedCount
      admittedCount
      fixedCost
      perRegistrationCost
      perComparisonCost

structure AdmissionRoundKey
    (TransitionId CandidateSetDigest PolicyId CostModelId : Type) where
  transitionId : TransitionId
  roundNumber : Nat
  roundCapacity : Nat
  principalQuota : Nat
  admittedCandidateSetDigest : CandidateSetDigest
  admissionPolicyVersion : PolicyId
  costModelVersion : CostModelId

def AdmissionRoundCompatible
    {TransitionId CandidateSetDigest PolicyId CostModelId : Type}
    (left right :
      AdmissionRoundKey
        TransitionId CandidateSetDigest PolicyId CostModelId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.roundNumber = right.roundNumber ∧
  left.roundCapacity = right.roundCapacity ∧
  left.principalQuota = right.principalQuota ∧
  left.admittedCandidateSetDigest =
      right.admittedCandidateSetDigest ∧
  left.admissionPolicyVersion =
      right.admissionPolicyVersion ∧
  left.costModelVersion = right.costModelVersion

theorem authorized_admission_stays_within_capacity
    (state : AdmissionState)
    (authorized : AdmissionAuthorized state) :
    (admitOne state).admittedCount ≤
      state.roundCapacity :=
  Nat.succ_le_of_lt authorized

theorem full_round_blocks_admission
    (state : AdmissionState)
    (full :
      state.roundCapacity ≤ state.admittedCount) :
    ¬ AdmissionAuthorized state := by
  intro authorized
  exact (Nat.not_lt_of_ge full) authorized

theorem authorized_principal_admission_stays_within_quota
    (principalAdmittedCount principalQuota : Nat)
    (authorized :
      PrincipalQuotaAuthorized
        principalAdmittedCount principalQuota) :
    principalAdmittedCount + 1 ≤ principalQuota :=
  Nat.succ_le_of_lt authorized

theorem authorized_registration_processing_stays_within_capacity
    (state : RegistrationState)
    (authorized : RegistrationProcessingAuthorized state) :
    (processOneRegistration state).processedCount ≤
      state.processingCapacity :=
  Nat.succ_le_of_lt authorized

theorem full_registration_processing_budget_blocks_work
    (state : RegistrationState)
    (full :
      state.processingCapacity ≤ state.processedCount) :
    ¬ RegistrationProcessingAuthorized state := by
  intro authorized
  exact (Nat.not_lt_of_ge full) authorized

theorem per_principal_quota_alone_does_not_bound_global_count :
    ∀ proposedGlobalCapacity : Nat,
      ∃ principalCount principalQuota : Nat,
        proposedGlobalCapacity < principalCount ∧
        principalQuota = 1 := by
  intro proposedGlobalCapacity
  exact ⟨
    proposedGlobalCapacity + 1,
    1,
    by
      rw [Nat.add_one]
      exact Nat.lt_succ_self proposedGlobalCapacity,
    rfl
  ⟩

theorem comparison_count_is_bounded_by_candidate_count
    (candidateCount : Nat) :
    ComparisonCount candidateCount ≤ candidateCount := by
  cases candidateCount with
  | zero =>
      exact Nat.le_refl 0
  | succ remainingCandidates =>
      exact Nat.le_succ remainingCandidates

theorem comparison_count_is_bounded_by_round_capacity
    (admittedCount roundCapacity : Nat)
    (admissionBound :
      admittedCount ≤ roundCapacity) :
    ComparisonCount admittedCount ≤ roundCapacity :=
  Nat.le_trans
    (comparison_count_is_bounded_by_candidate_count
      admittedCount)
    admissionBound

theorem round_proof_cost_is_capacity_bounded
    (admittedCount roundCapacity fixedCost
      perComparisonCost : Nat)
    (admissionBound :
      admittedCount ≤ roundCapacity) :
    RoundProofCost
        admittedCount fixedCost perComparisonCost ≤
      fixedCost +
        roundCapacity * perComparisonCost := by
  unfold RoundProofCost
  exact Nat.add_le_add_left
    (Nat.mul_le_mul_right
      perComparisonCost
      (comparison_count_is_bounded_by_round_capacity
        admittedCount roundCapacity admissionBound))
    fixedCost

theorem recovery_proof_cost_is_attempt_and_capacity_bounded
    (attempts maximumAttempts admittedCount roundCapacity
      fixedCost perComparisonCost : Nat)
    (attemptBound : attempts ≤ maximumAttempts)
    (admissionBound : admittedCount ≤ roundCapacity) :
    RecoveryProofCost
        attempts admittedCount fixedCost perComparisonCost ≤
      maximumAttempts *
        (fixedCost +
          roundCapacity * perComparisonCost) := by
  unfold RecoveryProofCost
  exact Nat.mul_le_mul
    attemptBound
    (round_proof_cost_is_capacity_bounded
      admittedCount
      roundCapacity
      fixedCost
      perComparisonCost
      admissionBound)

theorem round_search_cost_is_dual_capacity_bounded
    (processedCount processingCapacity
      admittedCount admissionCapacity
      fixedCost perRegistrationCost
      perComparisonCost : Nat)
    (processingBound :
      processedCount ≤ processingCapacity)
    (admissionBound :
      admittedCount ≤ admissionCapacity) :
    RoundSearchCost
        processedCount
        admittedCount
        fixedCost
        perRegistrationCost
        perComparisonCost ≤
      fixedCost +
        processingCapacity * perRegistrationCost +
        admissionCapacity * perComparisonCost := by
  unfold RoundSearchCost
  exact Nat.add_le_add
    (Nat.add_le_add_left
      (Nat.mul_le_mul_right
        perRegistrationCost processingBound)
      fixedCost)
    (Nat.mul_le_mul_right
      perComparisonCost
      (comparison_count_is_bounded_by_round_capacity
        admittedCount admissionCapacity admissionBound))

theorem recovery_search_cost_is_globally_bounded
    (attempts maximumAttempts
      processedCount processingCapacity
      admittedCount admissionCapacity
      fixedCost perRegistrationCost
      perComparisonCost : Nat)
    (attemptBound : attempts ≤ maximumAttempts)
    (processingBound :
      processedCount ≤ processingCapacity)
    (admissionBound :
      admittedCount ≤ admissionCapacity) :
    RecoverySearchCost
        attempts
        processedCount
        admittedCount
        fixedCost
        perRegistrationCost
        perComparisonCost ≤
      maximumAttempts *
        (fixedCost +
          processingCapacity * perRegistrationCost +
          admissionCapacity * perComparisonCost) := by
  unfold RecoverySearchCost
  exact Nat.mul_le_mul
    attemptBound
    (round_search_cost_is_dual_capacity_bounded
      processedCount
      processingCapacity
      admittedCount
      admissionCapacity
      fixedCost
      perRegistrationCost
      perComparisonCost
      processingBound
      admissionBound)

theorem admission_round_compatibility_binds_capacity
    {TransitionId CandidateSetDigest PolicyId CostModelId : Type}
    (left right :
      AdmissionRoundKey
        TransitionId CandidateSetDigest PolicyId CostModelId)
    (compatible : AdmissionRoundCompatible left right) :
    left.roundCapacity = right.roundCapacity :=
  compatible.2.2.1

theorem changed_capacity_invalidates_admission_round
    {TransitionId CandidateSetDigest PolicyId CostModelId : Type}
    (left right :
      AdmissionRoundKey
        TransitionId CandidateSetDigest PolicyId CostModelId)
    (capacityChanged :
      left.roundCapacity ≠ right.roundCapacity) :
    ¬ AdmissionRoundCompatible left right := by
  intro compatible
  exact
    capacityChanged
      (admission_round_compatibility_binds_capacity
        left right compatible)

theorem same_admitted_count_does_not_imply_round_compatibility
    {TransitionId CandidateSetDigest PolicyId CostModelId : Type}
    (left right :
      AdmissionRoundKey
        TransitionId CandidateSetDigest PolicyId CostModelId)
    (admittedCount : Nat)
    (capacityChanged :
      left.roundCapacity ≠ right.roundCapacity) :
    admittedCount = admittedCount ∧
      ¬ AdmissionRoundCompatible left right :=
  ⟨rfl,
    changed_capacity_invalidates_admission_round
      left right capacityChanged⟩

theorem changed_candidate_set_invalidates_admission_round
    {TransitionId CandidateSetDigest PolicyId CostModelId : Type}
    (left right :
      AdmissionRoundKey
        TransitionId CandidateSetDigest PolicyId CostModelId)
    (candidateSetChanged :
      left.admittedCandidateSetDigest ≠
        right.admittedCandidateSetDigest) :
    ¬ AdmissionRoundCompatible left right := by
  intro compatible
  exact candidateSetChanged compatible.2.2.2.2.1

end ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost
