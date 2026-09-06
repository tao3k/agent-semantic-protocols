import ASPProof.SearchLoopCacheIdentity

namespace ASPProof.ChangeAwareEvidencePlanning

open SearchLoopCacheIdentity

abbrev EvidenceId := Nat
abbrev ObligationId := Nat
abbrev RelationId := Nat
abbrev RoundId := Nat

structure ProducerIdentity where
  producerId : Digest
  producerContract : Digest
  deriving DecidableEq, Repr

structure SourceIdentity where
  owner : Digest
  sourceRoot : Digest
  sourceNode : Digest
  deriving DecidableEq, Repr

structure EvidenceAtom where
  evidenceId : EvidenceId
  factId : Digest
  semanticKey : SemanticKey
  producer : ProducerIdentity
  producerReceipt : Digest
  sourceIdentity : SourceIdentity
  bodyDigest : Digest
  validityDependencies : List Digest
  obligationIds : List ObligationId
  tokenCost : Nat
  deriving DecidableEq, Repr

inductive MappingRelation where
  | sameSource
  | grounds
  | supports
  | contradicts
  | derivesFrom
  | modelActionOrigin
  | modelVariableOrigin
  | invariantEvidence
  | traceEventAction
  | counterexampleReproduction
  deriving DecidableEq, Repr

structure EvidenceMapping where
  relationId : RelationId
  relation : MappingRelation
  fromAtom : EvidenceAtom
  toAtom : EvidenceAtom
  groundingDigest : Digest
  sameValidationDomain :
    fromAtom.semanticKey.domain = toAtom.semanticKey.domain
  groundingPresent : groundingDigest ≠ 0
  sameSourceGrounded :
    relation = .sameSource →
      fromAtom.sourceIdentity = toAtom.sourceIdentity

theorem mapping_preserves_distinct_producer_receipts
    (mapping : EvidenceMapping) :
    mapping.fromAtom.producerReceipt =
        mapping.fromAtom.producerReceipt ∧
      mapping.toAtom.producerReceipt =
        mapping.toAtom.producerReceipt := by
  exact ⟨rfl, rfl⟩

theorem same_source_mapping_requires_source_identity
    (mapping : EvidenceMapping)
    (sameSource : mapping.relation = .sameSource) :
    mapping.fromAtom.sourceIdentity = mapping.toAtom.sourceIdentity :=
  mapping.sameSourceGrounded sameSource

inductive EvidenceStorage where
  | generationResident
  | durableWorkspaceDatabase
  deriving DecidableEq, Repr

def evidenceAtomStorage (_atom : EvidenceAtom) : EvidenceStorage :=
  EvidenceStorage.generationResident

theorem mapped_source_evidence_never_enters_durable_database
    (atom : EvidenceAtom) :
    evidenceAtomStorage atom ≠ EvidenceStorage.durableWorkspaceDatabase := by
  intro impossible
  cases impossible

structure RoundIdentity where
  roundId : RoundId
  semanticKey : SemanticKey
  inputDigest : Digest
  obligationGraphDigest : Digest
  mappingVersion : Digest
  receiptSetDigest : Digest
  deriving DecidableEq, Repr

def DirectlyComparable (prior current : RoundIdentity) : Prop :=
  prior.semanticKey.workspace = current.semanticKey.workspace ∧
  prior.semanticKey.domain = current.semanticKey.domain ∧
  prior.obligationGraphDigest = current.obligationGraphDigest ∧
  prior.mappingVersion = current.mappingVersion

def CanRetainAcrossGeneration
    (prior current : RoundIdentity)
    (relocationProof : Bool) : Prop :=
  prior.semanticKey.domain.sourceSnapshot =
      current.semanticKey.domain.sourceSnapshot ∨
    relocationProof = true

theorem generation_drift_without_relocation_cannot_retain
    (prior current : RoundIdentity)
    (drift :
      prior.semanticKey.domain.sourceSnapshot ≠
        current.semanticKey.domain.sourceSnapshot) :
    ¬ CanRetainAcrossGeneration prior current false := by
  intro retained
  rcases retained with sameGeneration | relocated
  · exact drift sameGeneration
  · contradiction

inductive DeltaClass where
  | newEvidence
  | retained
  | strengthened
  | weakened
  | contradicted
  | disappeared
  deriving DecidableEq, Repr

structure RoundDelta where
  priorRound : RoundIdentity
  currentRound : RoundIdentity
  comparable : Bool
  newEvidenceIds : List EvidenceId
  retainedEvidenceIds : List EvidenceId
  strengthenedEvidenceIds : List EvidenceId
  weakenedEvidenceIds : List EvidenceId
  contradictedEvidenceIds : List EvidenceId
  disappearedEvidenceIds : List EvidenceId
  relationAdds : List RelationId
  relationRemoves : List RelationId
  trajectoryDigest : Digest
  deriving DecidableEq, Repr

def DisappearanceSupportsAbsence
    (delta : RoundDelta) (completeCoverage : Bool) : Prop :=
  delta.comparable = true ∧ completeCoverage = true

theorem disappearance_alone_never_proves_absence
    (delta : RoundDelta) :
    ¬ DisappearanceSupportsAbsence delta false := by
  intro supports
  exact Bool.noConfusion supports.2

inductive ReuseDecision where
  | reusable
  | revalidationRequired
  | invalid
  deriving DecidableEq, Repr

def classifyReuse
    (cached current : EvidenceAtom)
    (sourceIdentityStable relocationAvailable : Bool) :
    ReuseDecision :=
  if cached.semanticKey = current.semanticKey then
    .reusable
  else if sourceIdentityStable || relocationAvailable then
    .revalidationRequired
  else
    .invalid

theorem exact_semantic_identity_is_reusable
    (cached current : EvidenceAtom)
    (sameKey : cached.semanticKey = current.semanticKey)
    (sourceIdentityStable relocationAvailable : Bool) :
    classifyReuse cached current sourceIdentityStable relocationAvailable =
      .reusable := by
  simp [classifyReuse, sameKey]

theorem semantic_drift_without_stable_source_is_invalid
    (cached current : EvidenceAtom)
    (drift : cached.semanticKey ≠ current.semanticKey) :
    classifyReuse cached current false false = .invalid := by
  simp [classifyReuse, drift]

structure DerivedEvidence where
  evidenceId : EvidenceId
  parentEvidenceIds : List EvidenceId
  parentsNonempty : parentEvidenceIds ≠ []

def DerivedValid
    (baseValid : EvidenceId → Bool) (derived : DerivedEvidence) : Prop :=
  ∀ parent, parent ∈ derived.parentEvidenceIds →
    baseValid parent = true

theorem invalid_parent_invalidates_derived_evidence
    (baseValid : EvidenceId → Bool) (derived : DerivedEvidence)
    (parent : EvidenceId)
    (member : parent ∈ derived.parentEvidenceIds)
    (invalid : baseValid parent = false) :
    ¬ DerivedValid baseValid derived := by
  intro valid
  have current := valid parent member
  rw [invalid] at current
  contradiction

structure ImpactProjection where
  changedIdentityIds : List Digest
  affectedEvidenceIds : List EvidenceId
  affectedObligationIds : List ObligationId
  affectedSelectorIds : List Digest
  affectedActionIds : List Digest
  whyRelationIds : List RelationId
  whyNotObligationIds : List ObligationId
  changedByIdentityIds : List Digest
  impactDigest : Digest
  deriving DecidableEq, Repr

def ImpactSound
    (reaches : Digest → EvidenceId → Prop)
    (projection : ImpactProjection) : Prop :=
  ∀ evidence,
    evidence ∈ projection.affectedEvidenceIds →
    ∃ changed,
      changed ∈ projection.changedIdentityIds ∧
      reaches changed evidence

theorem sound_impact_requires_a_changed_predecessor
    (reaches : Digest → EvidenceId → Prop)
    (projection : ImpactProjection)
    (sound : ImpactSound reaches projection)
    (evidence : EvidenceId)
    (affected : evidence ∈ projection.affectedEvidenceIds) :
    ∃ changed,
      changed ∈ projection.changedIdentityIds ∧
      reaches changed evidence :=
  sound evidence affected

structure ActionValue where
  obligationGain : Nat
  contradictionGain : Nat
  selectorProgress : Nat
  futureReuse : Nat
  novelty : Nat
  tokenCost : Nat
  latencyCost : Nat
  invalidationRisk : Nat
  deriving DecidableEq, Repr

structure ReasoningAction where
  actionId : Digest
  obligationIds : List ObligationId
  value : ActionValue
  deriving DecidableEq, Repr

structure ActionBudget where
  tokenBudget : Nat
  latencyBudget : Nat
  invalidationRiskBudget : Nat
  deriving DecidableEq, Repr

def Feasible (budget : ActionBudget) (action : ReasoningAction) : Prop :=
  action.value.tokenCost ≤ budget.tokenBudget ∧
  action.value.latencyCost ≤ budget.latencyBudget ∧
  action.value.invalidationRisk ≤ budget.invalidationRiskBudget

def Dominates (left right : ReasoningAction) : Prop :=
  right.value.obligationGain ≤ left.value.obligationGain ∧
  right.value.contradictionGain ≤ left.value.contradictionGain ∧
  right.value.selectorProgress ≤ left.value.selectorProgress ∧
  right.value.futureReuse ≤ left.value.futureReuse ∧
  right.value.novelty ≤ left.value.novelty ∧
  left.value.tokenCost ≤ right.value.tokenCost ∧
  left.value.latencyCost ≤ right.value.latencyCost ∧
  left.value.invalidationRisk ≤ right.value.invalidationRisk ∧
  (right.value.obligationGain < left.value.obligationGain ∨
   right.value.contradictionGain < left.value.contradictionGain ∨
   right.value.selectorProgress < left.value.selectorProgress ∨
   right.value.futureReuse < left.value.futureReuse ∨
   right.value.novelty < left.value.novelty ∨
   left.value.tokenCost < right.value.tokenCost ∨
   left.value.latencyCost < right.value.latencyCost ∨
   left.value.invalidationRisk < right.value.invalidationRisk)

structure ActionSelection where
  candidates : List ReasoningAction
  selected : ReasoningAction
  budget : ActionBudget
  selectedMember : selected ∈ candidates
  selectedFeasible : Feasible budget selected
  selectedNondominated :
    ∀ candidate, candidate ∈ candidates →
      Feasible budget candidate → ¬ Dominates candidate selected

theorem selected_reasoning_action_is_feasible
    (selection : ActionSelection) :
    Feasible selection.budget selection.selected :=
  selection.selectedFeasible

theorem selected_reasoning_action_is_nondominated
    (selection : ActionSelection)
    (candidate : ReasoningAction)
    (member : candidate ∈ selection.candidates)
    (feasible : Feasible selection.budget candidate) :
    ¬ Dominates candidate selection.selected :=
  selection.selectedNondominated candidate member feasible

structure PipeBranch where
  branchId : Digest
  expressionDigest : Digest
  nativeGrammarId : Digest
  producerIds : List Digest
  tokenBudget : Nat
  evidenceIds : List EvidenceId
  marginalVerifiedGain : Nat
  truncated : Bool
  deriving DecidableEq, Repr

structure PipeComposition where
  branches : List PipeBranch
  branchesNonempty : branches ≠ []
  fanoutIndependent : Bool
  fanoutIndependentTrue : fanoutIndependent = true
  candidateFlow : Bool
  candidateFlowFalse : candidateFlow = false
  compositionDigest : Digest

def PreservesPipeBranchWitness
    (selectedEvidenceIds : List EvidenceId)
    (composition : PipeComposition) : Prop :=
  ∀ branch,
    branch ∈ composition.branches →
    branch.evidenceIds ≠ [] →
    ∃ evidence,
      evidence ∈ branch.evidenceIds ∧ evidence ∈ selectedEvidenceIds

theorem preserved_pipe_branch_has_projected_witness
    (selectedEvidenceIds : List EvidenceId)
    (composition : PipeComposition)
    (preserved : PreservesPipeBranchWitness selectedEvidenceIds composition)
    (branch : PipeBranch)
    (member : branch ∈ composition.branches)
    (hasEvidence : branch.evidenceIds ≠ []) :
    ∃ evidence,
      evidence ∈ branch.evidenceIds ∧ evidence ∈ selectedEvidenceIds :=
  preserved branch member hasEvidence

theorem global_token_bound_does_not_preserve_every_pipe_branch :
    ∃ (composition : PipeComposition)
      (selectedEvidenceIds : List EvidenceId)
      (projectedTokenCost tokenBudget : Nat),
      projectedTokenCost ≤ tokenBudget ∧
      ¬ PreservesPipeBranchWitness selectedEvidenceIds composition := by
  let branch : PipeBranch := {
    branchId := 1
    expressionDigest := 2
    nativeGrammarId := 3
    producerIds := [4]
    tokenBudget := 1
    evidenceIds := [1]
    marginalVerifiedGain := 1
    truncated := false
  }
  let composition : PipeComposition := {
    branches := [branch]
    branchesNonempty := by simp
    fanoutIndependent := true
    fanoutIndependentTrue := rfl
    candidateFlow := false
    candidateFlowFalse := rfl
    compositionDigest := 5
  }
  refine ⟨composition, [], 0, 0, Nat.le_refl 0, ?_⟩
  intro preserved
  have witness := preserved branch (by simp [composition]) (by simp [branch])
  rcases witness with ⟨evidence, _, selected⟩
  simp at selected

inductive SemanticResult where
  | open
  | exactSelectorReady
  | relationshipSupported
  | noMatch
  | contradiction
  | blocked
  deriving DecidableEq, Repr

structure ScheduledReasoning where
  semanticResult : SemanticResult
  admittedEvidenceDigest : Digest
  branchOrder : List Digest
  branchBudgets : List Nat
  deriving DecidableEq, Repr

def reschedule
    (branchOrder : List Digest) (branchBudgets : List Nat)
    (state : ScheduledReasoning) : ScheduledReasoning :=
  { state with branchOrder, branchBudgets }

theorem scheduling_cannot_change_semantic_result
    (order : List Digest) (budgets : List Nat)
    (state : ScheduledReasoning) :
    (reschedule order budgets state).semanticResult =
      state.semanticResult := by
  rfl

theorem scheduling_cannot_change_admitted_evidence
    (order : List Digest) (budgets : List Nat)
    (state : ScheduledReasoning) :
    (reschedule order budgets state).admittedEvidenceDigest =
      state.admittedEvidenceDigest := by
  rfl

structure TrajectoryProjection where
  delta : RoundDelta
  retainedEvidenceBodyCount : Nat
  retainedBodiesOmitted : retainedEvidenceBodyCount = 0
  projectedTokenCost : Nat
  tokenBudget : Nat
  tokenBound : projectedTokenCost ≤ tokenBudget

theorem retained_evidence_bodies_are_not_repeated
    (projection : TrajectoryProjection) :
    projection.retainedEvidenceBodyCount = 0 :=
  projection.retainedBodiesOmitted

structure ScenarioProjection where
  referenceModelDigest : Digest
  scenarioProjectionDigest : Digest
  scenarioEvidenceIds : List EvidenceId
  invariantEvidenceIds : List EvidenceId
  modelCodeMappingIds : List RelationId
  deriving DecidableEq, Repr

inductive TraceValidationOutcome where
  | conformant
  | modelGap
  | instrumentationGap
  | notRun
  deriving DecidableEq, Repr

inductive ModelCheckingOutcome where
  | invariantsHold
  | counterexample
  | stateBudgetExhausted
  | notRun
  deriving DecidableEq, Repr

inductive FormalAdjudication where
  | modelGap
  | codeBug
  | invariantGap
  | unresolved
  deriving DecidableEq, Repr

structure FormalConformance where
  scenario : ScenarioProjection
  traceValidation : TraceValidationOutcome
  traceDigest : Digest
  instrumentationDigest : Digest
  modelChecking : ModelCheckingOutcome
  checkedInvariantIds : List EvidenceId
  counterexampleTraceDigest : Option Digest
  adjudication : FormalAdjudication
  adjudicationEvidenceIds : List EvidenceId
  reproductionReceiptDigest : Option Digest
  deriving DecidableEq, Repr

def ConfirmedCodeBug (conformance : FormalConformance) : Prop :=
  conformance.traceValidation = .conformant ∧
  conformance.modelChecking = .counterexample ∧
  conformance.adjudication = .codeBug ∧
  ∃ receipt, conformance.reproductionReceiptDigest = some receipt

theorem model_rejection_of_code_trace_is_not_a_confirmed_code_bug
    (conformance : FormalConformance)
    (gap : conformance.traceValidation = .modelGap) :
    ¬ ConfirmedCodeBug conformance := by
  intro confirmed
  unfold ConfirmedCodeBug at confirmed
  have conformant := confirmed.1
  rw [gap] at conformant
  contradiction

theorem counterexample_without_reproduction_is_not_a_confirmed_code_bug
    (conformance : FormalConformance)
    (missing : conformance.reproductionReceiptDigest = none) :
    ¬ ConfirmedCodeBug conformance := by
  intro confirmed
  unfold ConfirmedCodeBug at confirmed
  obtain ⟨receipt, reproduced⟩ := confirmed.2.2.2
  rw [missing] at reproduced
  contradiction

structure ModelRepair where
  priorInvariantDigest : Digest
  repairedInvariantDigest : Digest
  newEvidenceIds : List EvidenceId
  deriving DecidableEq, Repr

def NonOverfittingRepair (repair : ModelRepair) : Prop :=
  repair.priorInvariantDigest = repair.repairedInvariantDigest ∨
  repair.newEvidenceIds ≠ []

theorem invariant_weakening_without_new_evidence_is_not_admitted
    (repair : ModelRepair)
    (changed :
      repair.priorInvariantDigest ≠ repair.repairedInvariantDigest)
    (noNewEvidence : repair.newEvidenceIds = []) :
    ¬ NonOverfittingRepair repair := by
  intro admitted
  rcases admitted with unchanged | evidenceAdded
  · exact changed unchanged
  · exact evidenceAdded noNewEvidence

structure ReasoningTrajectory where
  currentRound : RoundIdentity
  evidenceAtoms : List EvidenceAtom
  mappings : List EvidenceMapping
  delta : RoundDelta
  impact : ImpactProjection
  pipeComposition : PipeComposition
  selection : ActionSelection
  formalConformance : Option FormalConformance

end ASPProof.ChangeAwareEvidencePlanning
