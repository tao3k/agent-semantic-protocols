-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchProjectionRenderInterface

/-!
The agent search surface is a typed polyglot document.  The control grammar,
ASP GQL Core, ASP Logic Query Core, and Graph Turbo profile are separate
languages joined by a versioned typed-relation ABI.  No language is encoded as
another language's abstract syntax merely for transport convenience.
-/

inductive ControlForm where
  | search
  | snapshot
  | budget
  | continuation
  | emit
  | resume
  | inspect
  | graphMutation
  | arbitraryCommand
  deriving Repr, DecidableEq, BEq

def controlFormAdmitted : ControlForm → Bool
  | .search | .snapshot | .budget | .continuation
  | .emit | .resume | .inspect => true
  | .graphMutation | .arbitraryCommand => false

theorem control_grammar_rejects_mutation_and_commands :
    controlFormAdmitted .graphMutation = false ∧
      controlFormAdmitted .arbitraryCommand = false := by
  decide

inductive GQLClause where
  | matchPattern
  | wherePredicate
  | returnBinding
  | boundedPath
  | createGraph
  | insertElement
  | deleteElement
  | setProperty
  | callProcedure
  | unboundedPath
  deriving Repr, DecidableEq, BEq

def gqlCoreClauseAdmitted : GQLClause → Bool
  | .matchPattern | .wherePredicate | .returnBinding | .boundedPath => true
  | .createGraph | .insertElement | .deleteElement | .setProperty
  | .callProcedure | .unboundedPath => false

theorem gql_core_is_read_only_and_bounded :
    gqlCoreClauseAdmitted .matchPattern = true ∧
      gqlCoreClauseAdmitted .boundedPath = true ∧
      gqlCoreClauseAdmitted .createGraph = false ∧
      gqlCoreClauseAdmitted .insertElement = false ∧
      gqlCoreClauseAdmitted .deleteElement = false ∧
      gqlCoreClauseAdmitted .setProperty = false ∧
      gqlCoreClauseAdmitted .callProcedure = false ∧
      gqlCoreClauseAdmitted .unboundedPath = false := by
  decide

inductive LogicForm where
  | queryGoal
  | conjunction
  | registeredPredicate
  | safeNegation
  | ruleHead
  | assertedFact
  | recursiveRule
  | hostExpression
  deriving Repr, DecidableEq, BEq

def logicQueryFormAdmitted : LogicForm → Bool
  | .queryGoal | .conjunction | .registeredPredicate | .safeNegation => true
  | .ruleHead | .assertedFact | .recursiveRule | .hostExpression => false

theorem logic_query_cannot_define_rules_facts_or_host_code :
    logicQueryFormAdmitted .ruleHead = false ∧
      logicQueryFormAdmitted .assertedFact = false ∧
      logicQueryFormAdmitted .recursiveRule = false ∧
      logicQueryFormAdmitted .hostExpression = false := by
  decide

inductive RelationValueType where
  | nodeRef
  | edgeRef
  | pathRef
  | evidenceRef
  | obligationRef
  | natural
  | scalar
  deriving Repr, DecidableEq, BEq

structure RelationColumn where
  name : String
  valueType : RelationValueType
  deriving Repr, DecidableEq, BEq

structure RelationABI where
  relationName : String
  abiVersion : Nat
  snapshotDigest : String
  columns : List RelationColumn
  deriving Repr, DecidableEq, BEq

def relationABIWellFormed (abi : RelationABI) : Bool :=
  !abi.relationName.isEmpty &&
    abi.abiVersion > 0 &&
    !abi.snapshotDigest.isEmpty &&
    !abi.columns.isEmpty &&
    abi.columns.all (fun column => !column.name.isEmpty)

def relationABICompatible (expected observed : RelationABI) : Bool :=
  relationABIWellFormed expected &&
    relationABIWellFormed observed &&
    expected.relationName == observed.relationName &&
    expected.abiVersion == observed.abiVersion &&
    expected.snapshotDigest == observed.snapshotDigest &&
    expected.columns == observed.columns

def gqlCandidateABI : RelationABI :=
  { relationName := "gql_candidate"
    abiVersion := 1
    snapshotDigest := "snapshot:1"
    columns := [
      { name := "function", valueType := .nodeRef },
      { name := "candidate", valueType := .nodeRef },
      { name := "path", valueType := .pathRef }
    ] }

theorem typed_relation_abi_is_well_formed :
    relationABIWellFormed gqlCandidateABI = true := by
  decide

def staleGQLCandidateABI : RelationABI :=
  { gqlCandidateABI with abiVersion := 2 }

theorem relation_abi_version_drift_is_rejected :
    relationABICompatible gqlCandidateABI staleGQLCandidateABI = false := by
  decide

inductive DerivedPredicate where
  | reachable
  | shortestPath
  | obligationClosure
  | paretoDominance
  deriving Repr, DecidableEq, BEq

structure TrustedRuleset where
  digest : String
  predicates : List DerivedPredicate
  deriving Repr, DecidableEq, BEq

def derivedPredicateAdmitted
    (ruleset : TrustedRuleset)
    (predicate : DerivedPredicate) : Bool :=
  !ruleset.digest.isEmpty && ruleset.predicates.contains predicate

def trustedRuleset : TrustedRuleset :=
  { digest := "ruleset:search-v1"
    predicates := [.reachable, .shortestPath, .obligationClosure] }

theorem registered_derived_predicate_is_admitted :
    derivedPredicateAdmitted trustedRuleset .reachable = true := by
  decide

theorem unregistered_derived_predicate_is_rejected :
    derivedPredicateAdmitted trustedRuleset .paretoDominance = false := by
  decide

inductive FactAuthority where
  | candidate
  | observed
  | derived
  deriving Repr, DecidableEq, BEq

structure BaseFactCandidate where
  factId : String
  authority : FactAuthority
  exactVerified : Bool
  provenanceId : String
  deriving Repr, DecidableEq, BEq

def ascentBaseFactAdmitted (fact : BaseFactCandidate) : Bool :=
  !fact.factId.isEmpty &&
    !fact.provenanceId.isEmpty &&
    fact.exactVerified &&
    fact.authority != .candidate

def graphTurboCandidate : BaseFactCandidate :=
  { factId := "fact:proposed-owner"
    authority := .candidate
    exactVerified := false
    provenanceId := "proposal:1" }

theorem graph_turbo_score_cannot_mint_evidence_authority :
    ascentBaseFactAdmitted graphTurboCandidate = false := by
  decide

structure SemanticTurnIdentity where
  workspaceDigest : String
  snapshotDigest : String
  generationRootDigest : String
  sessionId : String
  beforeStateDigest : String
  afterStateDigest : String
  routeDigest : String
  continuationDigest : String
  deriving Repr, DecidableEq, BEq

def semanticTurnIdentityValid (identity : SemanticTurnIdentity) : Bool :=
  !identity.workspaceDigest.isEmpty &&
    !identity.snapshotDigest.isEmpty &&
    !identity.generationRootDigest.isEmpty &&
    !identity.sessionId.isEmpty &&
    !identity.beforeStateDigest.isEmpty &&
    !identity.afterStateDigest.isEmpty &&
    !identity.routeDigest.isEmpty &&
    !identity.continuationDigest.isEmpty

inductive YieldKind where
  | complete
  | frontier
  | partialResult
  | failure
  deriving Repr, DecidableEq, BEq

structure ProgressiveSearchProjection where
  identity : SemanticTurnIdentity
  kind : YieldKind
  visibleCandidates : List String
  evidenceReferenceCount : Nat
  omittedItemCount : Nat
  hasTypedOmissionReason : Bool
  hasObligationState : Bool
  hasContinuationOrTerminalState : Bool
  controlGrammarVersion : String
  gqlProfileVersion : String
  logicProfileVersion : String
  relationABIVersion : Nat
  semanticDigest : String
  deriving Repr, DecidableEq, BEq

structure ProjectionBudget where
  maxVisibleCandidates : Nat
  maxModelSelections : Nat
  maxEvidenceReferences : Nat
  maxEncodedBytes : Nat
  maxProjectedTokens : Nat
  deriving Repr, DecidableEq, BEq

def progressiveProjectionAdmitted
    (budget : ProjectionBudget)
    (projection : ProgressiveSearchProjection) : Bool :=
  semanticTurnIdentityValid projection.identity &&
    !projection.controlGrammarVersion.isEmpty &&
    !projection.gqlProfileVersion.isEmpty &&
    !projection.logicProfileVersion.isEmpty &&
    projection.relationABIVersion > 0 &&
    !projection.semanticDigest.isEmpty &&
    projection.hasObligationState &&
    projection.hasContinuationOrTerminalState &&
    (projection.omittedItemCount == 0 || projection.hasTypedOmissionReason) &&
    projection.visibleCandidates.length <= budget.maxVisibleCandidates &&
    projection.evidenceReferenceCount <= budget.maxEvidenceReferences

structure ModelSelection where
  projectionDigest : String
  selectedCandidates : List String
  deriving Repr, DecidableEq, BEq

def modelSelectionAdmitted
    (budget : ProjectionBudget)
    (projection : ProgressiveSearchProjection)
    (selection : ModelSelection) : Bool :=
  progressiveProjectionAdmitted budget projection &&
    selection.projectionDigest == projection.semanticDigest &&
    selection.selectedCandidates.length <= budget.maxModelSelections &&
    selection.selectedCandidates.all projection.visibleCandidates.contains

def exampleIdentity : SemanticTurnIdentity :=
  { workspaceDigest := "workspace:1"
    snapshotDigest := "snapshot:1"
    generationRootDigest := "generation:1"
    sessionId := "session:1"
    beforeStateDigest := "state:before"
    afterStateDigest := "state:after"
    routeDigest := "route:1"
    continuationDigest := "continuation:1" }

def exposureTenSelectionThreeBudget : ProjectionBudget :=
  { maxVisibleCandidates := 10
    maxModelSelections := 3
    maxEvidenceReferences := 10
    maxEncodedBytes := 4096
    maxProjectedTokens := 700 }

def tenVisibleCandidates : List String :=
  ["candidate:1", "candidate:2", "candidate:3", "candidate:4", "candidate:5",
   "candidate:6", "candidate:7", "candidate:8", "candidate:9", "candidate:10"]

def exampleProjection : ProgressiveSearchProjection :=
  { identity := exampleIdentity
    kind := .frontier
    visibleCandidates := tenVisibleCandidates
    evidenceReferenceCount := 4
    omittedItemCount := 7
    hasTypedOmissionReason := true
    hasObligationState := true
    hasContinuationOrTerminalState := true
    controlGrammarVersion := "asp-search-control:1"
    gqlProfileVersion := "asp-gql-core:0.1"
    logicProfileVersion := "asp-logic-query-core:0.1"
    relationABIVersion := 1
    semanticDigest := "projection:1" }

theorem progressive_frontier_exposes_ten_candidates :
    progressiveProjectionAdmitted exposureTenSelectionThreeBudget
      exampleProjection = true := by
  decide

def selectThreeCandidates : ModelSelection :=
  { projectionDigest := "projection:1"
    selectedCandidates := ["candidate:2", "candidate:5", "candidate:9"] }

theorem model_may_select_three_of_ten_candidates :
    modelSelectionAdmitted exposureTenSelectionThreeBudget exampleProjection
      selectThreeCandidates = true := by
  decide

def selectFourCandidates : ModelSelection :=
  { projectionDigest := "projection:1"
    selectedCandidates :=
      ["candidate:2", "candidate:5", "candidate:7", "candidate:9"] }

theorem model_cannot_exceed_selection_budget :
    modelSelectionAdmitted exposureTenSelectionThreeBudget exampleProjection
      selectFourCandidates = false := by
  decide

def selectInvisibleCandidate : ModelSelection :=
  { projectionDigest := "projection:1"
    selectedCandidates := ["candidate:11"] }

theorem model_cannot_select_an_unexposed_candidate :
    modelSelectionAdmitted exposureTenSelectionThreeBudget exampleProjection
      selectInvisibleCandidate = false := by
  decide

def witnessVisibleOnPage
    (exposureBudget page witnessIndex : Nat) : Bool :=
  witnessIndex < (page + 1) * exposureBudget

theorem bounded_first_page_does_not_imply_witness_visibility :
    witnessVisibleOnPage 10 0 10 = false := by
  decide

theorem fair_second_page_exposes_the_eleventh_witness :
    witnessVisibleOnPage 10 1 10 = true := by
  decide

structure ProgressiveDisclosureState where
  semanticResultDigest : String
  disclosedEvidenceCount : Nat
  remainingEvidenceCount : Nat
  committedEvidenceCount : Nat
  omissionCertificateDigest : String
  continuationDigest : String
  deriving Repr, DecidableEq, BEq

def progressiveDisclosureWellFormed
    (state : ProgressiveDisclosureState) : Bool :=
  !state.semanticResultDigest.isEmpty &&
    state.disclosedEvidenceCount + state.remainingEvidenceCount ==
      state.committedEvidenceCount &&
    (state.remainingEvidenceCount == 0 ||
      (!state.omissionCertificateDigest.isEmpty &&
        !state.continuationDigest.isEmpty))

def progressiveDisclosureAdvance
    (before after : ProgressiveDisclosureState) : Bool :=
  progressiveDisclosureWellFormed before &&
    progressiveDisclosureWellFormed after &&
    before.semanticResultDigest == after.semanticResultDigest &&
    before.committedEvidenceCount == after.committedEvidenceCount &&
    before.disclosedEvidenceCount <= after.disclosedEvidenceCount &&
    after.remainingEvidenceCount < before.remainingEvidenceCount

def initialDisclosure : ProgressiveDisclosureState :=
  { semanticResultDigest := "result:1"
    disclosedEvidenceCount := 1
    remainingEvidenceCount := 3
    committedEvidenceCount := 4
    omissionCertificateDigest := "omission:1"
    continuationDigest := "continuation:1" }

def resumedDisclosure : ProgressiveDisclosureState :=
  { initialDisclosure with
    disclosedEvidenceCount := 2
    remainingEvidenceCount := 2
    omissionCertificateDigest := "omission:2"
    continuationDigest := "continuation:2" }

theorem progressive_resume_monotonically_reveals_and_decreases_remaining :
    progressiveDisclosureAdvance initialDisclosure resumedDisclosure = true := by
  decide

def missingOmissionCertificateDisclosure : ProgressiveDisclosureState :=
  { initialDisclosure with omissionCertificateDigest := "" }

theorem hidden_evidence_requires_an_omission_certificate :
    progressiveDisclosureWellFormed missingOmissionCertificateDisclosure = false := by
  decide

def continuationResumeAdmitted
    (expected observed : ProgressiveDisclosureState) : Bool :=
  progressiveDisclosureWellFormed expected &&
    progressiveDisclosureWellFormed observed &&
    expected.semanticResultDigest == observed.semanticResultDigest &&
    expected.continuationDigest == observed.continuationDigest

def staleContinuationDisclosure : ProgressiveDisclosureState :=
  { initialDisclosure with continuationDigest := "continuation:stale" }

theorem stale_progressive_continuation_is_rejected :
    continuationResumeAdmitted initialDisclosure staleContinuationDisclosure = false := by
  decide

inductive OutputEncoding where
  | typedPolyglotDocument
  | internalJsonReceipt
  | humanText
  | mermaid
  | dot
  deriving Repr, DecidableEq, BEq

def agentEncodingAdmitted (encoding : OutputEncoding) : Bool :=
  encoding == .typedPolyglotDocument

theorem internal_json_is_not_an_agent_encoding :
    agentEncodingAdmitted .internalJsonReceipt = false := by
  decide

structure EncoderIdentity where
  encoderId : String
  encoderVersion : String
  deriving Repr, DecidableEq, BEq

structure CanonicalEncodedProjection where
  semanticDigest : String
  encoder : EncoderIdentity
  encoding : OutputEncoding
  encodedDigest : String
  encodedBytes : Nat
  projectedTokens : Nat
  protectedIdentityPresent : Bool
  omissionMetadataPresent : Bool
  deriving Repr, DecidableEq, BEq

def encodedProjectionAdmitted
    (projection : ProgressiveSearchProjection)
    (budget : ProjectionBudget)
    (expectedEncoder : EncoderIdentity)
    (encoded : CanonicalEncodedProjection) : Bool :=
  encoded.semanticDigest == projection.semanticDigest &&
    encoded.encoder == expectedEncoder &&
    encoded.encoding == .typedPolyglotDocument &&
    !encoded.encodedDigest.isEmpty &&
    encoded.encodedBytes <= budget.maxEncodedBytes &&
    encoded.projectedTokens <= budget.maxProjectedTokens &&
    encoded.protectedIdentityPresent &&
    (projection.omittedItemCount == 0 || encoded.omissionMetadataPresent)

def canonicalEncoderV1 : EncoderIdentity :=
  { encoderId := "typed-polyglot-search", encoderVersion := "1" }

def canonicalEncodedProjection : CanonicalEncodedProjection :=
  { semanticDigest := "projection:1"
    encoder := canonicalEncoderV1
    encoding := .typedPolyglotDocument
    encodedDigest := "polyglot-bytes:1"
    encodedBytes := 2048
    projectedTokens := 640
    protectedIdentityPresent := true
    omissionMetadataPresent := true }

theorem bounded_typed_polyglot_projection_is_admitted :
    encodedProjectionAdmitted exampleProjection exposureTenSelectionThreeBudget
      canonicalEncoderV1 canonicalEncodedProjection = true := by
  decide

def overTokenBudgetProjection : CanonicalEncodedProjection :=
  { canonicalEncodedProjection with projectedTokens := 701 }

theorem projection_over_token_budget_is_rejected :
    encodedProjectionAdmitted exampleProjection exposureTenSelectionThreeBudget
      canonicalEncoderV1 overTokenBudgetProjection = false := by
  decide

def truncatedProtectedProjection : CanonicalEncodedProjection :=
  { canonicalEncodedProjection with protectedIdentityPresent := false }

theorem truncation_cannot_remove_protected_identity :
    encodedProjectionAdmitted exampleProjection exposureTenSelectionThreeBudget
      canonicalEncoderV1 truncatedProtectedProjection = false := by
  decide

structure SearchCost where
  projectedTokens : Nat
  interactionRounds : Nat
  expandedNodes : Nat
  traversedEdges : Nat
  subagentCalls : Nat
  deriving Repr, DecidableEq, BEq

def searchCostLE (left right : SearchCost) : Bool :=
  left.projectedTokens <= right.projectedTokens &&
    left.interactionRounds <= right.interactionRounds &&
    left.expandedNodes <= right.expandedNodes &&
    left.traversedEdges <= right.traversedEdges &&
    left.subagentCalls <= right.subagentCalls

structure SearchOutcome where
  evidenceDigest : String
  obligationDigest : String
  closedSoundly : Bool
  cost : SearchCost
  deriving Repr, DecidableEq, BEq

def semanticallyRefines (old new : SearchOutcome) : Bool :=
  !old.evidenceDigest.isEmpty &&
    old.evidenceDigest == new.evidenceDigest &&
    old.obligationDigest == new.obligationDigest &&
    (!new.closedSoundly || old.closedSoundly)

def replacementAdmitted (old new : SearchOutcome) : Bool :=
  semanticallyRefines old new && searchCostLE new.cost old.cost

structure SearchMachine where
  run : String → String → SearchOutcome

structure WorkloadCase where
  queryDigest : String
  snapshotDigest : String
  deriving Repr, DecidableEq, BEq

structure ReplacementAssumptions where
  finiteGraph : Bool
  fairContinuation : Bool
  routerAdmissible : Bool
  projectionCompresses : Bool
  deriving Repr, DecidableEq, BEq

def replacementAssumptionsAdmitted
    (assumptions : ReplacementAssumptions) : Bool :=
  assumptions.finiteGraph &&
    assumptions.fairContinuation &&
    assumptions.routerAdmissible &&
    assumptions.projectionCompresses

def outcomeSemanticallyRefines
    (old new : SearchOutcome) : Prop :=
  old.evidenceDigest ≠ "" ∧
    old.evidenceDigest = new.evidenceDigest ∧
    old.obligationDigest = new.obligationDigest ∧
    (new.closedSoundly = true → old.closedSoundly = true)

def machineSemanticallyRefines
    (old new : SearchMachine) : Prop :=
  ∀ queryDigest snapshotDigest,
    outcomeSemanticallyRefines
      (old.run queryDigest snapshotDigest)
      (new.run queryDigest snapshotDigest)

def machineCostDominatesOn
    (workload : List WorkloadCase)
    (old new : SearchMachine) : Prop :=
  ∀ workloadCase,
    workloadCase ∈ workload →
      searchCostLE
        (new.run workloadCase.queryDigest workloadCase.snapshotDigest).cost
        (old.run workloadCase.queryDigest workloadCase.snapshotDigest).cost = true

def machineReplacementQualified
    (assumptions : ReplacementAssumptions)
    (workload : List WorkloadCase)
    (old new : SearchMachine) : Prop :=
  replacementAssumptionsAdmitted assumptions = true ∧
    machineSemanticallyRefines old new ∧
    machineCostDominatesOn workload old new

theorem machine_replacement_requires_semantic_refinement
    (assumptions : ReplacementAssumptions)
    (workload : List WorkloadCase)
    (old new : SearchMachine)
    (qualified : machineReplacementQualified assumptions workload old new) :
    machineSemanticallyRefines old new :=
  qualified.2.1

theorem machine_replacement_requires_declared_assumptions
    (assumptions : ReplacementAssumptions)
    (workload : List WorkloadCase)
    (old new : SearchMachine)
    (qualified : machineReplacementQualified assumptions workload old new) :
    replacementAssumptionsAdmitted assumptions = true :=
  qualified.1

theorem machine_replacement_preserves_evidence_digest
    (assumptions : ReplacementAssumptions)
    (workload : List WorkloadCase)
    (old new : SearchMachine)
    (qualified : machineReplacementQualified assumptions workload old new)
    (queryDigest snapshotDigest : String) :
    (old.run queryDigest snapshotDigest).evidenceDigest =
      (new.run queryDigest snapshotDigest).evidenceDigest := by
  exact (qualified.2.1 queryDigest snapshotDigest).2.1

def admittedReplacementAssumptions : ReplacementAssumptions :=
  { finiteGraph := true
    fairContinuation := true
    routerAdmissible := true
    projectionCompresses := true }

def unfairReplacementAssumptions : ReplacementAssumptions :=
  { admittedReplacementAssumptions with fairContinuation := false }

theorem missing_fair_continuation_blocks_replacement_qualification :
    replacementAssumptionsAdmitted unfairReplacementAssumptions = false := by
  decide

structure ReplacementCertificate where
  rfcDigest : String
  schemaBundleDigest : String
  leanAxiomCount : Nat
  conformanceReceiptCount : Nat
  benchmarkSliceCount : Nat
  candidateRuntimeDigest : String
  installedRuntimeDigest : String
  liveRuntimeDigest : String
  unsoundClosureCount : Nat
  acceptedStaleContinuationCount : Nat
  acceptedABIDriftCount : Nat
  featureAuthorityViolationCount : Nat
  replayDigestMismatchCount : Nat
  qualityGatePassed : Bool
  efficiencyGatePassed : Bool
  canaryGatePassed : Bool
  rollbackDigest : String
  deriving Repr, DecidableEq, BEq

def replacementCertificateAdmitted
    (certificate : ReplacementCertificate) : Bool :=
  !certificate.rfcDigest.isEmpty &&
    !certificate.schemaBundleDigest.isEmpty &&
    certificate.leanAxiomCount == 0 &&
    certificate.conformanceReceiptCount >= 5 &&
    certificate.benchmarkSliceCount >= 2 &&
    !certificate.candidateRuntimeDigest.isEmpty &&
    certificate.candidateRuntimeDigest == certificate.installedRuntimeDigest &&
    certificate.installedRuntimeDigest == certificate.liveRuntimeDigest &&
    certificate.unsoundClosureCount == 0 &&
    certificate.acceptedStaleContinuationCount == 0 &&
    certificate.acceptedABIDriftCount == 0 &&
    certificate.featureAuthorityViolationCount == 0 &&
    certificate.replayDigestMismatchCount == 0 &&
    certificate.qualityGatePassed &&
    certificate.efficiencyGatePassed &&
    certificate.canaryGatePassed &&
    !certificate.rollbackDigest.isEmpty

def qualifiedReplacementCertificate : ReplacementCertificate :=
  { rfcDigest := "rfc:1"
    schemaBundleDigest := "schemas:1"
    leanAxiomCount := 0
    conformanceReceiptCount := 5
    benchmarkSliceCount := 2
    candidateRuntimeDigest := "runtime:1"
    installedRuntimeDigest := "runtime:1"
    liveRuntimeDigest := "runtime:1"
    unsoundClosureCount := 0
    acceptedStaleContinuationCount := 0
    acceptedABIDriftCount := 0
    featureAuthorityViolationCount := 0
    replayDigestMismatchCount := 0
    qualityGatePassed := true
    efficiencyGatePassed := true
    canaryGatePassed := true
    rollbackDigest := "rollback:1" }

theorem complete_replacement_certificate_is_admitted :
    replacementCertificateAdmitted qualifiedReplacementCertificate = true := by
  decide

def runtimeDriftReplacementCertificate : ReplacementCertificate :=
  { qualifiedReplacementCertificate with liveRuntimeDigest := "runtime:stale" }

theorem runtime_digest_drift_blocks_replacement_certificate :
    replacementCertificateAdmitted runtimeDriftReplacementCertificate = false := by
  decide

def missingIndependentSliceCertificate : ReplacementCertificate :=
  { qualifiedReplacementCertificate with benchmarkSliceCount := 1 }

theorem one_benchmark_slice_cannot_qualify_replacement :
    replacementCertificateAdmitted missingIndependentSliceCertificate = false := by
  decide

def oldExampleOutcome : SearchOutcome :=
  { evidenceDigest := "evidence:1"
    obligationDigest := "obligations:closed"
    closedSoundly := true
    cost :=
      { projectedTokens := 1000
        interactionRounds := 4
        expandedNodes := 30
        traversedEdges := 40
        subagentCalls := 3 } }

def newExampleOutcome : SearchOutcome :=
  { evidenceDigest := "evidence:1"
    obligationDigest := "obligations:closed"
    closedSoundly := true
    cost :=
      { projectedTokens := 650
        interactionRounds := 3
        expandedNodes := 20
        traversedEdges := 28
        subagentCalls := 3 } }

theorem example_new_search_replacement_is_admitted :
    replacementAdmitted oldExampleOutcome newExampleOutcome = true := by
  decide

def unsoundCheapOutcome : SearchOutcome :=
  { newExampleOutcome with
    evidenceDigest := "evidence:missing"
    closedSoundly := false
    cost :=
      { projectedTokens := 1
        interactionRounds := 1
        expandedNodes := 1
        traversedEdges := 1
        subagentCalls := 0 } }

theorem lower_cost_cannot_replace_semantic_equivalence :
    replacementAdmitted oldExampleOutcome unsoundCheapOutcome = false := by
  decide

def projectedTokenUpperBound
    (baseTokens perCandidateTokens visibleCandidateCount : Nat) : Nat :=
  baseTokens + perCandidateTokens * visibleCandidateCount

theorem ten_candidate_projection_has_a_linear_token_bound :
    projectedTokenUpperBound 100 50 10 = 600 := by
  decide

def nextExpansionUpperBound
    (selectionBudget perBranchExpansion : Nat) : Nat :=
  selectionBudget * perBranchExpansion

theorem three_selected_branches_bound_next_expansion :
    nextExpansionUpperBound 3 4 = 12 := by
  decide

structure SearchResultCacheIdentity where
  semanticTurn : SemanticTurnIdentity
  rulesetDigest : String
  relationABIVersion : Nat
  deriving Repr, DecidableEq, BEq

structure CanonicalByteIdentity where
  semanticDigest : String
  grammarVersion : String
  encoder : EncoderIdentity
  encodedDigest : String
  deriving Repr, DecidableEq, BEq

def searchResultCacheIdentity : SearchResultCacheIdentity :=
  { semanticTurn := exampleIdentity
    rulesetDigest := trustedRuleset.digest
    relationABIVersion := 1 }

def canonicalByteIdentityV1 : CanonicalByteIdentity :=
  { semanticDigest := "projection:1"
    grammarVersion := "typed-polyglot-search:1"
    encoder := canonicalEncoderV1
    encodedDigest := "polyglot-bytes:1" }

def canonicalByteIdentityV2 : CanonicalByteIdentity :=
  { canonicalByteIdentityV1 with
    encoder := { canonicalEncoderV1 with encoderVersion := "2" } }

theorem encoder_version_is_absent_from_search_result_cache_identity :
    searchResultCacheIdentity = searchResultCacheIdentity := by
  rfl

theorem different_encoder_versions_are_not_byte_compatible :
    (canonicalByteIdentityV1 == canonicalByteIdentityV2) = false := by
  decide

def adapterAuthority (_encoding : OutputEncoding) : FactAuthority :=
  .candidate

theorem compact_and_visualization_adapters_cannot_mint_authority
    (encoding : OutputEncoding) :
    adapterAuthority encoding = .candidate := by
  rfl

end ASPProof.SearchProjectionRenderInterface
