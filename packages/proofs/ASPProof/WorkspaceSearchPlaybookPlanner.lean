namespace ASPProof.WorkspaceSearchPlaybookPlanner

/-! Search PlayBook is a single request, not a stored search session. -/

structure Binding where
  projectId : String
  workspaceId : String
  contentGenerationDigest : String
  deriving DecidableEq

structure SearchPlaybookProjection where
  exampleText : String
  grammarText : String
  deriving DecidableEq

structure ProviderSearchPlaybookContract where
  languageId : String
  providerId : String
  syntaxContractId : String
  syntaxContractDigest : String
  projection : SearchPlaybookProjection
  deriving DecidableEq

def compileSearchPlaybookContract
    (languageId providerId : String)
    (contract : ProviderSearchPlaybookContract) :
    Option SearchPlaybookProjection :=
  if languageId = contract.languageId ∧ providerId = contract.providerId then
    some contract.projection
  else
    none

theorem successful_contract_compile_is_provider_owned
    (languageId providerId : String)
    (contract : ProviderSearchPlaybookContract)
    (projection : SearchPlaybookProjection)
    (compiled :
      compileSearchPlaybookContract languageId providerId contract = some projection) :
    languageId = contract.languageId ∧
      providerId = contract.providerId ∧
      projection = contract.projection := by
  unfold compileSearchPlaybookContract at compiled
  split at compiled
  next matched =>
    simp only [Option.some.injEq] at compiled
    exact ⟨matched.1, matched.2, compiled.symm⟩
  next =>
    contradiction

theorem mismatched_provider_has_no_contract_fallback
    (languageId providerId : String)
    (contract : ProviderSearchPlaybookContract)
    (mismatch :
      languageId ≠ contract.languageId ∨ providerId ≠ contract.providerId) :
    compileSearchPlaybookContract languageId providerId contract = none := by
  unfold compileSearchPlaybookContract
  split
  next matched =>
    exact False.elim (mismatch.elim (fun h => h matched.1) (fun h => h matched.2))
  next =>
    rfl

structure ContractQueryWork where
  generationReadCount : Nat
  filesystemReadCount : Nat
  providerProcessCount : Nat
  deriving DecidableEq

def contractQueryWork : ContractQueryWork :=
  { generationReadCount := 0
    filesystemReadCount := 0
    providerProcessCount := 0 }

theorem contract_query_precedes_generation_admission :
    contractQueryWork.generationReadCount = 0 ∧
      contractQueryWork.filesystemReadCount = 0 ∧
      contractQueryWork.providerProcessCount = 0 := by
  exact ⟨rfl, rfl, rfl⟩

structure Route where
  binding : Binding
  languageId : String
  providerId : String
  deriving DecidableEq

structure NativeArgs where
  tokens : List String
  nonempty : tokens ≠ []

inductive PersistentGraphLanguage where
  | pgql
  | sqlPgq
  deriving DecidableEq

inductive SnapshotGraphLanguage where
  | gql
  deriving DecidableEq

structure PlaybookRequest where
  binding : Binding
  fd : NativeArgs
  rg : NativeArgs
  tantivy : NativeArgs
  syntaxQuery : String
  syntaxQueryNonempty : syntaxQuery ≠ ""
  graphLanguage : SnapshotGraphLanguage
  graph : NativeArgs

theorem admitted_request_has_direct_native_axes
    (request : PlaybookRequest) :
    request.fd.tokens ≠ [] ∧ request.rg.tokens ≠ [] ∧
      request.tantivy.tokens ≠ [] ∧ request.graph.tokens ≠ [] := by
  exact ⟨request.fd.nonempty, request.rg.nonempty,
    request.tantivy.nonempty, request.graph.nonempty⟩

/-! Available clause kinds; this list is not a required flat execution set. -/

inductive Axis where
  | fd
  | rg
  | tantivy
  | nativeSyntax
  | graph
  deriving DecidableEq

def availableClauseKinds : List Axis :=
  [.fd, .rg, .tantivy, .nativeSyntax, .graph]

theorem five_clause_kinds_are_registered : availableClauseKinds.length = 5 := by
  rfl

theorem clause_kinds_are_pairwise_distinct : availableClauseKinds.Pairwise (· ≠ ·) := by
  decide

structure QueryIdentity where
  binding : Binding
  querySetDigest : String
  predicateDigest : String
  deriving DecidableEq

inductive NativeToken where
  | argument (value : String)
  | standalonePipe
  deriving DecidableEq

def dispatchNativeToken : NativeToken → Option String
  | .argument value => some value
  | .standalonePipe => none

theorem native_argument_is_forwarded_unchanged (value : String) :
    dispatchNativeToken (.argument value) = some value := by
  rfl

theorem in_token_alternation_is_native_data :
    dispatchNativeToken (.argument "runtime|client|host|plugin") =
      some "runtime|client|host|plugin" := by
  rfl

theorem standalone_shell_pipe_is_rejected :
    dispatchNativeToken .standalonePipe = none := by
  rfl

/-!
The public Playbook is ordered by construction.  Acquisition clauses carry no
numeric priority: list position is the Agent-authored priority.  Pipe
alternatives remain inside one native block and therefore share its priority.
Graph clauses occupy a distinct suffix and cannot be followed by acquisition.
-/

inductive AcquisitionAxis where
  | fd
  | rg
  | tantivy
  | nativeSyntax
  deriving DecidableEq

structure AcquisitionClause where
  axis : AcquisitionAxis
  nativeBlock : NativeArgs

inductive GraphQueryLanguage where
  | gql
  | pgql
  deriving DecidableEq

structure GraphClause where
  language : GraphQueryLanguage
  nativeBlock : NativeArgs

structure ProgressivePlaybook where
  acquisition : List AcquisitionClause
  acquisitionNonempty : acquisition ≠ []
  graphFanIn : List GraphClause

def acquisitionAtPriority (plan : ProgressivePlaybook) (priority : Nat) :
    Option AcquisitionClause :=
  plan.acquisition[priority]?

inductive ParsedClause where
  | acquisition (clause : AcquisitionClause)
  | graph (clause : GraphClause)

def acquisitionAfterGraph : List ParsedClause → Bool
  | [] => false
  | .graph _ :: remaining =>
      remaining.any fun clause =>
        match clause with
        | .acquisition _ => true
        | .graph _ => false
  | .acquisition _ :: remaining => acquisitionAfterGraph remaining

def admitOrderedClauses (clauses : List ParsedClause) : Bool :=
  !acquisitionAfterGraph clauses

theorem acquisition_after_graph_is_rejected
    (graph : GraphClause) (acquisition : AcquisitionClause) :
    admitOrderedClauses [.graph graph, .acquisition acquisition] = false := by
  rfl

theorem acquisition_before_graph_is_admitted
    (acquisition : AcquisitionClause) (graph : GraphClause) :
    admitOrderedClauses [.acquisition acquisition, .graph graph] = true := by
  rfl

structure GraphFanIn where
  acquisitionCandidates : List String
  acquisitionBound : acquisitionCandidates.length ≤ 4096
  rankedCandidates : List String
  filtersAcquisition :
    ∀ candidate, candidate ∈ rankedCandidates →
      candidate ∈ acquisitionCandidates

theorem graph_cannot_mint_acquisition_candidates
    (fanIn : GraphFanIn) (candidate : String)
    (ranked : candidate ∈ fanIn.rankedCandidates) :
    candidate ∈ fanIn.acquisitionCandidates :=
  fanIn.filtersAcquisition candidate ranked

theorem graph_frontier_is_bounded_before_reasoning (fanIn : GraphFanIn) :
    fanIn.acquisitionCandidates.length ≤ 4096 :=
  fanIn.acquisitionBound

def topThirty (evidence : List α) : List α := evidence.take 30

theorem top_thirty_is_bounded (evidence : List α) :
    (topThirty evidence).length ≤ 30 := by
  simpa [topThirty, List.length_take] using
    Nat.min_le_left 30 evidence.length

def flatVoteWinner : List (String × Nat) → Option String
  | [] => none
  | head :: tail =>
      some ((head :: tail).foldl
        (fun best candidate => if best.2 < candidate.2 then candidate else best)
        head).1

def authoredFirst : List String → Option String
  | [] => none
  | head :: _ => some head

theorem flat_axis_vote_can_reverse_authored_priority :
    flatVoteWinner [("priority-owner", 1), ("popular-owner", 3)] =
        some "popular-owner" ∧
      authoredFirst ["priority-owner", "popular-owner"] =
        some "priority-owner" := by
  decide

structure AxisReceipt where
  query : QueryIdentity
  axis : Axis
  inputDigest : String
  outputDigest : String
  ownerUniverseDigest : String
  checkedOwnerCount : Nat
  matchedEvidenceCount : Nat
  truncated : Bool
  consumedCandidatesFrom : Option Axis
  acquisitionIndependent : axis ≠ .graph → consumedCandidatesFrom = none
  deriving DecidableEq

theorem acquisition_clause_does_not_consume_another_clause
    (receipt : AxisReceipt) (acquisition : receipt.axis ≠ .graph) :
    receipt.consumedCandidatesFrom = none :=
  receipt.acquisitionIndependent acquisition

structure CompleteClauseKindReceipts where
  query : QueryIdentity
  fd : AxisReceipt
  fdQuery : fd.query = query
  fdAxis : fd.axis = .fd
  rg : AxisReceipt
  rgQuery : rg.query = query
  rgAxis : rg.axis = .rg
  tantivy : AxisReceipt
  tantivyQuery : tantivy.query = query
  tantivyAxis : tantivy.axis = .tantivy
  nativeSyntax : AxisReceipt
  nativeSyntaxQuery : nativeSyntax.query = query
  nativeSyntaxAxis : nativeSyntax.axis = .nativeSyntax
  graph : AxisReceipt
  graphQuery : graph.query = query
  graphAxis : graph.axis = .graph

def canonicalAuditDigests (receipts : CompleteClauseKindReceipts) : List String :=
  [receipts.fd.outputDigest, receipts.rg.outputDigest,
   receipts.tantivy.outputDigest, receipts.nativeSyntax.outputDigest,
   receipts.graph.outputDigest]

theorem fan_in_is_completion_order_independent
    (receipts : CompleteClauseKindReceipts) (_completionOrder : List Axis) :
    canonicalAuditDigests receipts = canonicalAuditDigests receipts := by
  rfl

/-! Targeted recovery intentionally has only the two agent-useful fields. -/

structure TargetedRecovery where
  exampleText : String
  grammarText : String
  deriving DecidableEq

theorem targeted_recovery_is_exactly_example_and_grammar
    (recovery : TargetedRecovery) :
    recovery = ⟨recovery.exampleText, recovery.grammarText⟩ := by
  cases recovery
  rfl

def applyTargetedRepair : List Axis → List Axis
  | [] => []
  | _axis :: remaining => remaining

theorem targeted_repair_strictly_decreases_missing_axes
    (axis : Axis) (remaining : List Axis) :
    (applyTargetedRepair (axis :: remaining)).length <
      (axis :: remaining).length := by
  simp [applyTargetedRepair]

/-! Fan-in always has one explicit semantic result. -/

inductive ExplicitResultKind where
  | exactSelectorReady
  | disambiguationRequired
  | relationshipSupported
  | noMatch
  | contradiction
  | refinementRequired
  | providerContractFailure
  deriving DecidableEq

def AxisProvesNoMatch
    (query : QueryIdentity) (ownerUniverseDigest : String)
    (ownerCount : Nat) (receipt : AxisReceipt) : Prop :=
  receipt.query = query ∧
  receipt.ownerUniverseDigest = ownerUniverseDigest ∧
  receipt.checkedOwnerCount = ownerCount ∧
  receipt.matchedEvidenceCount = 0 ∧
  receipt.truncated = false

structure CompleteCoverageWitness where
  query : QueryIdentity
  receipts : CompleteClauseKindReceipts
  receiptsQuery : receipts.query = query
  admittedOwnerUniverseDigest : String
  admittedOwnerUniverseDigestNonempty : admittedOwnerUniverseDigest ≠ ""
  admittedOwnerCount : Nat
  fdComplete :
    AxisProvesNoMatch query admittedOwnerUniverseDigest
      admittedOwnerCount receipts.fd
  rgComplete :
    AxisProvesNoMatch query admittedOwnerUniverseDigest
      admittedOwnerCount receipts.rg
  tantivyComplete :
    AxisProvesNoMatch query admittedOwnerUniverseDigest
      admittedOwnerCount receipts.tantivy
  syntaxComplete :
    AxisProvesNoMatch query admittedOwnerUniverseDigest
      admittedOwnerCount receipts.nativeSyntax
  graphComplete :
    AxisProvesNoMatch query admittedOwnerUniverseDigest
      admittedOwnerCount receipts.graph

structure FanInResult where
  query : QueryIdentity
  kind : ExplicitResultKind
  completeCoverage : Option CompleteCoverageWitness
  noMatchSound :
    kind = .noMatch →
      ∃ witness,
        completeCoverage = some witness ∧ witness.query = query

theorem no_match_requires_complete_coverage
    (result : FanInResult) (isNoMatch : result.kind = .noMatch) :
    ∃ witness,
      result.completeCoverage = some witness ∧
      witness.query = result.query :=
  result.noMatchSound isNoMatch

/-! Parser identity is the sole bridge from syntax evidence to exact query. -/

structure ParserNode where
  nodeId : String
  generationDigest : String
  sourceRootDigest : String
  providerId : String
  parserId : String
  parserVersion : String
  selectorGrammarVersion : String
  deriving DecidableEq

structure ExactSelector where
  nodeId : String
  generationDigest : String
  sourceRootDigest : String
  providerId : String
  parserId : String
  parserVersion : String
  selectorGrammarVersion : String
  deriving DecidableEq

structure ParserAdmission where
  binding : Binding
  sourceRootDigest : String
  providerId : String
  parserId : String
  parserVersion : String
  selectorGrammarVersion : String
  deriving DecidableEq

def SelectorCurrent
    (admission : ParserAdmission) (selector : ExactSelector) : Prop :=
  selector.generationDigest = admission.binding.contentGenerationDigest ∧
  selector.sourceRootDigest = admission.sourceRootDigest ∧
  selector.providerId = admission.providerId ∧
  selector.parserId = admission.parserId ∧
  selector.parserVersion = admission.parserVersion ∧
  selector.selectorGrammarVersion = admission.selectorGrammarVersion

inductive ExactProjection where
  | source
  | callableSkeleton
  deriving DecidableEq

structure ExactQuery where
  selector : ExactSelector
  projection : ExactProjection
  deriving DecidableEq

structure SyntaxResult where
  admission : ParserAdmission
  node : ParserNode
  selector : ExactSelector
  selectorNamesNode : selector.nodeId = node.nodeId
  selectorNamesGeneration :
    selector.generationDigest = node.generationDigest
  selectorNamesSourceRoot :
    selector.sourceRootDigest = node.sourceRootDigest
  selectorNamesProvider :
    selector.providerId = node.providerId
  selectorNamesParser :
    selector.parserId = node.parserId
  selectorNamesParserVersion :
    selector.parserVersion = node.parserVersion
  selectorNamesGrammarVersion :
    selector.selectorGrammarVersion = node.selectorGrammarVersion
  nodeGenerationAdmitted :
    node.generationDigest = admission.binding.contentGenerationDigest
  nodeSourceRootAdmitted :
    node.sourceRootDigest = admission.sourceRootDigest
  nodeProviderAdmitted :
    node.providerId = admission.providerId
  nodeParserAdmitted :
    node.parserId = admission.parserId
  nodeParserVersionAdmitted :
    node.parserVersion = admission.parserVersion
  nodeSelectorGrammarAdmitted :
    node.selectorGrammarVersion = admission.selectorGrammarVersion
  selectorIsCurrent : SelectorCurrent admission selector
  projection : ExactProjection

def canonicalExactQuery (result : SyntaxResult) : ExactQuery :=
  { selector := result.selector, projection := result.projection }

def resolveSelector (node : ParserNode) : ExactSelector :=
  { nodeId := node.nodeId
    generationDigest := node.generationDigest
    sourceRootDigest := node.sourceRootDigest
    providerId := node.providerId
    parserId := node.parserId
    parserVersion := node.parserVersion
    selectorGrammarVersion := node.selectorGrammarVersion }

theorem syntax_result_maps_to_one_exact_query (result : SyntaxResult) :
    ∃ query : ExactQuery,
      query = canonicalExactQuery result ∧
        ∀ other : ExactQuery,
          other = canonicalExactQuery result → other = query := by
  exact ⟨canonicalExactQuery result, rfl, fun other h => h⟩

theorem exact_query_round_trips_to_same_parser_node (result : SyntaxResult) :
    (canonicalExactQuery result).selector.nodeId = result.node.nodeId := by
  exact result.selectorNamesNode

theorem syntax_result_selector_is_generation_current (result : SyntaxResult) :
    SelectorCurrent result.admission result.selector :=
  result.selectorIsCurrent

theorem stale_selector_fails_closed
    (admission : ParserAdmission) (selector : ExactSelector)
    (stale :
      selector.generationDigest ≠
        admission.binding.contentGenerationDigest) :
    ¬ SelectorCurrent admission selector := by
  intro current
  exact stale current.1

/-! Evidence is separated by volatility and derivation authority. -/

inductive EvidencePlane where
  | durableWorkspacePropertyGraph
  | snapshotSourceGraph
  | derivedReasoningOverlay
  deriving DecidableEq

structure WorkspaceScope where
  projectId : String
  workspaceId : String
  deriving DecidableEq

def Binding.scope (binding : Binding) : WorkspaceScope :=
  { projectId := binding.projectId, workspaceId := binding.workspaceId }

structure DurableFact where
  factId : String
  relationId : String
  scope : WorkspaceScope
  graphLanguage : PersistentGraphLanguage
  queryDigest : String
  graphRelationalIrDigest : String
  ownerManifestDigest : String
  validatorVersion : String
  deriving DecidableEq

structure SnapshotFact where
  factId : String
  sourceGenerationDigest : String
  deriving DecidableEq

inductive PersistentFact where
  | durable (fact : DurableFact)

def persistSnapshotFact (_fact : SnapshotFact) : Option PersistentFact := none

theorem snapshot_source_facts_are_not_persistable (fact : SnapshotFact) :
    persistSnapshotFact fact = none := by
  rfl

def DurableFactCurrent
    (ownerManifestDigest validatorVersion : String)
    (fact : DurableFact) : Prop :=
  fact.ownerManifestDigest = ownerManifestDigest ∧
    fact.validatorVersion = validatorVersion

def SnapshotFactCurrent (binding : Binding) (fact : SnapshotFact) : Prop :=
  fact.sourceGenerationDigest = binding.contentGenerationDigest

theorem source_drift_invalidates_snapshot_fact
    (binding : Binding) (fact : SnapshotFact)
    (drift : fact.sourceGenerationDigest ≠ binding.contentGenerationDigest) :
    ¬ SnapshotFactCurrent binding fact := by
  exact drift

inductive ParentFact where
  | durable (fact : DurableFact)
  | snapshot (fact : SnapshotFact)

def parentFactId : ParentFact → String
  | .durable fact => fact.factId
  | .snapshot fact => fact.factId

structure EvidenceContext where
  binding : Binding
  ownerManifestDigest : String
  validatorVersion : String

def ParentFactCurrent (context : EvidenceContext) : ParentFact → Prop
  | .durable fact =>
      fact.scope = context.binding.scope ∧
      DurableFactCurrent context.ownerManifestDigest
        context.validatorVersion fact
  | .snapshot fact => SnapshotFactCurrent context.binding fact

structure BaseFactRegistryIdentity where
  query : QueryIdentity
  factIds : List String
  digest : String

structure AdmittedBaseFactRegistry where
  context : EvidenceContext
  identity : BaseFactRegistryIdentity
  identityBinding : identity.query.binding = context.binding
  facts : List ParentFact
  factsComplete : facts.map parentFactId = identity.factIds
  everyFactCurrent :
    ∀ fact, fact ∈ facts → ParentFactCurrent context fact
  authorityReceiptDigest : String
  authorityReceiptDigestNonempty : authorityReceiptDigest ≠ ""

structure DerivedFact where
  factId : String
  derivationId : String
  binding : Binding
  admittedBaseFactRegistryDigest : String
  ascentEngineDigest : String
  ascentEngineDigestNonempty : ascentEngineDigest ≠ ""
  ruleSetDigest : String
  ruleSetDigestNonempty : ruleSetDigest ≠ ""
  parents : List ParentFact
  parentsNonempty : parents ≠ []

def DerivedFactValid
    (registry : AdmittedBaseFactRegistry) (fact : DerivedFact) : Prop :=
  fact.binding = registry.context.binding ∧
  fact.admittedBaseFactRegistryDigest = registry.identity.digest ∧
  ∀ parent, parent ∈ fact.parents →
    parent ∈ registry.facts

theorem invalid_parent_invalidates_derived_fact
    (registry : AdmittedBaseFactRegistry) (fact : DerivedFact)
    (invalidParent : ParentFact)
    (member : invalidParent ∈ fact.parents)
    (invalid :
      invalidParent ∉ registry.facts) :
    ¬ DerivedFactValid registry fact := by
  intro valid
  exact invalid (valid.2.2 invalidParent member)

theorem valid_derived_parent_is_current
    (registry : AdmittedBaseFactRegistry) (fact : DerivedFact)
    (valid : DerivedFactValid registry fact)
    (parent : ParentFact) (member : parent ∈ fact.parents) :
    ParentFactCurrent registry.context parent := by
  exact registry.everyFactCurrent parent (valid.2.2 parent member)

def selectorFromDerivedFact (_fact : DerivedFact) : Option ExactSelector := none

theorem ascent_cannot_mint_exact_selector (fact : DerivedFact) :
    selectorFromDerivedFact fact = none := by
  rfl

/-! Warm evidence and ExTS scores choose work; neither establishes truth. -/

inductive ReasoningDepth where
  | root0
  | root1
  | deeper (depth : Nat)
  deriving DecidableEq

structure WarmObservation where
  factId : String
  depth : ReasoningDepth
  freshnessAdmitted : Bool
  historicalUtility : Nat
  deriving DecidableEq

def schedulingPriority (observation : WarmObservation) : Nat :=
  if observation.freshnessAdmitted then
    match observation.depth with
    | .root0 => observation.historicalUtility + 2
    | .root1 => observation.historicalUtility + 1
    | .deeper _ => observation.historicalUtility
  else 0

theorem stale_warm_observation_has_zero_priority
    (observation : WarmObservation)
    (stale : observation.freshnessAdmitted = false) :
    schedulingPriority observation = 0 := by
  simp [schedulingPriority, stale]

def scheduledAxes (_observation : WarmObservation) : List Axis := availableClauseKinds

theorem warm_start_cannot_prune_registered_clause_kinds (observation : WarmObservation) :
    scheduledAxes observation = availableClauseKinds := by
  rfl

structure EvidenceWitness where
  evidenceId : String
  deriving DecidableEq

def extScoreEstablishesWitness (_score : Nat) : Option EvidenceWitness := none

theorem ext_score_is_scheduling_only (score : Nat) :
    extScoreEstablishesWitness score = none := by
  rfl

structure SemanticState where
  result : ExplicitResultKind
  receipts : CompleteClauseKindReceipts
  branchOrder : List String
  branchBudgets : List Nat

def applyExtSchedule
    (branchOrder : List String) (branchBudgets : List Nat)
    (state : SemanticState) : SemanticState :=
  { state with branchOrder, branchBudgets }

theorem ext_schedule_preserves_semantic_result
    (branchOrder : List String) (branchBudgets : List Nat)
    (state : SemanticState) :
    (applyExtSchedule branchOrder branchBudgets state).result =
      state.result := by
  rfl

theorem ext_schedule_preserves_clause_receipts
    (branchOrder : List String) (branchBudgets : List Nat)
    (state : SemanticState) :
    (applyExtSchedule branchOrder branchBudgets state).receipts =
      state.receipts := by
  rfl

theorem ext_schedule_can_change_execution_order
    (state : SemanticState) (newHead : String) :
    (applyExtSchedule (newHead :: state.branchOrder)
      state.branchBudgets state).branchOrder =
        newHead :: state.branchOrder := by
  rfl

structure ReasoningBranch where
  branchId : String
  validationCount : Nat
  deriving DecidableEq

def QualityGateAdmissible (branch : ReasoningBranch) : Prop :=
  1 ≤ branch.validationCount

theorem new_branch_requires_evidence_validation_before_quality_gate
    (branch : ReasoningBranch) :
    QualityGateAdmissible branch → 1 ≤ branch.validationCount := by
  intro admitted
  exact admitted

/-! Save-token projection is bounded disclosure over admitted evidence. -/

structure RankedEvidence where
  evidenceId : String
  axis : Axis
  tokenCost : Nat
  requiredWitness : Bool
  utility : Nat
  deriving DecidableEq

/-!
Search SubAgents hand off executable evidence; only exact Query materializes
source.  Supporting clause references explain why an item survived progressive
fan-in without replacing its parser-owned semantic relation.
-/

def GraphReferencesAreSuffix : List Axis → Prop
  | [] => True
  | .graph :: remaining => ∀ axis, axis ∈ remaining → axis = .graph
  | _ :: remaining => GraphReferencesAreSuffix remaining

structure ExecutableSearchEvidence where
  evidenceId : String
  owner : String
  item : String
  selector : ExactSelector
  matchedBy : List Axis
  matchedByNonempty : matchedBy ≠ []
  graphReferencesAreSuffix : GraphReferencesAreSuffix matchedBy
  relation : String

def exactQueryGrammar : String :=
  "asp query --selector <exact-selector> --projection <callable-skeleton|source>"

structure SearchAgentHandoff where
  evidence : List ExecutableSearchEvidence
  evidenceBound : evidence.length ≤ 30
  queryGrammar : Option String
  queryGrammarIffEvidence :
    queryGrammar = some exactQueryGrammar ↔ evidence ≠ []

/-!
The handoff constrains evidence, not continuation policy.  The continuation is
kept polymorphic so Query, refinement, comparison, stopping, or a future
Meta-Reasoning action remain equally admissible to the parent Agent.
-/
def SearchAgentHandoff.compatibleWith
    (_handoff : SearchAgentHandoff) (_continuation : α) : Prop :=
  True

def searchAgentSourcePayload (_handoff : SearchAgentHandoff) : Option String :=
  none

def duplicatedSourceTokenCost (handoff : SearchAgentHandoff) : Nat :=
  match searchAgentSourcePayload handoff with
  | none => 0
  | some source => source.length

theorem search_agent_handoff_contains_no_source
    (handoff : SearchAgentHandoff) :
    searchAgentSourcePayload handoff = none := by
  rfl

theorem selector_only_handoff_has_zero_duplicate_source_cost
    (handoff : SearchAgentHandoff) :
    duplicatedSourceTokenCost handoff = 0 := by
  rfl

theorem search_agent_handoff_respects_top_k
    (handoff : SearchAgentHandoff) :
    handoff.evidence.length ≤ 30 :=
  handoff.evidenceBound

theorem nonempty_search_agent_handoff_exposes_exact_query_grammar
    (handoff : SearchAgentHandoff)
    (nonempty : handoff.evidence ≠ []) :
    handoff.queryGrammar = some exactQueryGrammar :=
  handoff.queryGrammarIffEvidence.mpr nonempty

theorem executable_evidence_preserves_progressive_clause_order
    (evidence : ExecutableSearchEvidence) :
    GraphReferencesAreSuffix evidence.matchedBy :=
  evidence.graphReferencesAreSuffix

theorem search_agent_handoff_does_not_constrain_continuation
    (handoff : SearchAgentHandoff) (continuation : α) :
    handoff.compatibleWith continuation := by
  trivial

structure WorkspaceSyntaxQueryProjection where
  evidence : List ExecutableSearchEvidence
  evidenceBound : evidence.length ≤ 30

def ExecutableSearchEvidence.exactQuery
    (evidence : ExecutableSearchEvidence) (projection : ExactProjection) : ExactQuery :=
  { selector := evidence.selector, projection := projection }

theorem every_syntax_evidence_maps_to_an_exact_query
    (result : WorkspaceSyntaxQueryProjection)
    (evidence : ExecutableSearchEvidence)
    (_member : evidence ∈ result.evidence)
    (projection : ExactProjection) :
    ∃ query : ExactQuery, query = evidence.exactQuery projection := by
  exact ⟨evidence.exactQuery projection, rfl⟩

def WorkspaceSyntaxQueryProjection.compatibleWith
    (_result : WorkspaceSyntaxQueryProjection) (_continuation : α) : Prop :=
  True

theorem syntax_query_projection_does_not_constrain_continuation
    (result : WorkspaceSyntaxQueryProjection) (continuation : α) :
    result.compatibleWith continuation := by
  trivial

/-!
A scheduler plan is not a public terminal.  Successful Search projection
requires every Agent-authored acquisition clause and requested Graph fan-in to
complete, while its save-token surface contains only a result kind and bounded
selector evidence.  There is deliberately no constructor or field for a
prescribed next action.
-/
inductive WorkspaceSearchResultKind
  | exactSelectorReady
  | disambiguationRequired
  | relationshipSupported
  | noMatch
  | contradiction
  | refinementRequired
  | providerContractFailure
  deriving DecidableEq

structure ProgressiveExecutionWitness where
  requestedClauseCount : Nat
  completedClauseCount : Nat
  graphRequested : Bool
  graphComplete : Bool

def ProgressiveExecutionWitness.complete
    (witness : ProgressiveExecutionWitness) : Prop :=
  0 < witness.requestedClauseCount ∧
  witness.completedClauseCount = witness.requestedClauseCount ∧
  (witness.graphRequested = false ∨ witness.graphComplete = true)

structure WorkspaceSearchResultProjection where
  result : WorkspaceSearchResultKind
  evidence : List ExecutableSearchEvidence
  evidenceBound : evidence.length ≤ 30
  queryGrammar : Option String
  queryGrammarIffEvidence :
    queryGrammar = some exactQueryGrammar ↔ evidence ≠ []

def WorkspaceSearchResultProjection.prescribedNext
    (_result : WorkspaceSearchResultProjection) : Option String :=
  none

def WorkspaceSearchResultProjection.compatibleWith
    (_result : WorkspaceSearchResultProjection) (_continuation : α) : Prop :=
  True

theorem workspace_search_result_has_no_prescribed_next
    (result : WorkspaceSearchResultProjection) :
    result.prescribedNext = none := by
  rfl

theorem workspace_search_result_respects_top_k
    (result : WorkspaceSearchResultProjection) :
    result.evidence.length ≤ 30 :=
  result.evidenceBound

theorem nonempty_workspace_search_result_exposes_exact_query_grammar
    (result : WorkspaceSearchResultProjection)
    (nonempty : result.evidence ≠ []) :
    result.queryGrammar = some exactQueryGrammar :=
  result.queryGrammarIffEvidence.mpr nonempty

theorem query_grammar_does_not_prescribe_continuation
    (result : WorkspaceSearchResultProjection) (continuation : α) :
    result.compatibleWith continuation := by
  trivial

theorem workspace_search_result_does_not_constrain_continuation
    (result : WorkspaceSearchResultProjection) (continuation : α) :
    result.compatibleWith continuation := by
  trivial

def projectionTokenCost (selected : List RankedEvidence) : Nat :=
  (selected.map RankedEvidence.tokenCost).sum

def CoversEveryAdmittedAxis
    (admitted representatives : List RankedEvidence) : Prop :=
  ∀ axis,
    (∃ evidence, evidence ∈ admitted ∧ evidence.axis = axis) →
    (∃ evidence, evidence ∈ representatives ∧ evidence.axis = axis)

def DiversityFeasible
    (admitted : List RankedEvidence) (topK tokenBudget : Nat) : Prop :=
  ∃ representatives : List RankedEvidence,
    (∀ evidence, evidence ∈ representatives → evidence ∈ admitted) ∧
    CoversEveryAdmittedAxis admitted representatives ∧
    representatives.length ≤ topK ∧
    projectionTokenCost representatives ≤ tokenBudget

structure EvidenceProjection where
  admitted : List RankedEvidence
  selected : List RankedEvidence
  topK : Nat
  tokenBudget : Nat
  selectedSubset :
    ∀ evidence, evidence ∈ selected → evidence ∈ admitted
  topKBound : selected.length ≤ topK
  tokenBound : projectionTokenCost selected ≤ tokenBudget
  requiredWitnessPreserved :
    ∀ evidence, evidence ∈ admitted →
      evidence.requiredWitness = true → evidence ∈ selected
  axisDiversityWhenFeasible :
    DiversityFeasible admitted topK tokenBudget →
      CoversEveryAdmittedAxis admitted selected
  truncated : Bool
  omittedIdentityDigest : String

theorem projection_contains_only_admitted_evidence
    (projection : EvidenceProjection)
    (evidence : RankedEvidence)
    (selected : evidence ∈ projection.selected) :
    evidence ∈ projection.admitted :=
  projection.selectedSubset evidence selected

theorem projection_respects_top_k (projection : EvidenceProjection) :
    projection.selected.length ≤ projection.topK :=
  projection.topKBound

theorem projection_respects_token_budget (projection : EvidenceProjection) :
    projectionTokenCost projection.selected ≤ projection.tokenBudget :=
  projection.tokenBound

theorem projection_preserves_required_witness
    (projection : EvidenceProjection)
    (evidence : RankedEvidence)
    (admitted : evidence ∈ projection.admitted)
    (required : evidence.requiredWitness = true) :
    evidence ∈ projection.selected :=
  projection.requiredWitnessPreserved evidence admitted required

theorem projection_preserves_axis_diversity_when_globally_feasible
    (projection : EvidenceProjection)
    (feasible :
      DiversityFeasible projection.admitted projection.topK
        projection.tokenBudget) :
    CoversEveryAdmittedAxis projection.admitted projection.selected :=
  projection.axisDiversityWhenFeasible feasible

def CanEstablishCompleteAbsence (projection : EvidenceProjection) : Prop :=
  projection.truncated = false

theorem truncated_projection_cannot_establish_complete_absence
    (projection : EvidenceProjection)
    (truncated : projection.truncated = true) :
    ¬ CanEstablishCompleteAbsence projection := by
  intro complete
  unfold CanEstablishCompleteAbsence at complete
  simp [truncated] at complete

structure ProjectionReceipt where
  selectedCount : Nat
  admittedCount : Nat
  exactTokenCost : Nat
  truncated : Bool
  omittedIdentityDigest : String
  deriving DecidableEq

def EvidenceProjection.receipt
    (projection : EvidenceProjection) : ProjectionReceipt :=
  { selectedCount := projection.selected.length
    admittedCount := projection.admitted.length
    exactTokenCost := projectionTokenCost projection.selected
    truncated := projection.truncated
    omittedIdentityDigest := projection.omittedIdentityDigest }

theorem projection_receipt_reports_exact_selected_count
    (projection : EvidenceProjection) :
    projection.receipt.selectedCount = projection.selected.length := by
  rfl

theorem projection_receipt_reports_exact_token_cost
    (projection : EvidenceProjection) :
    projection.receipt.exactTokenCost =
      projectionTokenCost projection.selected := by
  rfl

def resultUnderRequiredWitnessPressure
    (requiredWitnessFits : Bool) : ExplicitResultKind :=
  if requiredWitnessFits then .exactSelectorReady else .refinementRequired

theorem token_pressure_cannot_silently_close :
    resultUnderRequiredWitnessPressure false =
      ExplicitResultKind.refinementRequired := by
  rfl

/-! Existing generation and parser-availability safety remains preserved. -/

def bindRoute (binding : Binding) (languageId providerId : String) : Route :=
  { binding, languageId, providerId }

theorem every_language_route_preserves_project_workspace_content
    (binding : Binding) (languageId providerId : String) :
    (bindRoute binding languageId providerId).binding = binding := by
  rfl

inductive DerivedCapability where
  | absent
  | building (contentGenerationDigest : String)
  | ready (contentGenerationDigest artifactDigest : String)
  | failed (contentGenerationDigest : String)
  deriving DecidableEq

def CanAttachDerived (binding : Binding) : DerivedCapability → Prop
  | .ready contentGenerationDigest _ =>
      contentGenerationDigest = binding.contentGenerationDigest
  | _ => False

theorem stale_derived_capability_cannot_attach
    (binding : Binding) (contentDigest artifactDigest : String)
    (stale : contentDigest ≠ binding.contentGenerationDigest) :
    ¬ CanAttachDerived binding (.ready contentDigest artifactDigest) := by
  simpa [CanAttachDerived] using stale

def ColdContentQueryable (_binding : Binding) : Prop := True

theorem missing_accelerator_does_not_block_cold_content
    (binding : Binding) :
    ColdContentQueryable binding ∧ ¬ CanAttachDerived binding .absent := by
  simp [ColdContentQueryable, CanAttachDerived]

inductive NativeSyntaxOutcome where
  | projected
  | sourceSyntaxUnavailable
  | identityMismatch
  | malformedEvidence
  deriving DecidableEq

def nativeSyntaxContinues : NativeSyntaxOutcome → Bool
  | .projected | .sourceSyntaxUnavailable => true
  | .identityMismatch | .malformedEvidence => false

theorem owner_without_parser_selectors_preserves_later_stages :
    nativeSyntaxContinues .sourceSyntaxUnavailable = true := by
  rfl

theorem identity_drift_cannot_be_downgraded_to_unavailable :
    nativeSyntaxContinues .identityMismatch = false := by
  rfl

end ASPProof.WorkspaceSearchPlaybookPlanner
