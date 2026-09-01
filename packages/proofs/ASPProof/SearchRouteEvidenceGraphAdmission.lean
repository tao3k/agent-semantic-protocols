namespace SearchRouteEvidenceGraphAdmission

structure RouteCost where
  graphHops : Nat
  tokenCost : Nat
  interactionRounds : Nat
deriving DecidableEq, Repr

structure MeasurementContext where
  graphSnapshotIdentity : Nat
  modelProfileIdentity : Nat
  semanticSearchCacheIdentity : Nat
  modelPrefixCacheIdentity : Nat
  measurementProtocolIdentity : Nat
  uncertaintyProtocolIdentity : Nat
deriving DecidableEq, Repr

def NoWorse (candidate baseline : RouteCost) : Prop :=
  candidate.graphHops ≤ baseline.graphHops ∧
  candidate.tokenCost ≤ baseline.tokenCost ∧
  candidate.interactionRounds ≤ baseline.interactionRounds

def StrictlyBetter (candidate baseline : RouteCost) : Prop :=
  NoWorse candidate baseline ∧ candidate ≠ baseline

structure RouteCostInterval where
  lower : RouteCost
  upper : RouteCost
deriving DecidableEq, Repr

def WellFormedCostInterval (interval : RouteCostInterval) : Prop :=
  NoWorse interval.lower interval.upper

def RobustlyBetter
    (candidate baseline : RouteCostInterval) : Prop :=
  WellFormedCostInterval candidate ∧
  WellFormedCostInterval baseline ∧
  StrictlyBetter candidate.upper baseline.lower

def ExternalityBound
    (budget baseline candidate : RouteCost) : Prop :=
  candidate.graphHops ≤ baseline.graphHops + budget.graphHops ∧
  candidate.tokenCost ≤ baseline.tokenCost + budget.tokenCost ∧
  candidate.interactionRounds ≤
    baseline.interactionRounds + budget.interactionRounds

structure EvidenceRoute (Evidence : Type) where
  evidence : List Evidence
  reachesGoal : Prop
  evidenceValid : Prop
  measurementContext : MeasurementContext
  cost : RouteCost

def SoundRoute (route : EvidenceRoute Evidence) : Prop :=
  route.reachesGoal → route.evidenceValid

def EvidenceItemsValid
    (validEvidence : Evidence → Prop)
    (route : EvidenceRoute Evidence) : Prop :=
  ∀ evidence, evidence ∈ route.evidence → validEvidence evidence

def EvidenceComplete
    (requiredEvidence : Evidence → Prop)
    (route : EvidenceRoute Evidence) : Prop :=
  ∀ evidence, requiredEvidence evidence → evidence ∈ route.evidence

def PreservesReachability
    (baseline extended : Workload → EvidenceRoute Evidence) : Prop :=
  ∀ workload, (baseline workload).reachesGoal →
    (extended workload).reachesGoal

def PreservesEvidenceSoundness
    (extended : Workload → EvidenceRoute Evidence) : Prop :=
  ∀ workload, SoundRoute (extended workload)

def CertifiedImprovement
    (baseline extended : EvidenceRoute Evidence) : Prop :=
  baseline.measurementContext = extended.measurementContext ∧
  baseline.reachesGoal ∧
  baseline.evidenceValid ∧
  extended.reachesGoal ∧
  extended.evidenceValid ∧
  StrictlyBetter extended.cost baseline.cost

def IndependentValue
    (owner : Workload → Owner)
    (independentOwners : Owner → Owner → Prop)
    (admittedWorkload : Workload → Prop)
    (baseline extended : Workload → EvidenceRoute Evidence) : Prop :=
  ∃ left right,
    admittedWorkload left ∧
    admittedWorkload right ∧
    independentOwners (owner left) (owner right) ∧
    CertifiedImprovement (baseline left) (extended left) ∧
    CertifiedImprovement (baseline right) (extended right)

def BoundedExternality
    (budget : RouteCost)
    (baseline extended : Workload → RouteCost) : Prop :=
  ∀ workload, ExternalityBound budget (baseline workload) (extended workload)

def Isolated
    (affected : Workload → Prop)
    (baseline extended : Workload → RouteCost) : Prop :=
  ∀ workload, ¬ affected workload → extended workload = baseline workload

structure SharedComplexity where
  schemaConcepts : Nat
  routerStates : Nat
  maintenanceObligations : Nat
deriving DecidableEq, Repr

def ComplexityWithin
    (budget delta : SharedComplexity) : Prop :=
  delta.schemaConcepts ≤ budget.schemaConcepts ∧
  delta.routerStates ≤ budget.routerStates ∧
  delta.maintenanceObligations ≤ budget.maintenanceObligations

structure LifecyclePolicy where
  admittedAt : Nat
  reviewAt : Nat
  expiresAt : Nat
deriving DecidableEq, Repr

def WellFormedLifecycle (policy : LifecyclePolicy) : Prop :=
  policy.admittedAt < policy.reviewAt ∧
  policy.reviewAt ≤ policy.expiresAt

structure RenewalReceipt where
  issuedAt : Nat
  newExpiresAt : Nat
  currentValueEvidenceIdentity : Nat
  maintenanceOwnerIdentity : Nat
deriving DecidableEq, Repr

def ValidRenewal
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (receipt : RenewalReceipt) : Prop :=
  receipt.issuedAt ≤ policy.expiresAt ∧
  policy.expiresAt < receipt.newExpiresAt ∧
  authorizedRenewal receipt

def LifecycleValidAt
    (now : Nat)
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (renewal : Option RenewalReceipt) : Prop :=
  now ≤ policy.expiresAt ∨
  ∃ receipt,
    renewal = some receipt ∧
    ValidRenewal policy authorizedRenewal receipt ∧
    now ≤ receipt.newExpiresAt

def MustErase
    (now : Nat)
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (renewal : Option RenewalReceipt) : Prop :=
  ¬ LifecycleValidAt now policy authorizedRenewal renewal

structure DeletionWitness
    (BaseState ExtendedState : Type)
    (Workload Evidence : Type)
    (baseline extended : Workload → EvidenceRoute Evidence) where
  baseState : BaseState
  extendedState : ExtendedState
  embed : BaseState → ExtendedState
  remove : ExtendedState → BaseState
  eraseFeature : ExtendedState → ExtendedState
  baseRoute : BaseState → Workload → EvidenceRoute Evidence
  extendedRoute : ExtendedState → Workload → EvidenceRoute Evidence
  removeEmbed : ∀ state, remove (embed state) = state
  removeExtended : remove extendedState = baseState
  eraseIdempotent :
    eraseFeature (eraseFeature extendedState) = eraseFeature extendedState
  baselineBound :
    ∀ workload, baseRoute baseState workload = baseline workload
  extendedBound :
    ∀ workload, extendedRoute extendedState workload = extended workload
  deletionRestoresBaseline :
    ∀ workload,
      extendedRoute (eraseFeature extendedState) workload =
        baseline workload

structure CoreAdmissionCertificate
    (Owner Workload Evidence BaseState ExtendedState : Type)
    (owner : Workload → Owner)
    (independentOwners : Owner → Owner → Prop)
    (authorizedPolicy :
      (Owner → Owner → Prop) →
      (Workload → Prop) →
      (Workload → Prop) →
      RouteCost →
      SharedComplexity →
      Prop)
    (evidenceVerifier :
      Workload → MeasurementContext → List Evidence → Prop)
    (authorizedVerifier :
      (Workload → MeasurementContext → List Evidence → Prop) → Prop)
    (admittedWorkload affected measuredWorkload : Workload → Prop)
    (baseline extended : Workload → EvidenceRoute Evidence)
    (externalityBudget : RouteCost)
    (complexityBudget complexityDelta : SharedComplexity) where
  policyAuthorized :
    authorizedPolicy independentOwners affected measuredWorkload
      externalityBudget complexityBudget
  measurementCoverage :
    ∀ workload, admittedWorkload workload → measuredWorkload workload
  verifierAuthorized : authorizedVerifier evidenceVerifier
  baselineEvidenceValidityBound :
    ∀ workload,
      (baseline workload).evidenceValid ↔
        evidenceVerifier
          workload
          (baseline workload).measurementContext
          (baseline workload).evidence
  evidenceValidityBound :
    ∀ workload,
      (extended workload).evidenceValid ↔
        evidenceVerifier
          workload
          (extended workload).measurementContext
          (extended workload).evidence
  independenceIrreflexive :
    ∀ candidateOwner, ¬ independentOwners candidateOwner candidateOwner
  independenceSymmetric :
    ∀ leftOwner rightOwner,
      independentOwners leftOwner rightOwner →
      independentOwners rightOwner leftOwner
  independentValue :
    IndependentValue owner independentOwners admittedWorkload
      baseline extended
  reachability : PreservesReachability baseline extended
  baselineEvidenceSoundness : PreservesEvidenceSoundness baseline
  evidenceSoundness : PreservesEvidenceSoundness extended
  boundedExternality :
    BoundedExternality externalityBudget
      (fun workload => (baseline workload).cost)
      (fun workload => (extended workload).cost)
  isolation :
    Isolated affected
      (fun workload => (baseline workload).cost)
      (fun workload => (extended workload).cost)
  complexity : ComplexityWithin complexityBudget complexityDelta
  lifecyclePolicy : LifecyclePolicy
  lifecycleWellFormed : WellFormedLifecycle lifecyclePolicy
  deletion :
    DeletionWitness BaseState ExtendedState Workload Evidence
      baseline extended

theorem admitted_change_has_two_independent_beneficiaries
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    IndependentValue owner independentOwners admittedWorkload
      baseline extended :=
  certificate.independentValue

theorem admitted_change_beneficiary_owners_are_distinct
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    ∃ left right,
      admittedWorkload left ∧
      admittedWorkload right ∧
      owner left ≠ owner right ∧
      CertifiedImprovement (baseline left) (extended left) ∧
      CertifiedImprovement (baseline right) (extended right) := by
  rcases certificate.independentValue with
    ⟨left, right, leftAdmitted, rightAdmitted, independent,
      leftBetter, rightBetter⟩
  refine ⟨left, right, leftAdmitted, rightAdmitted, ?_, leftBetter, rightBetter⟩
  intro ownersEqual
  apply certificate.independenceIrreflexive (owner left)
  simpa [ownersEqual] using independent

theorem admitted_change_preserves_search_correctness
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    PreservesReachability baseline extended ∧
    PreservesEvidenceSoundness baseline ∧
    PreservesEvidenceSoundness extended :=
  ⟨certificate.reachability,
    certificate.baselineEvidenceSoundness,
    certificate.evidenceSoundness⟩

theorem admitted_change_has_bounded_externality
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    BoundedExternality externalityBudget
      (fun workload => (baseline workload).cost)
      (fun workload => (extended workload).cost) ∧
    ComplexityWithin complexityBudget complexityDelta :=
  ⟨certificate.boundedExternality, certificate.complexity⟩

theorem admitted_change_is_isolated_and_deletable
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    Isolated affected
      (fun workload => (baseline workload).cost)
      (fun workload => (extended workload).cost) ∧
    Nonempty (DeletionWitness BaseState ExtendedState Workload Evidence
      baseline extended) :=
  ⟨certificate.isolation, ⟨certificate.deletion⟩⟩

theorem admitted_change_uses_authorized_policy
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    authorizedPolicy independentOwners affected measuredWorkload
      externalityBudget complexityBudget :=
  certificate.policyAuthorized

theorem admitted_change_has_authorized_measurement_coverage
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    (∀ workload,
      admittedWorkload workload → measuredWorkload workload) ∧
    authorizedPolicy independentOwners affected measuredWorkload
      externalityBudget complexityBudget :=
  ⟨certificate.measurementCoverage, certificate.policyAuthorized⟩

theorem admitted_change_binds_authorized_evidence_verifier
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    authorizedVerifier evidenceVerifier ∧
    (∀ workload,
      (baseline workload).evidenceValid ↔
        evidenceVerifier
          workload
          (baseline workload).measurementContext
          (baseline workload).evidence) ∧
    ∀ workload,
      (extended workload).evidenceValid ↔
        evidenceVerifier
          workload
          (extended workload).measurementContext
          (extended workload).evidence :=
  ⟨certificate.verifierAuthorized,
    certificate.baselineEvidenceValidityBound,
    certificate.evidenceValidityBound⟩

theorem admitted_change_has_bounded_lifecycle
    (certificate :
      CoreAdmissionCertificate Owner Workload Evidence BaseState ExtendedState
        owner independentOwners authorizedPolicy
        evidenceVerifier authorizedVerifier
        admittedWorkload affected measuredWorkload baseline extended
        externalityBudget complexityBudget complexityDelta) :
    WellFormedLifecycle certificate.lifecyclePolicy :=
  certificate.lifecycleWellFormed

theorem silence_does_not_renew_expired_feature
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (expired : policy.expiresAt < now) :
    ¬ LifecycleValidAt now policy authorizedRenewal none := by
  simp [LifecycleValidAt, Nat.not_le_of_gt expired]

theorem unauthorized_receipt_does_not_renew_expired_feature
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (receipt : RenewalReceipt)
    (expired : policy.expiresAt < now)
    (unauthorized : ¬ authorizedRenewal receipt) :
    ¬ LifecycleValidAt now policy authorizedRenewal (some receipt) := by
  simp [LifecycleValidAt, ValidRenewal, Nat.not_le_of_gt expired,
    unauthorized]

theorem authorized_timely_renewal_extends_lifecycle
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (receipt : RenewalReceipt)
    (issuedInTime : receipt.issuedAt ≤ policy.expiresAt)
    (extendsUntil : policy.expiresAt < receipt.newExpiresAt)
    (authorized : authorizedRenewal receipt)
    (notExpiredAgain : now ≤ receipt.newExpiresAt) :
    LifecycleValidAt now policy authorizedRenewal (some receipt) := by
  right
  exact ⟨receipt, rfl, ⟨issuedInTime, extendsUntil, authorized⟩, notExpiredAgain⟩

def twoHopBaseline : RouteCost :=
  { graphHops := 2, tokenCost := 4, interactionRounds := 2 }

def oneHopPayloadDetour : RouteCost :=
  { graphHops := 1, tokenCost := 10, interactionRounds := 3 }

theorem fewer_graph_hops_do_not_imply_lower_agent_cost :
    oneHopPayloadDetour.graphHops < twoHopBaseline.graphHops ∧
    ¬ NoWorse oneHopPayloadDetour twoHopBaseline := by
  simp [oneHopPayloadDetour, twoHopBaseline, NoWorse]

def localBaseline (_ : Bool) : RouteCost :=
  { graphHops := 2, tokenCost := 4, interactionRounds := 2 }

def localExtension : Bool → RouteCost
  | true => { graphHops := 1, tokenCost := 3, interactionRounds := 1 }
  | false => localBaseline false

def successfulRoute (cost : RouteCost) : EvidenceRoute Unit :=
  {
    evidence := []
    reachesGoal := True
    evidenceValid := True
    measurementContext := {
      graphSnapshotIdentity := 0
      modelProfileIdentity := 0
      semanticSearchCacheIdentity := 0
      modelPrefixCacheIdentity := 0
      measurementProtocolIdentity := 0
      uncertaintyProtocolIdentity := 0
    }
    cost := cost
  }

def localBaselineRoute (workload : Bool) : EvidenceRoute Unit :=
  successfulRoute (localBaseline workload)

def localExtensionRoute (workload : Bool) : EvidenceRoute Unit :=
  successfulRoute (localExtension workload)

theorem one_owner_improvement_does_not_establish_shared_value :
    CertifiedImprovement (localBaselineRoute true) (localExtensionRoute true) ∧
    ¬ IndependentValue
      (fun workload : Bool => workload)
      (fun left right => left ≠ right)
      (fun _ => True)
      localBaselineRoute
      localExtensionRoute := by
  constructor
  · simp [CertifiedImprovement, localBaselineRoute, localExtensionRoute, StrictlyBetter, NoWorse, successfulRoute, localBaseline, localExtension]
  · intro evidence
    rcases evidence with
      ⟨left, right, _, _, ownersDiffer, leftBetter, rightBetter⟩
    cases left <;> cases right <;>
      simp [localExtensionRoute, localBaselineRoute, successfulRoute,
        localExtension, localBaseline, CertifiedImprovement,
        StrictlyBetter, NoWorse] at *

def boundedBaseline (_ : Bool) : RouteCost :=
  { graphHops := 1, tokenCost := 1, interactionRounds := 1 }

def boundedButLeaky (_ : Bool) : RouteCost :=
  { graphHops := 2, tokenCost := 2, interactionRounds := 2 }

def unitBudget : RouteCost :=
  { graphHops := 1, tokenCost := 1, interactionRounds := 1 }

theorem bounded_externality_does_not_imply_isolation :
    BoundedExternality unitBudget boundedBaseline boundedButLeaky ∧
    ¬ Isolated (fun _ : Bool => False) boundedBaseline boundedButLeaky := by
  constructor
  · intro workload
    cases workload <;>
  simp [ExternalityBound, unitBudget, boundedBaseline, boundedButLeaky]
  · intro isolated
    have equality := isolated false (by simp)
    simp [boundedBaseline, boundedButLeaky] at equality

theorem unchanged_router_has_no_admission_value
    :
    ¬ IndependentValue owner independentOwners admittedWorkload
      baseline baseline := by
  intro evidence
  rcases evidence with ⟨left, _, _, _, _, leftBetter, _⟩
  exact leftBetter.2.2.2.2.2.2 rfl

def coldMeasurementContext : MeasurementContext :=
  {
    graphSnapshotIdentity := 7
    modelProfileIdentity := 11
    semanticSearchCacheIdentity := 0
    modelPrefixCacheIdentity := 0
    measurementProtocolIdentity := 3
    uncertaintyProtocolIdentity := 5
  }

def warmSemanticCacheContext : MeasurementContext :=
  {
    graphSnapshotIdentity := 7
    modelProfileIdentity := 11
    semanticSearchCacheIdentity := 1
    modelPrefixCacheIdentity := 0
    measurementProtocolIdentity := 3
    uncertaintyProtocolIdentity := 5
  }

def coldBaselineRoute : EvidenceRoute Unit :=
  {
    evidence := []
    reachesGoal := True
    evidenceValid := True
    measurementContext := coldMeasurementContext
    cost := { graphHops := 3, tokenCost := 8, interactionRounds := 3 }
  }

def warmCacheRoute : EvidenceRoute Unit :=
  {
    evidence := []
    reachesGoal := True
    evidenceValid := True
    measurementContext := warmSemanticCacheContext
    cost := { graphHops := 2, tokenCost := 5, interactionRounds := 2 }
  }

theorem lower_cost_under_different_cache_context_is_not_certified :
    StrictlyBetter warmCacheRoute.cost coldBaselineRoute.cost ∧
    ¬ CertifiedImprovement coldBaselineRoute warmCacheRoute := by
  simp [StrictlyBetter, NoWorse, warmCacheRoute, coldBaselineRoute, CertifiedImprovement, coldMeasurementContext, warmSemanticCacheContext]

def uncertainBaselineCost : RouteCostInterval :=
  {
    lower := { graphHops := 5, tokenCost := 5, interactionRounds := 5 }
    upper := { graphHops := 9, tokenCost := 9, interactionRounds := 9 }
  }

def uncertainCandidateCost : RouteCostInterval :=
  {
    lower := { graphHops := 4, tokenCost := 4, interactionRounds := 4 }
    upper := { graphHops := 10, tokenCost := 10, interactionRounds := 10 }
  }

theorem better_point_estimate_does_not_imply_robust_improvement :
    StrictlyBetter uncertainCandidateCost.lower uncertainBaselineCost.lower ∧
    ¬ RobustlyBetter uncertainCandidateCost uncertainBaselineCost := by
  simp [StrictlyBetter, NoWorse, RobustlyBetter, uncertainCandidateCost, uncertainBaselineCost]

def partialEvidenceRoute : EvidenceRoute Bool :=
  {
    evidence := [true]
    reachesGoal := True
    evidenceValid := True
    measurementContext := {
      graphSnapshotIdentity := 17
      modelProfileIdentity := 4
      semanticSearchCacheIdentity := 0
      modelPrefixCacheIdentity := 0
      measurementProtocolIdentity := 6
      uncertaintyProtocolIdentity := 2
    }
    cost := { graphHops := 1, tokenCost := 1, interactionRounds := 1 }
  }

theorem valid_returned_evidence_does_not_imply_required_evidence_completeness :
    EvidenceItemsValid (fun _ : Bool => True) partialEvidenceRoute ∧
    ¬ EvidenceComplete (fun _ : Bool => True) partialEvidenceRoute := by
  constructor
  · intro evidence _
    trivial
  · intro complete
    have missingFalse := complete false (by trivial)
    simp [partialEvidenceRoute] at missingFalse

structure PersistentGraphState where
  featureEnabled : Bool
  semanticRouteCached : Bool
  learnedRoutePreference : Bool
  derivedGraphIndexPresent : Bool
deriving DecidableEq, Repr

def persistentStateSelectsFeatureRoute
    (state : PersistentGraphState) : Bool :=
  state.featureEnabled ||
  state.semanticRouteCached ||
  state.learnedRoutePreference ||
  state.derivedGraphIndexPresent

def baselinePersistentState : PersistentGraphState :=
  {
    featureEnabled := false
    semanticRouteCached := false
    learnedRoutePreference := false
    derivedGraphIndexPresent := false
  }

def featureDerivedPersistentState : PersistentGraphState :=
  {
    featureEnabled := true
    semanticRouteCached := true
    learnedRoutePreference := true
    derivedGraphIndexPresent := true
  }

def eraseRouterCodeOnly
    (state : PersistentGraphState) : PersistentGraphState :=
  { state with featureEnabled := false }

def eraseFeatureCausalClosure
    (_ : PersistentGraphState) : PersistentGraphState :=
  baselinePersistentState

theorem deleting_router_code_does_not_delete_derived_search_state :
    (eraseRouterCodeOnly featureDerivedPersistentState).featureEnabled = false ∧
    persistentStateSelectsFeatureRoute
      (eraseRouterCodeOnly featureDerivedPersistentState) = true ∧
    eraseRouterCodeOnly featureDerivedPersistentState ≠
      baselinePersistentState := by
  simp [eraseRouterCodeOnly, featureDerivedPersistentState, persistentStateSelectsFeatureRoute, baselinePersistentState]

theorem erasing_feature_causal_closure_restores_baseline_state :
    eraseFeatureCausalClosure featureDerivedPersistentState =
      baselinePersistentState ∧
    persistentStateSelectsFeatureRoute
      (eraseFeatureCausalClosure featureDerivedPersistentState) = false := by
  simp [eraseFeatureCausalClosure, baselinePersistentState, persistentStateSelectsFeatureRoute]

theorem expired_silent_feature_requires_causal_closure_erasure
    (policy : LifecyclePolicy)
    (authorizedRenewal : RenewalReceipt → Prop)
    (expired : policy.expiresAt < now) :
    MustErase now policy authorizedRenewal none ∧
    eraseFeatureCausalClosure featureDerivedPersistentState =
      baselinePersistentState := by
  constructor
  · exact silence_does_not_renew_expired_feature
      policy authorizedRenewal expired
  · rfl

structure CandidateArtifactIdentity where
  providerDigestValid : Bool
  runtimeDigestValid : Bool
  semanticContractDigestValid : Bool
deriving DecidableEq, Repr

def CandidateArtifactIdentity.complete
    (identity : CandidateArtifactIdentity) : Bool :=
  identity.providerDigestValid &&
  identity.runtimeDigestValid &&
  identity.semanticContractDigestValid

inductive CandidateAdmissionReason where
  | missingCandidateArtifactIdentity
  | invalidCandidateArtifactIdentity
deriving DecidableEq, Repr

inductive SharedSearchCoreAdmission where
  | ready (identity : CandidateArtifactIdentity)
  | blocked (reason : CandidateAdmissionReason)
deriving DecidableEq, Repr

def admitSharedRustSearchCore
    (identity : Option CandidateArtifactIdentity) :
    SharedSearchCoreAdmission :=
  match identity with
  | none => .blocked .missingCandidateArtifactIdentity
  | some candidateIdentity =>
      if candidateIdentity.complete then
        .ready candidateIdentity
      else
        .blocked .invalidCandidateArtifactIdentity

def sharedSearchCoreReady : SharedSearchCoreAdmission → Bool
  | .ready _ => true
  | .blocked _ => false

theorem missing_shared_core_identity_is_blocked :
    admitSharedRustSearchCore none =
      .blocked
        .missingCandidateArtifactIdentity := by
  rfl

theorem invalid_shared_core_identity_is_blocked
    (identity : CandidateArtifactIdentity)
    (incomplete : identity.complete = false) :
    admitSharedRustSearchCore (some identity) =
      .blocked .invalidCandidateArtifactIdentity := by
  change
    (if identity.complete then
      SharedSearchCoreAdmission.ready identity
    else
      SharedSearchCoreAdmission.blocked .invalidCandidateArtifactIdentity) =
      SharedSearchCoreAdmission.blocked .invalidCandidateArtifactIdentity
  rw [incomplete]
  rfl

theorem blocked_shared_core_admission_is_not_ready
    (reason : CandidateAdmissionReason) :
    sharedSearchCoreReady (.blocked reason) = false := by
  rfl

theorem identity_failure_cannot_admit_shared_core
    (identity : Option CandidateArtifactIdentity)
    (identityFailure :
      identity = none ∨
      ∃ candidateIdentity,
        identity = some candidateIdentity ∧
        candidateIdentity.complete = false) :
    sharedSearchCoreReady (admitSharedRustSearchCore identity) = false := by
  rcases identityFailure with rfl | ⟨candidateIdentity, rfl, incomplete⟩
  · rfl
  · change
      sharedSearchCoreReady
        (if candidateIdentity.complete then
          SharedSearchCoreAdmission.ready candidateIdentity
        else
          SharedSearchCoreAdmission.blocked .invalidCandidateArtifactIdentity) = false
    rw [incomplete]
    rfl

end SearchRouteEvidenceGraphAdmission
