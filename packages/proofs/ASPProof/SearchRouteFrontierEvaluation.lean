import ASPProof.SearchRouteEvidenceGraphAdmission

namespace SearchRouteFrontierEvaluation

open SearchRouteEvidenceGraphAdmission

abbrev CandidateUniverse (Route : Type) := Route → Prop
abbrev CandidateFrontier (Route : Type) := Route → Prop
abbrev Feasible (Route : Type) := Route → Prop
abbrev EvidenceSufficient (Route : Type) := Route → Prop

def Eligible
    {Route : Type}
    (feasible : Feasible Route)
    (evidenceSufficient : EvidenceSufficient Route)
    (route : Route) : Prop :=
  feasible route ∧ evidenceSufficient route

def LocallyOptimal
    {Route : Type}
    (cost : Route → RouteCost)
    (frontier : CandidateFrontier Route)
    (feasible : Feasible Route)
    (evidenceSufficient : EvidenceSufficient Route)
    (chosen : Route) : Prop :=
  frontier chosen
    ∧ Eligible feasible evidenceSufficient chosen
    ∧ ∀ alternative,
      frontier alternative →
      Eligible feasible evidenceSufficient alternative →
      ¬StrictlyBetter (cost alternative) (cost chosen)

def GloballyOptimal
    {Route : Type}
    (cost : Route → RouteCost)
    (candidateSet : CandidateUniverse Route)
    (feasible : Feasible Route)
    (evidenceSufficient : EvidenceSufficient Route)
    (chosen : Route) : Prop :=
  candidateSet chosen
    ∧ Eligible feasible evidenceSufficient chosen
    ∧ ∀ alternative,
      candidateSet alternative →
      Eligible feasible evidenceSufficient alternative →
      ¬StrictlyBetter (cost alternative) (cost chosen)

def FrontierComplete
    {Route : Type}
    (cost : Route → RouteCost)
    (candidateSet : CandidateUniverse Route)
    (frontier : CandidateFrontier Route)
    (feasible : Feasible Route)
    (evidenceSufficient : EvidenceSufficient Route) : Prop :=
  (∀ route, frontier route → candidateSet route)
    ∧ ∀ candidate,
      candidateSet candidate →
      Eligible feasible evidenceSufficient candidate →
      ∃ representative,
        frontier representative
          ∧ Eligible feasible evidenceSufficient representative
          ∧ NoWorse (cost representative) (cost candidate)

theorem noWorse_trans
    {left middle right : RouteCost}
    (leftMiddle : NoWorse left middle)
    (middleRight : NoWorse middle right) :
    NoWorse left right := by
  exact ⟨
    Nat.le_trans leftMiddle.1 middleRight.1,
    Nat.le_trans leftMiddle.2.1 middleRight.2.1,
    Nat.le_trans leftMiddle.2.2 middleRight.2.2
  ⟩

theorem noWorse_antisymm
    {left right : RouteCost}
    (leftRight : NoWorse left right)
    (rightLeft : NoWorse right left) :
    left = right := by
  cases left
  cases right
  simp_all [NoWorse, Nat.le_antisymm_iff]

theorem strictlyBetter_of_noWorse_of_strictlyBetter
    {left middle right : RouteCost}
    (leftMiddle : NoWorse left middle)
    (middleRight : StrictlyBetter middle right) :
    StrictlyBetter left right := by
  refine ⟨noWorse_trans leftMiddle middleRight.1, ?_⟩
  intro leftRight
  apply middleRight.2
  apply noWorse_antisymm middleRight.1
  simpa [leftRight] using leftMiddle

theorem complete_frontier_local_optimum_is_global
    {Route : Type}
    {cost : Route → RouteCost}
    {candidateSet : CandidateUniverse Route}
    {frontier : CandidateFrontier Route}
    {feasible : Feasible Route}
    {evidenceSufficient : EvidenceSufficient Route}
    {chosen : Route}
    (complete :
      FrontierComplete cost candidateSet frontier feasible evidenceSufficient)
    (localOptimal :
      LocallyOptimal cost frontier feasible evidenceSufficient chosen) :
    GloballyOptimal cost candidateSet feasible evidenceSufficient chosen := by
  refine ⟨complete.1 chosen localOptimal.1, localOptimal.2.1, ?_⟩
  intro alternative alternativeInUniverse alternativeFeasible alternativeBetter
  obtain ⟨representative, representativeInFrontier, representativeFeasible, representativeNoWorse⟩ :=
    complete.2 alternative alternativeInUniverse alternativeFeasible
  exact localOptimal.2.2 representative representativeInFrontier representativeFeasible
    (strictlyBetter_of_noWorse_of_strictlyBetter representativeNoWorse alternativeBetter)

inductive ExampleRoute
  | observed
  | omitted
  deriving DecidableEq

def exampleCost : ExampleRoute → RouteCost
  | .observed => ⟨2, 100, 2⟩
  | .omitted => ⟨1, 40, 1⟩

def exampleUniverse : CandidateUniverse ExampleRoute := fun _ => True
def incompleteFrontier : CandidateFrontier ExampleRoute := fun route => route = .observed
def exampleFeasible : Feasible ExampleRoute := fun _ => True
def exampleEvidenceSufficient : EvidenceSufficient ExampleRoute := fun _ => True

theorem incomplete_frontier_can_hide_a_globally_better_route :
    LocallyOptimal exampleCost incompleteFrontier exampleFeasible
        exampleEvidenceSufficient .observed
      ∧ ¬GloballyOptimal exampleCost exampleUniverse exampleFeasible
        exampleEvidenceSufficient .observed := by
  constructor
  · simp [LocallyOptimal, Eligible, incompleteFrontier, exampleFeasible,
      exampleEvidenceSufficient, exampleCost, StrictlyBetter, NoWorse]
  · intro global
    exact global.2.2 .omitted (by trivial) (by simp [Eligible, exampleFeasible,
      exampleEvidenceSufficient])
      (by simp [exampleCost, StrictlyBetter, NoWorse])

def insufficientEvidence : EvidenceSufficient ExampleRoute
  | .observed => True
  | .omitted => False

theorem cheaper_but_evidence_incomplete_route_is_not_an_eligible_dominator :
    StrictlyBetter (exampleCost .omitted) (exampleCost .observed)
      ∧ ¬Eligible exampleFeasible insufficientEvidence .omitted := by
  simp [exampleCost, StrictlyBetter, NoWorse, Eligible, exampleFeasible,
    insufficientEvidence]

structure EvaluationContext (Identity : Type) where
  graphSnapshotIdentity : Identity
  candidateUniverseIdentity : Identity
  workloadSplitIdentity : Identity
  semanticSearchCacheIdentity : Identity
  modelPrefixCacheIdentity : Identity
  evaluationProtocolIdentity : Identity

structure RouterPolicyComparison (Identity : Type) where
  baselinePolicyIdentity : Identity
  extendedPolicyIdentity : Identity

structure EvaluationSplit (Workload : Type) where
  training : Workload → Prop
  evaluation : Workload → Prop

def NoEvaluationLeakage
    {Workload : Type}
    (split : EvaluationSplit Workload) : Prop :=
  ∀ workload, split.evaluation workload → ¬split.training workload

def EvaluationCovered
    {Workload : Type}
    (admitted : Workload → Prop)
    (split : EvaluationSplit Workload) : Prop :=
  ∀ workload, admitted workload → split.evaluation workload

structure GeneralizationCertificate
    (Identity Workload : Type)
    (admitted : Workload → Prop)
    (split : EvaluationSplit Workload)
    (baselineContext extendedContext : EvaluationContext Identity)
    (policyComparison : RouterPolicyComparison Identity)
    (improves : Workload → Prop) : Prop where
  comparableContext : baselineContext = extendedContext
  distinctRouterPolicies :
    policyComparison.baselinePolicyIdentity ≠
      policyComparison.extendedPolicyIdentity
  noEvaluationLeakage : NoEvaluationLeakage split
  evaluationCovered : EvaluationCovered admitted split
  improvementOnEvaluation : ∀ workload, split.evaluation workload → improves workload

theorem certified_generalization_uses_disjoint_evaluation
    {Identity Workload : Type}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {improves : Workload → Prop}
    (certificate :
      GeneralizationCertificate Identity Workload admitted split
        baselineContext extendedContext policyComparison improves) :
    NoEvaluationLeakage split :=
  certificate.noEvaluationLeakage

theorem certified_generalization_binds_candidate_universe_identity
    {Identity Workload : Type}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {improves : Workload → Prop}
    (certificate :
      GeneralizationCertificate Identity Workload admitted split
        baselineContext extendedContext policyComparison improves) :
    baselineContext.candidateUniverseIdentity =
      extendedContext.candidateUniverseIdentity := by
  exact congrArg EvaluationContext.candidateUniverseIdentity
    certificate.comparableContext

theorem certified_generalization_binds_cache_identities
    {Identity Workload : Type}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {improves : Workload → Prop}
    (certificate :
      GeneralizationCertificate Identity Workload admitted split
        baselineContext extendedContext policyComparison improves) :
    baselineContext.semanticSearchCacheIdentity =
        extendedContext.semanticSearchCacheIdentity
      ∧ baselineContext.modelPrefixCacheIdentity =
        extendedContext.modelPrefixCacheIdentity := by
  constructor
  · exact congrArg EvaluationContext.semanticSearchCacheIdentity
      certificate.comparableContext
  · exact congrArg EvaluationContext.modelPrefixCacheIdentity
      certificate.comparableContext

theorem certified_generalization_compares_distinct_router_policies
    {Identity Workload : Type}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {improves : Workload → Prop}
    (certificate :
      GeneralizationCertificate Identity Workload admitted split
        baselineContext extendedContext policyComparison improves) :
    policyComparison.baselinePolicyIdentity ≠
      policyComparison.extendedPolicyIdentity :=
  certificate.distinctRouterPolicies

abbrev CandidateUniverseVerifier (Identity Route : Type) :=
  Identity → CandidateUniverse Route → Prop

abbrev WorkloadSplitVerifier (Identity Workload : Type) :=
  Identity → EvaluationSplit Workload → Prop

def NonEquivocating
    {Identity Value : Type}
    (verifier : Identity → Value → Prop) : Prop :=
  ∀ identity left right,
    verifier identity left →
    verifier identity right →
    left = right

def ContentAddressed
    {Identity Value : Type}
    (commit : Value → Identity)
    (verifier : Identity → Value → Prop) : Prop :=
  ∀ identity value, verifier identity value ↔ identity = commit value

theorem injective_content_addressing_is_non_equivocating
    {Identity Value : Type}
    {commit : Value → Identity}
    {verifier : Identity → Value → Prop}
    (contentAddressed : ContentAddressed commit verifier)
    (collisionFree : Function.Injective commit) :
    NonEquivocating verifier := by
  intro identity left right leftVerified rightVerified
  apply collisionFree
  exact ((contentAddressed identity left).1 leftVerified).symm.trans
    ((contentAddressed identity right).1 rightVerified)

structure SemanticCommitmentCertificate
    (Identity Route Workload : Type)
    (candidateSet : CandidateUniverse Route)
    (split : EvaluationSplit Workload)
    (baselineContext extendedContext : EvaluationContext Identity)
    (candidateUniverseVerifier : CandidateUniverseVerifier Identity Route)
    (workloadSplitVerifier : WorkloadSplitVerifier Identity Workload)
    (authorizedCandidateUniverseVerifier :
      CandidateUniverseVerifier Identity Route → Prop)
    (authorizedWorkloadSplitVerifier :
      WorkloadSplitVerifier Identity Workload → Prop) : Prop where
  candidateUniverseVerifierAuthorized :
    authorizedCandidateUniverseVerifier candidateUniverseVerifier
  workloadSplitVerifierAuthorized :
    authorizedWorkloadSplitVerifier workloadSplitVerifier
  candidateUniverseVerifierNonEquivocating :
    NonEquivocating candidateUniverseVerifier
  workloadSplitVerifierNonEquivocating :
    NonEquivocating workloadSplitVerifier
  baselineCandidateUniverseBound :
    candidateUniverseVerifier
      baselineContext.candidateUniverseIdentity candidateSet
  extendedCandidateUniverseBound :
    candidateUniverseVerifier
      extendedContext.candidateUniverseIdentity candidateSet
  baselineWorkloadSplitBound :
    workloadSplitVerifier baselineContext.workloadSplitIdentity split
  extendedWorkloadSplitBound :
    workloadSplitVerifier extendedContext.workloadSplitIdentity split

structure BoundGeneralizationCertificate
    (Identity Route Workload : Type)
    (candidateSet : CandidateUniverse Route)
    (admitted : Workload → Prop)
    (split : EvaluationSplit Workload)
    (baselineContext extendedContext : EvaluationContext Identity)
    (policyComparison : RouterPolicyComparison Identity)
    (candidateUniverseVerifier : CandidateUniverseVerifier Identity Route)
    (workloadSplitVerifier : WorkloadSplitVerifier Identity Workload)
    (authorizedCandidateUniverseVerifier :
      CandidateUniverseVerifier Identity Route → Prop)
    (authorizedWorkloadSplitVerifier :
      WorkloadSplitVerifier Identity Workload → Prop)
    (improves : Workload → Prop) : Prop where
  generalization :
    GeneralizationCertificate Identity Workload admitted split
      baselineContext extendedContext policyComparison improves
  semanticCommitment :
    SemanticCommitmentCertificate Identity Route Workload candidateSet split
      baselineContext extendedContext candidateUniverseVerifier
      workloadSplitVerifier authorizedCandidateUniverseVerifier
      authorizedWorkloadSplitVerifier

theorem bound_generalization_uses_authorized_semantic_commitments
    {Identity Route Workload : Type}
    {candidateSet : CandidateUniverse Route}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {candidateUniverseVerifier : CandidateUniverseVerifier Identity Route}
    {workloadSplitVerifier : WorkloadSplitVerifier Identity Workload}
    {authorizedCandidateUniverseVerifier :
      CandidateUniverseVerifier Identity Route → Prop}
    {authorizedWorkloadSplitVerifier :
      WorkloadSplitVerifier Identity Workload → Prop}
    {improves : Workload → Prop}
    (certificate :
      BoundGeneralizationCertificate Identity Route Workload candidateSet
        admitted split baselineContext extendedContext policyComparison
        candidateUniverseVerifier workloadSplitVerifier
        authorizedCandidateUniverseVerifier authorizedWorkloadSplitVerifier
        improves) :
    authorizedCandidateUniverseVerifier candidateUniverseVerifier
      ∧ authorizedWorkloadSplitVerifier workloadSplitVerifier
      ∧ candidateUniverseVerifier
        baselineContext.candidateUniverseIdentity candidateSet
      ∧ candidateUniverseVerifier
        extendedContext.candidateUniverseIdentity candidateSet
      ∧ workloadSplitVerifier baselineContext.workloadSplitIdentity split
      ∧ workloadSplitVerifier extendedContext.workloadSplitIdentity split := by
  exact ⟨
    certificate.semanticCommitment.candidateUniverseVerifierAuthorized,
    certificate.semanticCommitment.workloadSplitVerifierAuthorized,
    certificate.semanticCommitment.baselineCandidateUniverseBound,
    certificate.semanticCommitment.extendedCandidateUniverseBound,
    certificate.semanticCommitment.baselineWorkloadSplitBound,
    certificate.semanticCommitment.extendedWorkloadSplitBound
  ⟩

theorem bound_generalization_verifiers_do_not_equivocate
    {Identity Route Workload : Type}
    {candidateSet : CandidateUniverse Route}
    {admitted : Workload → Prop}
    {split : EvaluationSplit Workload}
    {baselineContext extendedContext : EvaluationContext Identity}
    {policyComparison : RouterPolicyComparison Identity}
    {candidateUniverseVerifier : CandidateUniverseVerifier Identity Route}
    {workloadSplitVerifier : WorkloadSplitVerifier Identity Workload}
    {authorizedCandidateUniverseVerifier :
      CandidateUniverseVerifier Identity Route → Prop}
    {authorizedWorkloadSplitVerifier :
      WorkloadSplitVerifier Identity Workload → Prop}
    {improves : Workload → Prop}
    (certificate :
      BoundGeneralizationCertificate Identity Route Workload candidateSet
        admitted split baselineContext extendedContext policyComparison
        candidateUniverseVerifier workloadSplitVerifier
        authorizedCandidateUniverseVerifier authorizedWorkloadSplitVerifier
        improves) :
    NonEquivocating candidateUniverseVerifier
      ∧ NonEquivocating workloadSplitVerifier :=
  ⟨
    certificate.semanticCommitment.candidateUniverseVerifierNonEquivocating,
    certificate.semanticCommitment.workloadSplitVerifierNonEquivocating
  ⟩

theorem verified_identity_has_cross_receipt_consistency
    {Identity Value : Type}
    {verifier : Identity → Value → Prop}
    (nonEquivocating : NonEquivocating verifier)
    {identity : Identity}
    {left right : Value}
    (leftVerified : verifier identity left)
    (rightVerified : verifier identity right) :
    left = right :=
  nonEquivocating identity left right leftVerified rightVerified

def leftCandidateSet : CandidateUniverse Bool := fun route => route = false
def rightCandidateSet : CandidateUniverse Bool := fun route => route = true

theorem left_candidate_set_differs_from_right_candidate_set :
    leftCandidateSet ≠ rightCandidateSet := by
  intro setsEqual
  have falseMembershipEqual := congrFun setsEqual false
  simp [leftCandidateSet, rightCandidateSet] at falseMembershipEqual

def permissiveCandidateUniverseVerifier :
    CandidateUniverseVerifier Unit Bool :=
  fun _ _ => True

theorem permissive_verifier_accepts_conflicting_bindings :
    permissiveCandidateUniverseVerifier () leftCandidateSet
      ∧ permissiveCandidateUniverseVerifier () rightCandidateSet
      ∧ leftCandidateSet ≠ rightCandidateSet
      ∧ ¬NonEquivocating permissiveCandidateUniverseVerifier := by
  refine ⟨by trivial, by trivial, left_candidate_set_differs_from_right_candidate_set, ?_⟩
  intro nonEquivocating
  exact left_candidate_set_differs_from_right_candidate_set
    (nonEquivocating () leftCandidateSet rightCandidateSet (by trivial) (by trivial))

def opaqueIdentityContext : EvaluationContext Unit where
  graphSnapshotIdentity := ()
  candidateUniverseIdentity := ()
  workloadSplitIdentity := ()
  semanticSearchCacheIdentity := ()
  modelPrefixCacheIdentity := ()
  evaluationProtocolIdentity := ()

theorem equal_candidate_universe_identity_does_not_bind_candidate_semantics :
    opaqueIdentityContext.candidateUniverseIdentity =
        opaqueIdentityContext.candidateUniverseIdentity
      ∧ leftCandidateSet ≠ rightCandidateSet := by
  constructor
  · rfl
  · exact left_candidate_set_differs_from_right_candidate_set

def leakedSingletonSplit : EvaluationSplit Unit where
  training := fun _ => True
  evaluation := fun _ => True

theorem training_success_on_the_evaluation_workload_is_not_generalization :
    (∀ workload, leakedSingletonSplit.training workload → True)
      ∧ ¬NoEvaluationLeakage leakedSingletonSplit := by
  constructor
  · intro _ _
    trivial
  · intro noLeakage
    exact noLeakage () (by trivial) (by trivial)

structure EvaluationGeneration where
  graphSnapshot : Nat
  candidateUniverse : Nat
  workloadSplit : Nat
  routerLearningState : Nat
  deriving DecidableEq

def GenerationNoOlder
    (newer older : EvaluationGeneration) : Prop :=
  older.graphSnapshot ≤ newer.graphSnapshot
    ∧ older.candidateUniverse ≤ newer.candidateUniverse
    ∧ older.workloadSplit ≤ newer.workloadSplit
    ∧ older.routerLearningState ≤ newer.routerLearningState

theorem generation_no_older_trans
    {newest middle oldest : EvaluationGeneration}
    (newestMiddle : GenerationNoOlder newest middle)
    (middleOldest : GenerationNoOlder middle oldest) :
    GenerationNoOlder newest oldest := by
  exact ⟨
    Nat.le_trans middleOldest.1 newestMiddle.1,
    Nat.le_trans middleOldest.2.1 newestMiddle.2.1,
    Nat.le_trans middleOldest.2.2.1 newestMiddle.2.2.1,
    Nat.le_trans middleOldest.2.2.2 newestMiddle.2.2.2
  ⟩

structure EvaluationReceipt (Identity : Type) where
  receiptIdentity : Identity
  evaluationRunIdentity : Identity
  context : EvaluationContext Identity
  generation : EvaluationGeneration
  issuedAt : Nat
  expiresAt : Nat

def FreshAt
    {Identity : Type}
    (now : Nat)
    (receipt : EvaluationReceipt Identity) : Prop :=
  receipt.issuedAt ≤ now ∧ now ≤ receipt.expiresAt

def CurrentAt
    {Identity : Type}
    (currentGeneration : EvaluationGeneration)
    (receipt : EvaluationReceipt Identity) : Prop :=
  receipt.generation = currentGeneration

def Unconsumed
    {Identity : Type}
    (consumed : Identity → Prop)
    (receipt : EvaluationReceipt Identity) : Prop :=
  ¬consumed receipt.receiptIdentity

structure TemporalEvaluationCertificate
    (Identity : Type)
    (now : Nat)
    (currentGeneration : EvaluationGeneration)
    (consumed : Identity → Prop)
    (baselineReceipt extendedReceipt : EvaluationReceipt Identity) : Prop where
  sameEvaluationRun :
    baselineReceipt.evaluationRunIdentity =
      extendedReceipt.evaluationRunIdentity
  distinctReceiptIdentities :
    baselineReceipt.receiptIdentity ≠ extendedReceipt.receiptIdentity
  comparableContext :
    baselineReceipt.context = extendedReceipt.context
  sameGeneration :
    baselineReceipt.generation = extendedReceipt.generation
  baselineFresh : FreshAt now baselineReceipt
  extendedFresh : FreshAt now extendedReceipt
  baselineCurrent : CurrentAt currentGeneration baselineReceipt
  extendedCurrent : CurrentAt currentGeneration extendedReceipt
  baselineUnconsumed : Unconsumed consumed baselineReceipt
  extendedUnconsumed : Unconsumed consumed extendedReceipt

theorem temporal_certificate_uses_current_generation
    {Identity : Type}
    {now : Nat}
    {currentGeneration : EvaluationGeneration}
    {consumed : Identity → Prop}
    {baselineReceipt extendedReceipt : EvaluationReceipt Identity}
    (certificate :
      TemporalEvaluationCertificate Identity now currentGeneration consumed
        baselineReceipt extendedReceipt) :
    baselineReceipt.generation = currentGeneration
      ∧ extendedReceipt.generation = currentGeneration :=
  ⟨certificate.baselineCurrent, certificate.extendedCurrent⟩

theorem temporal_certificate_rejects_consumed_receipts
    {Identity : Type}
    {now : Nat}
    {currentGeneration : EvaluationGeneration}
    {consumed : Identity → Prop}
    {baselineReceipt extendedReceipt : EvaluationReceipt Identity}
    (certificate :
      TemporalEvaluationCertificate Identity now currentGeneration consumed
        baselineReceipt extendedReceipt) :
    ¬consumed baselineReceipt.receiptIdentity
      ∧ ¬consumed extendedReceipt.receiptIdentity :=
  ⟨certificate.baselineUnconsumed, certificate.extendedUnconsumed⟩

def zeroGeneration : EvaluationGeneration := ⟨0, 0, 0, 0⟩
def nextGraphGeneration : EvaluationGeneration := ⟨1, 0, 0, 0⟩

def staleReceipt : EvaluationReceipt Unit where
  receiptIdentity := ()
  evaluationRunIdentity := ()
  context := opaqueIdentityContext
  generation := zeroGeneration
  issuedAt := 0
  expiresAt := 1

def freshReceipt : EvaluationReceipt Unit where
  receiptIdentity := ()
  evaluationRunIdentity := ()
  context := opaqueIdentityContext
  generation := zeroGeneration
  issuedAt := 0
  expiresAt := 3

def exactCandidateUniverseVerifier :
    CandidateUniverseVerifier Unit Bool :=
  fun _ candidateSet => candidateSet = leftCandidateSet

theorem exact_candidate_verifier_is_non_equivocating :
    NonEquivocating exactCandidateUniverseVerifier := by
  intro _ left right leftVerified rightVerified
  exact leftVerified.trans rightVerified.symm

theorem semantic_non_equivocation_does_not_imply_freshness :
    NonEquivocating exactCandidateUniverseVerifier
      ∧ ¬FreshAt 2 staleReceipt := by
  exact ⟨
    exact_candidate_verifier_is_non_equivocating,
    by simp [FreshAt, staleReceipt]
  ⟩

def consumedAllUnitReceipts : Unit → Prop := fun _ => True

theorem fresh_receipt_can_still_be_replayed :
    FreshAt 1 freshReceipt
      ∧ ¬Unconsumed consumedAllUnitReceipts freshReceipt := by
  simp [FreshAt, Unconsumed, freshReceipt, consumedAllUnitReceipts]

def newerRunSameIdentityReceipt : EvaluationReceipt Unit where
  receiptIdentity := ()
  evaluationRunIdentity := ()
  context := opaqueIdentityContext
  generation := nextGraphGeneration
  issuedAt := 0
  expiresAt := 3

theorem same_run_identity_does_not_imply_same_generation :
    freshReceipt.evaluationRunIdentity =
        newerRunSameIdentityReceipt.evaluationRunIdentity
      ∧ freshReceipt.generation ≠ newerRunSameIdentityReceipt.generation := by
  simp [freshReceipt, newerRunSameIdentityReceipt, zeroGeneration,
    nextGraphGeneration]

def ConsumesReceiptPair
    {Identity : Type}
    (consumedBefore consumedAfter : Identity → Prop)
    (baselineReceipt extendedReceipt : EvaluationReceipt Identity) : Prop :=
  ∀ identity,
    consumedAfter identity ↔
      consumedBefore identity
        ∨ identity = baselineReceipt.receiptIdentity
        ∨ identity = extendedReceipt.receiptIdentity

structure AtomicAdmissionTransition
    (Identity : Type)
    (consumedBefore consumedAfter : Identity → Prop)
    (baselineReceipt extendedReceipt : EvaluationReceipt Identity) : Prop where
  baselineInitiallyUnconsumed :
    Unconsumed consumedBefore baselineReceipt
  extendedInitiallyUnconsumed :
    Unconsumed consumedBefore extendedReceipt
  consumesReceiptPair :
    ConsumesReceiptPair consumedBefore consumedAfter
      baselineReceipt extendedReceipt

theorem atomic_admission_marks_both_receipts_consumed
    {Identity : Type}
    {consumedBefore consumedAfter : Identity → Prop}
    {baselineReceipt extendedReceipt : EvaluationReceipt Identity}
    (transition :
      AtomicAdmissionTransition Identity consumedBefore consumedAfter
        baselineReceipt extendedReceipt) :
    consumedAfter baselineReceipt.receiptIdentity
      ∧ consumedAfter extendedReceipt.receiptIdentity := by
  constructor
  · exact (transition.consumesReceiptPair baselineReceipt.receiptIdentity).2
      (Or.inr (Or.inl rfl))
  · exact (transition.consumesReceiptPair extendedReceipt.receiptIdentity).2
      (Or.inr (Or.inr rfl))

def boolIdentityContext : EvaluationContext Bool where
  graphSnapshotIdentity := false
  candidateUniverseIdentity := false
  workloadSplitIdentity := false
  semanticSearchCacheIdentity := false
  modelPrefixCacheIdentity := false
  evaluationProtocolIdentity := false

def falseIdentityReceipt : EvaluationReceipt Bool where
  receiptIdentity := false
  evaluationRunIdentity := false
  context := boolIdentityContext
  generation := zeroGeneration
  issuedAt := 0
  expiresAt := 3

def trueIdentityReceipt : EvaluationReceipt Bool where
  receiptIdentity := true
  evaluationRunIdentity := false
  context := boolIdentityContext
  generation := zeroGeneration
  issuedAt := 0
  expiresAt := 3

def nothingConsumed : Bool → Prop := fun _ => False

theorem read_only_unconsumed_checks_do_not_consume_receipts :
    Unconsumed nothingConsumed falseIdentityReceipt
      ∧ Unconsumed nothingConsumed trueIdentityReceipt
      ∧ ¬ConsumesReceiptPair nothingConsumed nothingConsumed
        falseIdentityReceipt trueIdentityReceipt := by
  simp [Unconsumed, ConsumesReceiptPair, nothingConsumed,
    falseIdentityReceipt, trueIdentityReceipt]

end SearchRouteFrontierEvaluation
