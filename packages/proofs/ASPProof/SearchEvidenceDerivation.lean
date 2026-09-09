-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchEvidenceReflection

/-! Abstract design checks for R11–R20. Identities and certificates are model
values, not proofs of a parser, digest implementation, or running retriever. -/
namespace ASPProof.SearchEvidenceDerivation
open SearchEvidenceReflection

structure ConditionalEdge where
  snapshot : Nat
  allowed : List Configuration
  deriving DecidableEq, Repr

def holdsAt (snapshot : Nat) (cfg : Configuration) (edge : ConditionalEdge) : Prop :=
  edge.snapshot = snapshot ∧ cfg ∈ edge.allowed

/- This condition licenses a jointly possible behavior witness. A graph may
   still display mutually exclusive alternatives without asserting this path.
   Graph adjacency is a separate producer obligation, not modeled here. -/
def jointPath (snapshot : Nat) (edges : List ConditionalEdge) : Prop :=
  ∃ cfg, ∀ edge ∈ edges, holdsAt snapshot cfg edge

def unixEdge : ConditionalEdge := ⟨1, [.unix]⟩
def windowsEdge : ConditionalEdge := ⟨1, [.windows]⟩

theorem same_snapshot_individual_validity_is_insufficient :
    unixEdge.snapshot = windowsEdge.snapshot ∧
    holdsAt 1 .unix unixEdge ∧ holdsAt 1 .windows windowsEdge ∧
    ¬ jointPath 1 [unixEdge, windowsEdge] := by
  refine ⟨rfl, by unfold holdsAt; decide, by unfold holdsAt; decide, ?_⟩
  rintro ⟨cfg, h⟩
  have u := (h unixEdge (by simp)).2
  have w := (h windowsEdge (by simp)).2
  cases cfg <;> simp [unixEdge, windowsEdge] at u w

theorem joint_path_has_one_context (snapshot : Nat) (edges : List ConditionalEdge)
    (h : jointPath snapshot edges) :
    ∃ cfg, ∀ edge ∈ edges, edge.snapshot = snapshot ∧ cfg ∈ edge.allowed := h

theorem joint_path_subpath (snapshot : Nat) (edges subset : List ConditionalEdge)
    (h : jointPath snapshot edges) (included : ∀ e ∈ subset, e ∈ edges) :
    jointPath snapshot subset := by
  obtain ⟨cfg, valid⟩ := h
  exact ⟨cfg, fun e he => valid e (included e he)⟩

theorem joint_append_requires_shared_context (snapshot : Nat)
    (left right : List ConditionalEdge) :
    jointPath snapshot (left ++ right) ↔
      ∃ cfg, (∀ e ∈ left, holdsAt snapshot cfg e) ∧
        (∀ e ∈ right, holdsAt snapshot cfg e) := by
  constructor
  · rintro ⟨cfg, valid⟩
    exact ⟨cfg, (fun e he => valid e (List.mem_append_left right he)),
      (fun e he => valid e (List.mem_append_right left he))⟩
  · rintro ⟨cfg, leftValid, rightValid⟩
    refine ⟨cfg, ?_⟩
    intro e he
    rcases List.mem_append.mp he with hl | hr
    · exact leftValid e hl
    · exact rightValid e hr

def fullQuery (a b : Bool) : Bool := a && !b

theorem positive_clause_does_not_prove_query :
    (true : Bool) = true ∧ fullQuery true true = false := by decide

theorem query_satisfaction_requires_negative (a b : Bool) :
    fullQuery a b = true ↔ a = true ∧ b = false := by
  cases a <;> cases b <;> decide

theorem missing_negative_has_two_worlds :
    fullQuery true false = true ∧ fullQuery true true = false := by decide

/- Reuse the certified three-valued coverage model. No arbitrary `covered`
   list is accepted without its ValidView proof. -/
def fullDirectory : CertifiedView [membership, storeCall] :=
  ⟨⟨[membership, storeCall], [membership, storeCall]⟩, by
    constructor <;> intro f h
    · exact h
    · exact Iff.rfl⟩

def partialDirectory : CertifiedView [membership, storeCall] :=
  ⟨projectView [membership] fullDirectory.view,
    projection_preserves_validity _ _ _ fullDirectory.sound⟩

theorem partial_directory_preserves_positive :
    certifiedAnswer partialDirectory membership = .supported := by decide

theorem partial_directory_does_not_rule_out_omitted_member :
    certifiedAnswer partialDirectory storeCall = .unknown := by decide

theorem certified_absence_requires_truth_absence (truth : List Fact)
    (view : CertifiedView truth) (q : Fact)
    (h : certifiedAnswer view q = .ruledOut) : q ∉ truth :=
  certified_negative_is_not_merely_missing truth view q h

/- Concrete budget cardinalities, kept separate from semantic coverage. -/
set_option maxRecDepth 4096 in
theorem directory_budget_250_32_is_partial :
    (List.range 250).length = 250 ∧
    ((List.range 250).take 32).length = 32 ∧
    249 ∈ List.range 250 ∧ 249 ∉ (List.range 250).take 32 := by decide

/- Search execution cardinality belongs to one admitted Runtime generation.
   Tokio scheduling concurrency is intentionally absent from both structures:
   readiness scheduling cannot widen or truncate semantic evidence. -/
structure RuntimeSearchGenerationCardinality where
  generation : Nat
  indexedOwners : Nat
  corpusBytes : Nat
  corpusLines : Nat
  graphNodes : Nat
  graphEdges : Nat
  deriving DecidableEq, Repr

structure RuntimeSearchExecutionBudgetReceipt where
  generation : Nat
  rgMatches : Nat
  lexicalOwners : Nat
  syntaxSelectors : Nat
  graphCandidateOwners : Nat
  graphDepth : Nat
  graphNodes : Nat
  graphEdges : Nat
  graphResults : Nat
  evidenceItems : Nat
  deriving DecidableEq, Repr

def deriveRuntimeSearchExecutionBudget
    (cardinality : RuntimeSearchGenerationCardinality) :
    RuntimeSearchExecutionBudgetReceipt :=
  { generation := cardinality.generation
    rgMatches := cardinality.corpusLines
    lexicalOwners := cardinality.indexedOwners
    syntaxSelectors := max cardinality.corpusBytes cardinality.indexedOwners
    graphCandidateOwners := cardinality.indexedOwners
    graphDepth := min cardinality.graphNodes 16
    graphNodes := min cardinality.graphNodes 256
    graphEdges := min cardinality.graphEdges 1024
    graphResults := min cardinality.indexedOwners 30
    evidenceItems := 30 }

def runtimeSearchExecutionBudgetAdmitted
    (cardinality : RuntimeSearchGenerationCardinality)
    (candidate : RuntimeSearchExecutionBudgetReceipt) : Bool :=
  candidate == deriveRuntimeSearchExecutionBudget cardinality

theorem derived_runtime_search_execution_budget_is_admitted
    (cardinality : RuntimeSearchGenerationCardinality) :
    runtimeSearchExecutionBudgetAdmitted cardinality
      (deriveRuntimeSearchExecutionBudget cardinality) = true := by
  simp [runtimeSearchExecutionBudgetAdmitted]

theorem leaf_rg_limit_cannot_widen_runtime_budget
    (cardinality : RuntimeSearchGenerationCardinality)
    (candidate : RuntimeSearchExecutionBudgetReceipt)
    (widened : candidate.rgMatches ≠ cardinality.corpusLines) :
    runtimeSearchExecutionBudgetAdmitted cardinality candidate = false := by
  simp [runtimeSearchExecutionBudgetAdmitted, deriveRuntimeSearchExecutionBudget]
  intro admitted
  exact widened (congrArg RuntimeSearchExecutionBudgetReceipt.rgMatches admitted)

theorem derived_graph_budget_respects_v1_envelope
    (cardinality : RuntimeSearchGenerationCardinality) :
    (deriveRuntimeSearchExecutionBudget cardinality).graphDepth ≤ 16 ∧
      (deriveRuntimeSearchExecutionBudget cardinality).graphNodes ≤ 256 ∧
      (deriveRuntimeSearchExecutionBudget cardinality).graphEdges ≤ 1024 ∧
      (deriveRuntimeSearchExecutionBudget cardinality).graphResults ≤ 30 := by
  exact ⟨Nat.min_le_right _ _, Nat.min_le_right _ _, Nat.min_le_right _ _,
    Nat.min_le_right _ _⟩

/- Tokio may schedule independent workspace requests in any physical order,
   while semantic identity remains the full request key. Scheduling state and
   task counts are deliberately absent from this key. -/
structure RuntimeWorkspaceRequestKey where
  project : Nat
  workspace : Nat
  worktree : Nat
  generation : Nat
  session : Nat
  request : Nat
  deriving DecidableEq, Repr

theorem workspace_identity_partitions_runtime_requests
    (left right : RuntimeWorkspaceRequestKey)
    (differentWorkspace : left.workspace ≠ right.workspace) : left ≠ right := by
  intro equalKeys
  exact differentWorkspace (congrArg RuntimeWorkspaceRequestKey.workspace equalKeys)

/- Query readiness is the immutable resident base. Search attachments refine
   that base but cannot revoke it; this prevents topology, graph, or lexical
   construction failure from becoming an exact-Query failure. -/
structure RuntimeGenerationReadiness where
  residentBase : Bool
  executionPublication : Bool
  providerBinding : Bool
  graphAttachment : Bool
  tantivyAttachment : Bool
  topologyAttachment : Bool
  deriving DecidableEq, Repr

def runtimeQueryBaseReady (readiness : RuntimeGenerationReadiness) : Bool :=
  readiness.residentBase && readiness.executionPublication && readiness.providerBinding

def runtimeSearchReady (readiness : RuntimeGenerationReadiness) : Bool :=
  runtimeQueryBaseReady readiness && readiness.graphAttachment &&
    readiness.tantivyAttachment && readiness.topologyAttachment

theorem search_attachment_state_cannot_revoke_query_base
    (graph tantivy topology : Bool) :
    runtimeQueryBaseReady ⟨true, true, true, graph, tantivy, topology⟩ = true := by
  cases graph <;> cases tantivy <;> cases topology <;> decide

theorem topology_failure_blocks_search_without_revoking_query :
    let readiness : RuntimeGenerationReadiness :=
      ⟨true, true, true, true, true, false⟩
    runtimeQueryBaseReady readiness = true ∧ runtimeSearchReady readiness = false := by
  decide

abbrev EvidenceAtom := Witness

structure DisplayRow where
  alias : Nat
  evidence : EvidenceAtom
  collector : Collector
  deriving DecidableEq, Repr

def support (rows : List DisplayRow) (atom : EvidenceAtom) : Prop :=
  atom ∈ rows.map DisplayRow.evidence

def normalizedSupport (rows : List DisplayRow) : List EvidenceAtom :=
  (rows.map DisplayRow.evidence).eraseDups

def attribution (rows : List DisplayRow) (atom : EvidenceAtom) : List Collector :=
  ((rows.filter (fun row => row.evidence == atom)).map DisplayRow.collector).eraseDups

theorem normalized_support_membership (rows : List DisplayRow) (atom : EvidenceAtom) :
    atom ∈ normalizedSupport rows ↔ support rows atom := by
  simp [normalizedSupport, support]

theorem support_ignores_duplicate_rows (rows : List DisplayRow) (atom : EvidenceAtom) :
    support (rows ++ rows) atom ↔ support rows atom := by
  simp [support]

theorem support_ignores_reordering (a b : List DisplayRow) (atom : EvidenceAtom) :
    support (a ++ b) atom ↔ support (b ++ a) atom := by
  simp only [support, List.map_append, List.mem_append]
  exact or_comm

def rename (f : Nat → Nat) (row : DisplayRow) : DisplayRow :=
  { row with alias := f row.alias }

theorem support_ignores_aliases (f : Nat → Nat) (rows : List DisplayRow)
    (atom : EvidenceAtom) :
    support (rows.map (rename f)) atom ↔ support rows atom := by
  simp [support, List.map_map, Function.comp_def, rename]

theorem support_retains_attribution :
    (normalizedSupport [⟨0, occurrenceA, .rg⟩, ⟨1, occurrenceA, .lexical⟩]).length = 1 ∧
    attribution [⟨0, occurrenceA, .rg⟩, ⟨1, occurrenceA, .lexical⟩] occurrenceA =
      [.rg, .lexical] := by decide

theorem changed_binding_is_distinct_support :
    (normalizedSupport [⟨0, occurrenceA, .rg⟩,
      ⟨0, ⟨storeCall, .firstLiteral, afterBodyEdit⟩, .rg⟩]).length = 2 := by decide

/- Every premise is retained as an evidence identity, even if its display
   is omitted. Resolvability is a certificate obligation, not alias lookup. -/
structure Trace (archive : List EvidenceAtom) where
  premises : List EvidenceAtom
  resolvable : ∀ p ∈ premises, p ∈ archive

theorem trace_premises_recoverable (archive : List EvidenceAtom) (trace : Trace archive) :
    ∀ p ∈ trace.premises, p ∈ archive := trace.resolvable

/- Budget bound applies to one question-bound episode at a fixed binding.
   A new question may start another episode and revisit the same target.
   A new binding starts a new run; this theorem does not license old visited
   state reuse across bindings. The model proves budget termination, not that
   each selected query is useful or that producer coverage is authentic. -/
inductive BudgetRun (binding : Nat) : Nat → Nat → Nat → Prop where
  | done (budget : Nat) : BudgetRun binding budget 0 budget
  | step {budget count remaining : Nat} :
      BudgetRun binding budget count remaining →
      BudgetRun binding (budget + 1) (count + 1) remaining

theorem run_budget_conservation {binding budget count remaining : Nat}
    (run : BudgetRun binding budget count remaining) : count + remaining = budget := by
  induction run with
  | done => simp
  | step _ ih => omega

theorem run_has_finite_step_bound {binding budget count remaining : Nat}
    (run : BudgetRun binding budget count remaining) : count ≤ budget := by
  have h := run_budget_conservation run
  omega

theorem prior_budget_is_less_than_one_step_extended_budget (budget : Nat) :
    budget < budget + 1 := by omega

structure FrontierPotential where
  remainingBudget : Nat
  unresolvedCount : Nat
  deriving DecidableEq, Repr

def spendFrontierBudget : FrontierPotential → Option FrontierPotential
  | ⟨0, _⟩ => none
  | ⟨budget + 1, unresolved⟩ => some ⟨budget, unresolved⟩

theorem frontier_step_strictly_decreases_remaining_budget
    (budget unresolved : Nat) :
    spendFrontierBudget ⟨budget + 1, unresolved⟩ = some ⟨budget, unresolved⟩ ∧
      budget < budget + 1 := by simp [spendFrontierBudget]

def cycleNext (node : Bool) : Bool := !node

theorem eligible_call_cycle_returns_to_start (node : Bool) :
    cycleNext (cycleNext node) = node := by cases node <;> decide

def eligible (source target : Bool) : Bool := target == cycleNext source

theorem eligibility_alone_allows_two_way_cycle :
    eligible false true = true ∧ eligible true false = true := by decide

theorem spent_budget_cannot_take_another_step {binding count remaining : Nat}
    (run : BudgetRun binding 0 count remaining) : count = 0 := by
  have h := run_has_finite_step_bound run
  omega

/- rg and Tantivy jointly determine the file-context scope. Syntax layers may
   extract facts from that scope, but cannot add an owner to it. -/
structure AcquisitionCandidate where
  pathId : Nat
  binding : Nat
  deriving DecidableEq, Repr

structure RetrievalScopes where
  rgOwners : List Nat
  tantivyOwners : List Nat
  deriving DecidableEq, Repr

def fusedFileContextScope (scopes : RetrievalScopes) : List Nat :=
  scopes.rgOwners.filter fun owner => owner ∈ scopes.tantivyOwners

theorem owner_in_fused_scope_iff_in_rg_and_tantivy
    (scopes : RetrievalScopes) (owner : Nat) :
    owner ∈ fusedFileContextScope scopes ↔
      owner ∈ scopes.rgOwners ∧ owner ∈ scopes.tantivyOwners := by
  simp [fusedFileContextScope]

def exampleRetrievalScopes : RetrievalScopes :=
  ⟨[1, 2, 4], [2, 3, 4]⟩

def fusedScope : List Nat := fusedFileContextScope exampleRetrievalScopes

theorem retrieval_only_owner_is_not_in_fused_scope :
    1 ∉ fusedFileContextScope exampleRetrievalScopes ∧
    3 ∉ fusedFileContextScope exampleRetrievalScopes := by decide

theorem jointly_retrieved_owner_is_in_fused_scope :
    2 ∈ fusedFileContextScope exampleRetrievalScopes ∧
    4 ∈ fusedFileContextScope exampleRetrievalScopes := by decide

structure ScopeCalibration where
  languages : Option (List Nat)
  documents : Option (List Nat)
  deriving DecidableEq, Repr

def fileContextScopeWithCalibration
    (scopes : RetrievalScopes) (_calibration : ScopeCalibration) : List Nat :=
  fusedFileContextScope scopes

theorem optional_calibration_cannot_mint_file_context
    (scopes : RetrievalScopes) (calibration : ScopeCalibration) :
    fileContextScopeWithCalibration scopes calibration = fusedFileContextScope scopes := by
  rfl

inductive StructuralQueryForm where
  | treeSitterSExpression
  | astGrepPattern
  | exactNativeSelector
  deriving DecidableEq, Repr

structure StructuralMatch where
  ownerPathId : Nat
  selectorId : Nat
  queryForm : StructuralQueryForm
  deriving DecidableEq, Repr

def matchesInsideFileContext (fileContextScope : List Nat)
    (found : List StructuralMatch) : List StructuralMatch :=
  found.filter fun matched => matched.ownerPathId ∈ fileContextScope

def structuralFrontier (fileContextScope : List Nat) (explicitRequested : Bool)
    (automatic explicit : List StructuralMatch) : List StructuralMatch :=
  matchesInsideFileContext fileContextScope
    (if explicitRequested then explicit else automatic)

theorem structural_frontier_owner_is_in_file_context
    (fileContextScope : List Nat) (explicitRequested : Bool)
    (automatic explicit : List StructuralMatch) (matched : StructuralMatch)
    (member : matched ∈ structuralFrontier fileContextScope explicitRequested automatic explicit) :
    matched.ownerPathId ∈ fileContextScope := by
  simp only [structuralFrontier, matchesInsideFileContext, List.mem_filter] at member
  exact of_decide_eq_true member.2

def automaticStructuralMatches : List StructuralMatch :=
  [⟨2, 20, .treeSitterSExpression⟩, ⟨4, 40, .treeSitterSExpression⟩]

def exactStructuralMatches : List StructuralMatch :=
  [⟨2, 21, .exactNativeSelector⟩]

theorem explicit_structural_query_replaces_automatic_frontier :
    structuralFrontier fusedScope true automaticStructuralMatches exactStructuralMatches =
      exactStructuralMatches := by decide

theorem structural_query_cannot_reintroduce_owner_outside_file_context :
    structuralFrontier fusedScope true automaticStructuralMatches
      [⟨1, 11, .astGrepPattern⟩] = [] := by decide

structure NativeProjection where
  ownerPathId : Nat
  binding : Nat
  registered : Bool
  parsed : Bool
  selector : Node
  deriving DecidableEq, Repr

def selectorFrom (fileContextScope : List Nat) (candidate : AcquisitionCandidate)
    (projection : NativeProjection) : Option Node :=
  if candidate.pathId ∈ fileContextScope &&
    candidate.pathId == projection.ownerPathId &&
    candidate.binding == projection.binding && projection.registered && projection.parsed
  then some projection.selector
  else none

def candidate : AcquisitionCandidate := ⟨2, 1⟩
def outsideCandidate : AcquisitionCandidate := ⟨1, 1⟩
def validProjection : NativeProjection := ⟨2, 1, true, true, .refresh⟩
def unregisteredProjection : NativeProjection := ⟨2, 1, false, true, .refresh⟩
def staleProjection : NativeProjection := ⟨2, 2, true, true, .refresh⟩

theorem admitted_native_projection_mints_selector :
    selectorFrom fusedScope candidate validProjection = some .refresh := by decide

theorem candidate_path_alone_does_not_mint_selector :
    selectorFrom fusedScope candidate unregisteredProjection = none := by decide

theorem stale_native_projection_does_not_mint_selector :
    selectorFrom fusedScope candidate staleProjection = none := by decide

theorem syntax_projection_cannot_escape_fused_scope :
    selectorFrom fusedScope outsideCandidate
      { validProjection with ownerPathId := outsideCandidate.pathId } = none := by decide

structure NativeSyntaxAnchor where
  ownerPathId : Nat
  providerId : Nat
  hasItemFragment : Bool
  canonical : Bool
  binding : Nat
  deriving DecidableEq, Repr

def nativeSyntaxAnchorAdmitted (fileContextScope : List Nat)
    (expectedBinding : Nat) (anchor : NativeSyntaxAnchor) : Bool :=
  anchor.ownerPathId ∈ fileContextScope && anchor.providerId != 0 &&
    anchor.hasItemFragment && anchor.canonical &&
    anchor.binding == expectedBinding

def bareFileUri : NativeSyntaxAnchor := ⟨2, 1, false, true, 1⟩
def canonicalItemAnchor : NativeSyntaxAnchor := ⟨2, 1, true, true, 1⟩
def staleItemAnchor : NativeSyntaxAnchor := ⟨2, 1, true, true, 2⟩
def outsideItemAnchor : NativeSyntaxAnchor := ⟨1, 1, true, true, 1⟩

theorem bare_file_uri_is_not_a_native_syntax_anchor :
    nativeSyntaxAnchorAdmitted fusedScope 1 bareFileUri = false := by decide

theorem canonical_item_selector_can_anchor_native_syntax :
    nativeSyntaxAnchorAdmitted fusedScope 1 canonicalItemAnchor = true := by decide

theorem stale_item_selector_cannot_anchor_native_syntax :
    nativeSyntaxAnchorAdmitted fusedScope 1 staleItemAnchor = false := by decide

theorem native_syntax_anchor_cannot_escape_fused_scope :
    nativeSyntaxAnchorAdmitted fusedScope 1 outsideItemAnchor = false := by decide

/- A reasoning frontier ranks core candidates separately from the witness
   connectors required to explain them. Display omission is not absence. -/
structure RankedFrontier where
  candidateCount : Nat
  coreLimit : Nat
  selectedCount : Nat
  connectorCount : Nat
  omittedCount : Nat
  deriving DecidableEq, Repr

def frontier100x10 : RankedFrontier := ⟨100, 10, 10, 2, 90⟩

def coreTop10 : List Nat := (List.range 100).take 10

def witnessClosure (selected : List Nat) : List Nat :=
  if 1 ∈ selected then (0 :: selected).eraseDups else selected.eraseDups

theorem top_k_keeps_bounded_core_and_reports_omission :
    frontier100x10.selectedCount = frontier100x10.coreLimit ∧
    frontier100x10.selectedCount + frontier100x10.omittedCount =
      frontier100x10.candidateCount := by decide

theorem omitted_candidate_is_not_in_rendered_top_k :
    99 ∈ List.range 100 ∧ 99 ∉ coreTop10 := by decide

theorem witness_closure_retains_required_connector :
    0 ∈ witnessClosure [1] ∧ 1 ∈ witnessClosure [1] := by decide

theorem flat_top_k_can_drop_required_connector :
    1 ∈ [1] ∧ 0 ∉ [1] := by decide

/- Ascent output is a derivation delta, not a second rendering of base GQL
   facts. Existing facts are removed while genuinely new conclusions remain. -/
def derivationDelta (base inferred : List Nat) : List Nat :=
  (inferred.filter fun fact => fact ∉ base).eraseDups

theorem restating_gql_facts_produces_no_ascent_output :
    derivationDelta [1, 2] [1, 2, 1] = [] := by decide

theorem a_new_relation_survives_the_derivation_delta :
    derivationDelta [1, 2] [1, 2, 3] = [3] := by decide

/- Dense rows from one provider can occupy a naive prefix. A diversified
   frontier may retain a distinct relevant provider, but no provider receives
   a slot merely because it exists. -/
def denseProviderOrder : List Nat := [1, 1, 1, 2]
def naiveProviderTop3 : List Nat := denseProviderOrder.take 3
def diversifiedRelevantProviders : List Nat := [1, 2]

theorem naive_prefix_can_hide_a_distinct_provider :
    2 ∈ denseProviderOrder ∧ 2 ∉ naiveProviderTop3 := by decide

theorem diversified_frontier_can_retain_distinct_relevant_providers :
    1 ∈ diversifiedRelevantProviders ∧ 2 ∈ diversifiedRelevantProviders := by decide

theorem an_unselected_provider_has_no_automatic_quota :
    3 ∉ diversifiedRelevantProviders := by decide

/- R16: queryability is language-neutral. A registered native parser mints a
   canonical selector for every admitted syntax or document node it owns.
   Structured projections such as jq refine that selected node; they are not a
   substitute selector authority. -/
inductive NativeProducer where
  | rust | python | typescript | scheme | julia | org | markdown | json | nix
  deriving DecidableEq, Repr

inductive NativeNodeKind where
  | implementation | method | function | heading | object | attribute | form
  deriving DecidableEq, Repr

structure ParsedNativeNode where
  producer : NativeProducer
  kind : NativeNodeKind
  binding : Nat
  registered : Bool
  parsed : Bool
  canonicalSelector : Option Nat
  projectionProgram : Option Nat
  deriving DecidableEq, Repr

structure NativeQueryMapping where
  producer : NativeProducer
  selector : Nat
  projectionProgram : Option Nat
  deriving DecidableEq, Repr

def directQueryMapping (expectedBinding : Nat)
    (node : ParsedNativeNode) : Option NativeQueryMapping :=
  if node.binding == expectedBinding && node.registered && node.parsed then
    node.canonicalSelector.map fun selector =>
      ⟨node.producer, selector, node.projectionProgram⟩
  else none

def rustMethodNode : ParsedNativeNode :=
  ⟨.rust, .method, 1, true, true, some 11, none⟩

def orgHeadingNode : ParsedNativeNode :=
  ⟨.org, .heading, 1, true, true, some 21, none⟩

def markdownHeadingNode : ParsedNativeNode :=
  ⟨.markdown, .heading, 1, true, true, some 31, none⟩

def jsonObjectWithJqNode : ParsedNativeNode :=
  ⟨.json, .object, 1, true, true, some 41, some 42⟩

def pythonFunctionNode : ParsedNativeNode :=
  ⟨.python, .function, 1, true, true, some 51, none⟩

def typescriptFunctionNode : ParsedNativeNode :=
  ⟨.typescript, .function, 1, true, true, some 61, none⟩

def schemeFunctionNode : ParsedNativeNode :=
  ⟨.scheme, .function, 1, true, true, some 71, none⟩

def juliaFunctionNode : ParsedNativeNode :=
  ⟨.julia, .function, 1, true, true, some 81, none⟩

def nixAttributeNode : ParsedNativeNode :=
  ⟨.nix, .attribute, 1, true, true, some 91, none⟩

def jsonObjectWithoutSelector : ParsedNativeNode :=
  ⟨.json, .object, 1, true, true, none, some 42⟩

def staleDocumentNode : ParsedNativeNode :=
  ⟨.org, .heading, 2, true, true, some 21, none⟩

theorem admitted_polyglot_nodes_have_direct_query_mappings :
    directQueryMapping 1 rustMethodNode = some ⟨.rust, 11, none⟩ ∧
    directQueryMapping 1 orgHeadingNode = some ⟨.org, 21, none⟩ ∧
    directQueryMapping 1 markdownHeadingNode = some ⟨.markdown, 31, none⟩ ∧
    directQueryMapping 1 jsonObjectWithJqNode = some ⟨.json, 41, some 42⟩ ∧
    directQueryMapping 1 pythonFunctionNode = some ⟨.python, 51, none⟩ ∧
    directQueryMapping 1 typescriptFunctionNode = some ⟨.typescript, 61, none⟩ ∧
    directQueryMapping 1 schemeFunctionNode = some ⟨.scheme, 71, none⟩ ∧
    directQueryMapping 1 juliaFunctionNode = some ⟨.julia, 81, none⟩ ∧
    directQueryMapping 1 nixAttributeNode = some ⟨.nix, 91, none⟩ := by decide

abbrev ProducerRelation := NativeProducer × NativeProducer

def polyglotPipelineRelations : List ProducerRelation :=
  [(.rust, .python), (.julia, .python), (.python, .json),
   (.scheme, .json), (.typescript, .json), (.org, .json),
   (.markdown, .typescript), (.nix, .python)]

structure PolyglotPipelineFact where
  entry : NativeProducer
  ranker : NativeProducer
  scorer : NativeProducer
  packet : NativeProducer
  validator : NativeProducer
  renderer : NativeProducer
  contract : NativeProducer
  guide : NativeProducer
  package : NativeProducer
  deriving DecidableEq, Repr

def derivePolyglotPipeline (relations : List ProducerRelation) :
    Option PolyglotPipelineFact :=
  if relations.contains (.rust, .python) &&
      relations.contains (.julia, .python) &&
      relations.contains (.python, .json) &&
      relations.contains (.scheme, .json) &&
      relations.contains (.typescript, .json) &&
      relations.contains (.org, .json) &&
      relations.contains (.markdown, .typescript) &&
      relations.contains (.nix, .python) then
    some ⟨.rust, .python, .julia, .json, .scheme,
      .typescript, .org, .markdown, .nix⟩
  else none

theorem heterogeneous_relations_derive_one_polyglot_pipeline :
    derivePolyglotPipeline polyglotPipelineRelations =
      some ⟨.rust, .python, .julia, .json, .scheme,
        .typescript, .org, .markdown, .nix⟩ := by decide

theorem missing_language_relation_blocks_polyglot_pipeline :
    derivePolyglotPipeline
      (polyglotPipelineRelations.erase (.scheme, .json)) = none := by decide

theorem jq_projection_does_not_replace_a_canonical_selector :
    jsonObjectWithoutSelector.projectionProgram = some 42 ∧
    directQueryMapping 1 jsonObjectWithoutSelector = none := by decide

theorem stale_document_selector_is_not_queryable :
    directQueryMapping 1 staleDocumentNode = none := by decide

theorem every_admitted_selector_has_a_direct_mapping
    (expectedBinding selector : Nat) (node : ParsedNativeNode)
    (bindingMatches : node.binding = expectedBinding)
    (registered : node.registered = true)
    (parsed : node.parsed = true)
    (hasSelector : node.canonicalSelector = some selector) :
    directQueryMapping expectedBinding node =
      some ⟨node.producer, selector, node.projectionProgram⟩ := by
  simp [directQueryMapping, bindingMatches, registered, parsed, hasSelector]

/- A SourceExcerpt is a first-class evidence surface for a source format for
   which no registered native parser applies. It is not a recovery path for a
   native node whose selector resolution failed. -/
structure SourceExcerpt where
  pathId : Nat
  binding : Nat
  firstLine : Nat
  lastLine : Nat
  nativeParserApplicable : Bool
  selectorResolutionFailed : Bool
  deriving DecidableEq, Repr

def sourceExcerptAdmitted (expectedBinding : Nat) (excerpt : SourceExcerpt) : Bool :=
  excerpt.binding == expectedBinding && excerpt.firstLine > 0 &&
    excerpt.firstLine ≤ excerpt.lastLine &&
    !excerpt.nativeParserApplicable && !excerpt.selectorResolutionFailed

def unsupportedTextExcerpt : SourceExcerpt := ⟨90, 1, 80, 159, false, false⟩

def failedNativeSelectorExcerpt : SourceExcerpt := ⟨91, 1, 80, 159, true, true⟩

theorem source_excerpt_is_context_not_query_authority :
    sourceExcerptAdmitted 1 unsupportedTextExcerpt = true ∧
    directQueryMapping 1 jsonObjectWithoutSelector = none := by decide

theorem native_selector_failure_cannot_downgrade_to_source_excerpt :
    sourceExcerptAdmitted 1 failedNativeSelectorExcerpt = false := by decide

/- R20: topology combines a parser substrate, declared human semantics, and
   evidence-anchored model synthesis without collapsing their modalities. -/
structure ProjectRevision where
  structuralShape : Nat
  bodyContent : Nat
  deriving DecidableEq, Repr

def revisionA : ProjectRevision := ⟨7, 11⟩
def bodyEditedRevision : ProjectRevision := ⟨7, 12⟩
def structurallyEditedRevision : ProjectRevision := ⟨8, 12⟩

def structuralTopologyIdentity (revision : ProjectRevision) : Nat :=
  revision.structuralShape

def revisionContentIdentity (revision : ProjectRevision) : Nat :=
  revision.bodyContent

theorem body_only_edit_reuses_structural_topology_not_content :
    structuralTopologyIdentity revisionA =
      structuralTopologyIdentity bodyEditedRevision ∧
    revisionContentIdentity revisionA ≠
      revisionContentIdentity bodyEditedRevision := by decide

theorem structural_edit_changes_topology_identity :
    structuralTopologyIdentity revisionA ≠
      structuralTopologyIdentity structurallyEditedRevision := by decide

inductive TopologyModality where
  | parserFact | declared | synthesizedProposed | synthesizedAccepted
  deriving DecidableEq, Repr

def factualPremiseEligible : TopologyModality → Bool
  | .parserFact | .declared | .synthesizedAccepted => true
  | .synthesizedProposed => false

def frontierPremiseEligible : TopologyModality → Bool
  | .parserFact | .declared | .synthesizedProposed | .synthesizedAccepted => true

theorem proposed_semantics_cannot_prove_behavior :
    factualPremiseEligible .synthesizedProposed = false := by decide

theorem proposed_semantics_can_open_search_frontier :
    frontierPremiseEligible .synthesizedProposed = true := by decide

structure TopologyEvidenceBinding where
  structuralBinding : Nat
  semanticBinding : Nat
  evidenceBinding : Nat
  deriving DecidableEq, Repr

def jointlyAdmittedTopologyEvidence (expected : Nat)
    (binding : TopologyEvidenceBinding) : Bool :=
  binding.structuralBinding == expected &&
    binding.semanticBinding == expected && binding.evidenceBinding == expected

def currentTopologyEvidence : TopologyEvidenceBinding := ⟨1, 1, 1⟩
def staleSemanticTopologyEvidence : TopologyEvidenceBinding := ⟨1, 2, 1⟩

def topologyAssistedFactualDerivation (expected : Nat)
    (binding : TopologyEvidenceBinding) (modality : TopologyModality)
    (liveEvidence : Bool) : Bool :=
  jointlyAdmittedTopologyEvidence expected binding &&
    factualPremiseEligible modality && liveEvidence

theorem declared_topology_and_live_evidence_support_joint_derivation :
    topologyAssistedFactualDerivation 1 currentTopologyEvidence
      .declared true = true := by decide

theorem stale_semantic_topology_blocks_joint_derivation :
    topologyAssistedFactualDerivation 1 staleSemanticTopologyEvidence
      .declared true = false := by decide

theorem proposed_semantics_alone_cannot_enter_factual_derivation :
    topologyAssistedFactualDerivation 1 currentTopologyEvidence
      .synthesizedProposed true = false := by decide

structure TopologyLibraryIdentity where
  structural : Nat
  semantic : Nat
  program : Nat
  deriving DecidableEq, Repr

structure ProjectTopologyLibrary where
  binding : Nat
  identity : TopologyLibraryIdentity
  stableClosure : List Nat
  semanticClaims : List TopologyModality
  deriving DecidableEq, Repr

def admittedTopologyLibrary : ProjectTopologyLibrary :=
  ⟨1, ⟨7, 17, 27⟩, [100, 101], [.declared, .synthesizedAccepted]⟩

def staleTopologyLibrary : ProjectTopologyLibrary :=
  ⟨1, ⟨7, 18, 27⟩, [100, 101], [.declared, .synthesizedAccepted]⟩

def loadTopologyLibrary (expectedBinding : Nat)
    (expectedIdentity : TopologyLibraryIdentity)
    (library : ProjectTopologyLibrary) : Option ProjectTopologyLibrary :=
  if library.binding == expectedBinding && library.identity == expectedIdentity
  then some library else none

def factualTopologyClaims (library : ProjectTopologyLibrary) :
    List TopologyModality :=
  library.semanticClaims.filter factualPremiseEligible

def stageTopologyEvaluationInput (library : ProjectTopologyLibrary)
    (evidenceDelta : List Nat) : List Nat :=
  (library.stableClosure ++ evidenceDelta).eraseDups

theorem exact_topology_library_import_is_admitted :
    loadTopologyLibrary 1 ⟨7, 17, 27⟩ admittedTopologyLibrary =
      some admittedTopologyLibrary := by decide

theorem semantic_identity_drift_rejects_topology_library_import :
    loadTopologyLibrary 1 ⟨7, 17, 27⟩ staleTopologyLibrary = none := by decide

theorem proposed_claims_are_not_loaded_into_factual_closure :
    factualTopologyClaims
      { admittedTopologyLibrary with
        semanticClaims := [.declared, .synthesizedProposed] } =
      [.declared] := by decide

theorem live_evidence_delta_is_staged_without_claiming_new_closure :
    stageTopologyEvaluationInput admittedTopologyLibrary [102] =
      [100, 101, 102] := by decide

/- R17: Ascent must derive knowledge that is absent from the GQL EDB. The
   finite model below computes a bounded transitive closure; production uses a
   least fixed point, but this witness is sufficient to refute conclusion
   initialization as an inference implementation. -/
inductive ReasoningNode where
  | refreshRegistry | publishArtifact | commitReceipt | publicationHeading
  deriving DecidableEq, Repr

abbrev ReachabilityFact := ReasoningNode × ReasoningNode

def composeReachability (known edges : List ReachabilityFact) : List ReachabilityFact :=
  known.flatMap fun left =>
    edges.filterMap fun right =>
      if left.2 == right.1 then some (left.1, right.2) else none

def closureStep (edges known : List ReachabilityFact) : List ReachabilityFact :=
  (known ++ composeReachability known edges).eraseDups

def closureWithin : Nat → List ReachabilityFact → List ReachabilityFact
  | 0, edges => edges.eraseDups
  | fuel + 1, edges => closureStep edges (closureWithin fuel edges)

def reasoningEdges : List ReachabilityFact :=
  [(.refreshRegistry, .publishArtifact), (.publishArtifact, .commitReceipt)]

def documentEdges : List (ReasoningNode × ReasoningNode) :=
  [(.publicationHeading, .publishArtifact)]

def deriveDocumentedPaths (reachable : List ReachabilityFact)
    (documents : List (ReasoningNode × ReasoningNode)) : List ReachabilityFact :=
  (reachable.flatMap fun path =>
    documents.filterMap fun document =>
      if path.2 == document.2 then some (path.1, document.1) else none).eraseDups

theorem recursive_closure_derives_a_non_input_fact :
    (.refreshRegistry, .commitReceipt) ∈ closureWithin 1 reasoningEdges ∧
    (.refreshRegistry, .commitReceipt) ∉ reasoningEdges := by decide

theorem relational_join_derives_a_documented_path :
    (.refreshRegistry, .publicationHeading) ∈
      deriveDocumentedPaths (closureWithin 1 reasoningEdges) documentEdges := by decide

structure PublicationPathFact where
  origin : ReasoningNode
  action : ReasoningNode
  document : ReasoningNode
  terminal : ReasoningNode
  deriving DecidableEq, Repr

def emissionEdges : List (ReasoningNode × ReasoningNode) :=
  [(.publishArtifact, .commitReceipt)]

def derivePublicationPaths
    (calls documents emissions : List (ReasoningNode × ReasoningNode)) :
    List PublicationPathFact :=
  (calls.flatMap fun call =>
    documents.flatMap fun document =>
      emissions.filterMap fun emission =>
        if call.2 == document.2 && call.2 == emission.1 then
          some ⟨call.1, call.2, document.1, emission.2⟩
        else none).eraseDups

def publicationPaths : List PublicationPathFact :=
  derivePublicationPaths
    [(.refreshRegistry, .publishArtifact)] documentEdges emissionEdges

theorem three_relation_join_derives_publication_path :
    ⟨.refreshRegistry, .publishArtifact, .publicationHeading, .commitReceipt⟩ ∈
      publicationPaths := by decide

def queryableReasoningNode : ReasoningNode → Bool
  | .publishArtifact | .commitReceipt => true
  | _ => false

structure MaterializationSet where
  selectors : List ReasoningNode
  deriving DecidableEq, Repr

def publicationMaterializationSet (paths : List PublicationPathFact) :
    MaterializationSet :=
  ⟨(paths.flatMap fun path => [path.action, path.terminal]).filter
      queryableReasoningNode |>.eraseDups⟩

theorem publication_materialization_is_one_query_playbook_set :
    publicationMaterializationSet publicationPaths =
      ⟨[.publishArtifact, .commitReceipt]⟩ := by decide

inductive DerivedOutput where
  | knowledge (fact : ReachabilityFact)
  | frontier (anchor : ReasoningNode) (relationId : Nat)
  deriving DecidableEq, Repr

def admissibleAsPremise : DerivedOutput → Bool
  | .knowledge _ => true
  | .frontier _ _ => false

theorem a_search_frontier_is_a_proposal_not_a_fact :
    admissibleAsPremise (.frontier .publishArtifact 7) = false := by decide

/- R18: a custom relation participating in semi-naive evaluation must expose
   every newly concretized fact through delta. Otherwise a consequence can be
   skipped because the generated program intentionally omits all-total rules. -/
def deltaComplete (previous current delta : List Nat) : Bool :=
  current.all fun fact => previous.contains fact || delta.contains fact

theorem a_new_fact_that_skips_delta_violates_incremental_completeness :
    deltaComplete [1, 2] [1, 2, 3] [] = false := by decide

theorem a_new_fact_exposed_by_delta_satisfies_incremental_completeness :
    deltaComplete [1, 2] [1, 2, 3] [3] = true := by decide

/- R20-R27: Ascent is an internal inference engine. The reusable Project Topology is
the architecture authority, and the Agent receives one request-bound GQL settlement
   whose modalities and proof references distinguish native facts, derived
   conclusions, and model-authored semantic proposals. -/
inductive SettlementModality where
  | parserDirect | declared | derived | proposed
  deriving DecidableEq, Repr

structure SettledRelation where
  fact : ReachabilityFact
  modality : SettlementModality
  programIdentity : Option Nat
  proofIdentity : Option Nat
  deriving DecidableEq, Repr

def settleToGql (direct inferred : List ReachabilityFact)
    (programIdentity proofIdentity : Nat) : List SettledRelation :=
  direct.eraseDups.map (fun fact => ⟨fact, .parserDirect, none, none⟩) ++
    (inferred.filter (fun fact => fact ∉ direct)).eraseDups.map
      (fun fact => ⟨fact, .derived, some programIdentity, some proofIdentity⟩)

def settledPublicationGraph : List SettledRelation :=
  settleToGql reasoningEdges (closureWithin 1 reasoningEdges) 27 42

theorem gql_settlement_preserves_direct_and_marks_novel_derived :
    ⟨(.refreshRegistry, .publishArtifact), .parserDirect, none, none⟩ ∈
      settledPublicationGraph ∧
    ⟨(.refreshRegistry, .commitReceipt), .derived, some 27, some 42⟩ ∈
      settledPublicationGraph := by decide

/- The public Search projection is the closed product of evidence relations,
   topology boundaries, and currently queryable selectors. There is no
   Agent-facing planner or action channel in this type. Query grammar remains
   capability documentation outside the evidence model. -/
structure AgentFacingSearchProjection where
  relations : List SettledRelation
  boundaries : List DerivedOutput
  queryableSelectors : MaterializationSet
  deriving DecidableEq, Repr

def publicationAgentProjection : AgentFacingSearchProjection :=
  ⟨settledPublicationGraph,
    [.frontier .publishArtifact 7],
    publicationMaterializationSet publicationPaths⟩

theorem agent_facing_search_projection_is_fully_determined_by_evidence
    (left right : AgentFacingSearchProjection)
    (relations : left.relations = right.relations)
    (boundaries : left.boundaries = right.boundaries)
    (selectors : left.queryableSelectors = right.queryableSelectors) :
    left = right := by
  cases left
  cases right
  cases relations
  cases boundaries
  cases selectors
  rfl

theorem public_search_projection_has_no_out_of_band_decision_state
    (projection : AgentFacingSearchProjection) :
    projection =
      ⟨projection.relations, projection.boundaries,
        projection.queryableSelectors⟩ := by
  cases projection
  rfl

def defaultAgentProjectionExposesAscentSource : Bool := false

theorem default_agent_projection_is_one_gql_settlement :
    defaultAgentProjectionExposesAscentSource = false := by decide

inductive AnnotationState where
  | proposed | contested | accepted
  deriving DecidableEq, Repr

structure SemanticAnnotation where
  subject : ReasoningNode
  premises : List Nat
  producerIdentity : Nat
  binding : Nat
  state : AnnotationState
  deriving DecidableEq, Repr

def annotationCanProveBehavior (expectedBinding : Nat) : SemanticAnnotation → Bool
  | ⟨_, premises, _, binding, .accepted⟩ =>
      !premises.isEmpty && binding == expectedBinding
  | _ => false

def proposedPublicationSummary : SemanticAnnotation :=
  ⟨.refreshRegistry, [40, 42], 7, 1, .proposed⟩

def acceptedPublicationSummary : SemanticAnnotation :=
  ⟨.refreshRegistry, [40, 42], 7, 1, .accepted⟩

theorem natural_language_proposal_guides_but_does_not_prove :
    proposedPublicationSummary.premises = [40, 42] ∧
    annotationCanProveBehavior 1 proposedPublicationSummary = false := by decide

theorem admitted_natural_language_requires_current_binding :
    annotationCanProveBehavior 1 acceptedPublicationSummary = true ∧
    annotationCanProveBehavior 2 acceptedPublicationSummary = false := by decide

inductive RelationCoverage where
  | certifiedComplete
  | boundedPartial
  | unavailable
  deriving DecidableEq, Repr

inductive RelationKnowledge where
  | known
  | certifiedMissing
  | unresolved
  deriving DecidableEq, Repr

def classifyRelation (coverage : RelationCoverage) (hasWitness : Bool) :
    RelationKnowledge :=
  if hasWitness then .known else
    match coverage with
    | .certifiedComplete => .certifiedMissing
    | .boundedPartial | .unavailable => .unresolved

theorem partial_coverage_cannot_certify_missing :
    classifyRelation .boundedPartial false = .unresolved := by decide

theorem complete_coverage_without_a_witness_certifies_missing :
    classifyRelation .certifiedComplete false = .certifiedMissing := by decide

structure ReachedAt where
  seed : ReasoningNode
  node : ReasoningNode
  depth : Nat
  path : List ReasoningNode
  deriving DecidableEq, Repr

def publicationReachedAt : ReachedAt :=
  ⟨.refreshRegistry, .commitReceipt, 2,
    [.refreshRegistry, .publishArtifact, .commitReceipt]⟩

def reachedAtValid (reached : ReachedAt) : Bool :=
  reached.path.head? == some reached.seed &&
    reached.path.getLast? == some reached.node &&
    reached.depth + 1 == reached.path.length

theorem seed_relative_depth_is_bound_to_its_path :
    reachedAtValid publicationReachedAt = true := by decide

/- Python Graphs proposes a core; certification closes proof dependencies.
   This finite closure is deliberately general rather than the old 1 -> 0
   special case. -/
abbrev ProofDependency := Nat × Nat

def dependencyStep (dependencies : List ProofDependency)
    (selected : List Nat) : List Nat :=
  (selected ++ dependencies.filterMap (fun dependency =>
    if dependency.1 ∈ selected then some dependency.2 else none)).eraseDups

def dependencyClosure : Nat → List ProofDependency → List Nat → List Nat
  | 0, _, selected => selected.eraseDups
  | fuel + 1, dependencies, selected =>
      dependencyClosure fuel dependencies (dependencyStep dependencies selected)

def transitiveProofDependencies : List ProofDependency := [(3, 2), (2, 1)]

theorem decision_core_retains_transitive_proof_dependencies :
    dependencyClosure 2 transitiveProofDependencies [3] = [3, 2, 1] := by decide

/- Cross-version updates need explicit additions and removals. Semi-naive delta
   completeness within one monotone run cannot authorize append-only reuse. -/
structure RelationGenerationDelta where
  fromIdentity : Nat
  toIdentity : Nat
  added : List Nat
  removed : List Nat
  deriving DecidableEq, Repr

def applyGenerationDelta (expectedFrom : Nat) (previous : List Nat)
    (delta : RelationGenerationDelta) : Option (List Nat) :=
  if delta.fromIdentity != expectedFrom ||
      delta.added.any delta.removed.contains then none
  else some ((previous.filter fun fact => fact ∉ delta.removed) ++ delta.added).eraseDups

def removePublishedEdge : RelationGenerationDelta := ⟨1, 2, [], [2]⟩

theorem generation_replacement_retracts_deleted_facts :
    applyGenerationDelta 1 [1, 2] removePublishedEdge = some [1] := by decide

theorem addition_only_delta_check_is_insufficient_for_deletion :
    deltaComplete [1, 2] [1] [] = true := by decide

def fixedPoint (edges known : List ReachabilityFact) : Bool :=
  closureStep edges known == known

theorem fuel_exhaustion_is_not_a_fixed_point_certificate :
    fixedPoint reasoningEdges reasoningEdges = false := by decide

theorem bounded_closure_reaches_the_finite_example_fixed_point :
    fixedPoint reasoningEdges (closureWithin 2 reasoningEdges) = true := by decide

/- Project Topology is a reusable architecture authority. Search consumes one
   exact library generation; it does not own or reconstruct these identities. -/
structure ProjectTopologyIdentity where
  workspace : Nat
  sourceGeneration : Nat
  providerCatalog : Nat
  library : Nat
  topologyGeneration : Nat
  structural : Nat
  semantic : Nat
  inferenceProgram : Nat
  deriving DecidableEq, Repr

structure SearchTopologySettlementIdentity where
  workspace : Nat
  sourceGeneration : Nat
  providerCatalog : Nat
  library : Nat
  topologyGeneration : Nat
  structural : Nat
  semantic : Nat
  inferenceProgram : Nat
  deriving DecidableEq, Repr

def settlementBoundToLibrary (library : ProjectTopologyIdentity)
    (settlement : SearchTopologySettlementIdentity) : Bool :=
  library.workspace == settlement.workspace &&
    library.sourceGeneration == settlement.sourceGeneration &&
    library.providerCatalog == settlement.providerCatalog &&
    library.library == settlement.library &&
    library.topologyGeneration == settlement.topologyGeneration &&
    library.structural == settlement.structural &&
    library.semantic == settlement.semantic &&
    library.inferenceProgram == settlement.inferenceProgram

def projectTopologyIdentity : ProjectTopologyIdentity :=
  ⟨1, 11, 12, 13, 14, 15, 16, 17⟩

def exactSearchTopologyIdentity : SearchTopologySettlementIdentity :=
  ⟨1, 11, 12, 13, 14, 15, 16, 17⟩

def replayedSearchTopologyIdentity : SearchTopologySettlementIdentity :=
  ⟨1, 11, 12, 13, 99, 15, 16, 17⟩

theorem exact_search_projection_binds_the_reusable_topology_library :
    settlementBoundToLibrary projectTopologyIdentity exactSearchTopologyIdentity = true := by decide

theorem cross_generation_search_projection_replay_is_rejected :
    settlementBoundToLibrary projectTopologyIdentity replayedSearchTopologyIdentity = false := by decide

/- Accepted prose needs a separate admission receipt. State=accepted plus a
   matching semantic identity is insufficient by itself. -/
structure SemanticAdmissionReceipt where
  semanticBinding : Nat
  annotationIdentity : Nat
  receiptIdentity : Nat
  deriving DecidableEq, Repr

structure AdmittedSemanticAnnotation where
  annotationIdentity : Nat
  annotation : SemanticAnnotation
  admissionReceipt : Option SemanticAdmissionReceipt
  deriving DecidableEq, Repr

def admittedAnnotationCanProveBehavior (expectedBinding : Nat)
    (candidate : AdmittedSemanticAnnotation) : Bool :=
  annotationCanProveBehavior expectedBinding candidate.annotation &&
    match candidate.admissionReceipt with
    | none => false
    | some receipt =>
        receipt.semanticBinding == expectedBinding &&
          receipt.annotationIdentity == candidate.annotationIdentity &&
          receipt.receiptIdentity != 0

theorem accepted_annotation_without_admission_receipt_is_not_factual :
    admittedAnnotationCanProveBehavior 1
      ⟨7, acceptedPublicationSummary, none⟩ = false := by decide

theorem accepted_annotation_with_receipt_and_exact_binding_is_factual :
    admittedAnnotationCanProveBehavior 1
      ⟨7, acceptedPublicationSummary, some ⟨1, 7, 51⟩⟩ = true := by decide

theorem accepted_annotation_with_foreign_receipt_is_not_factual :
    admittedAnnotationCanProveBehavior 1
      ⟨7, acceptedPublicationSummary, some ⟨2, 8, 51⟩⟩ = false := by decide

/- A from-scratch equivalence digest is an external computation receipt, not a
   second self-declared copy of the candidate library digest. -/
structure TopologyRebuildReceipt where
  sourceIdentity : Nat
  programIdentity : Nat
  topologyGeneration : Nat
  recomputedLibrary : Nat
  receiptIdentity : Nat
  deriving DecidableEq, Repr

def fromScratchEquivalenceAdmitted
    (expectedSource expectedProgram expectedGeneration candidateLibrary : Nat)
    (independentlyAdmitted : List TopologyRebuildReceipt)
    (receipt : Option TopologyRebuildReceipt) : Bool :=
  match receipt with
  | none => false
  | some proof =>
      independentlyAdmitted.contains proof &&
        proof.sourceIdentity == expectedSource &&
        proof.programIdentity == expectedProgram &&
        proof.topologyGeneration == expectedGeneration &&
        proof.recomputedLibrary == candidateLibrary &&
        proof.receiptIdentity != 0

theorem self_declared_digest_equality_is_not_a_rebuild_certificate :
    fromScratchEquivalenceAdmitted 11 17 14 13 [] none = false := by decide

theorem external_rebuild_receipt_binds_generation_and_library :
    fromScratchEquivalenceAdmitted 11 17 14 13 [⟨11, 17, 14, 13, 61⟩]
      (some ⟨11, 17, 14, 13, 61⟩) = true := by decide

theorem stale_rebuild_receipt_is_rejected :
    fromScratchEquivalenceAdmitted 11 17 14 13 [⟨11, 17, 12, 13, 61⟩]
      (some ⟨11, 17, 12, 13, 61⟩) = false := by decide

theorem embedded_nonzero_rebuild_receipt_cannot_authorize_itself :
    fromScratchEquivalenceAdmitted 11 17 14 13 []
      (some ⟨11, 17, 14, 13, 61⟩) = false := by decide

theorem foreign_program_rebuild_receipt_is_rejected :
    fromScratchEquivalenceAdmitted 11 17 14 13 [⟨11, 99, 14, 13, 61⟩]
      (some ⟨11, 99, 14, 13, 61⟩) = false := by decide

theorem admitted_identity_collision_cannot_authorize_forged_rebuild_fields :
    fromScratchEquivalenceAdmitted 11 17 14 13 [⟨90, 91, 92, 93, 61⟩]
      (some ⟨11, 17, 14, 13, 61⟩) = false := by decide

/- The settled graph preserves binding, witness, modality and proof identity.
   A bare relation pair is never sufficient Agent-facing evidence. -/
structure BoundSettledRelation where
  fact : ReachabilityFact
  modality : SettlementModality
  binding : Nat
  witnesses : List Nat
  programIdentity : Option Nat
  proofIdentity : Option Nat
  deriving DecidableEq, Repr

def settleBoundRelation (binding : Nat) (direct inferred : List ReachabilityFact)
    (directWitnesses : List Nat) (programIdentity proofIdentity : Nat) :
    List BoundSettledRelation :=
  direct.eraseDups.map
      (fun fact => ⟨fact, .parserDirect, binding, directWitnesses, none, none⟩) ++
    (inferred.filter (fun fact => fact ∉ direct)).eraseDups.map
      (fun fact =>
        ⟨fact, .derived, binding, directWitnesses,
          some programIdentity, some proofIdentity⟩)

def boundPublicationGraph : List BoundSettledRelation :=
  settleBoundRelation 1 reasoningEdges (closureWithin 1 reasoningEdges) [40, 42] 27 43

theorem bound_gql_settlement_preserves_direct_witnesses :
    ⟨(.refreshRegistry, .publishArtifact), .parserDirect, 1, [40, 42], none, none⟩ ∈
      boundPublicationGraph := by decide

theorem bound_gql_settlement_marks_derived_proof_authority :
    ⟨(.refreshRegistry, .commitReceipt), .derived, 1, [40, 42], some 27, some 43⟩ ∈
      boundPublicationGraph := by decide

/- Deletion invalidation is transitive over proof dependencies. Addition-only
   semi-naive delta completeness cannot establish this property. -/
structure DerivedFactDependency where
  factId : Nat
  premises : List Nat
  deriving DecidableEq, Repr

def invalidationStep (derived : List DerivedFactDependency)
    (invalid : List Nat) : List Nat :=
  (invalid ++ derived.filterMap (fun fact =>
    if fact.premises.any invalid.contains then some fact.factId else none)).eraseDups

def invalidationClosure : Nat → List DerivedFactDependency → List Nat → List Nat
  | 0, _, invalid => invalid.eraseDups
  | fuel + 1, derived, invalid =>
      invalidationClosure fuel derived (invalidationStep derived invalid)

def transitiveDerivedFacts : List DerivedFactDependency :=
  [⟨3, [2]⟩, ⟨4, [3]⟩]

def retainedDerivedFacts (derived : List DerivedFactDependency)
    (invalid : List Nat) : List Nat :=
  derived.filterMap fun fact =>
    if fact.factId ∈ invalid then none else some fact.factId

theorem deleting_a_base_edge_invalidates_all_derived_descendants :
    invalidationClosure 2 transitiveDerivedFacts [2] = [2, 3, 4] ∧
      retainedDerivedFacts transitiveDerivedFacts
        (invalidationClosure 2 transitiveDerivedFacts [2]) = [] := by decide

inductive InferenceTerminationKind where
  | fixedPoint | budgetExhausted | blocked
  deriving DecidableEq, Repr

structure InferenceSettlementReceipt where
  sourceIdentity : Nat
  programIdentity : Nat
  candidate : List ReachabilityFact
  next : List ReachabilityFact
  termination : InferenceTerminationKind
  receiptIdentity : Nat
  deriving DecidableEq, Repr

def inferenceReceiptAdmitted
    (expectedSource expectedProgram : Nat)
    (independentlyAdmitted : List InferenceSettlementReceipt)
    (receipt : InferenceSettlementReceipt) : Bool :=
  independentlyAdmitted.contains receipt &&
    receipt.sourceIdentity == expectedSource &&
    receipt.programIdentity == expectedProgram &&
    receipt.termination == .fixedPoint && receipt.candidate == receipt.next

theorem budget_exhaustion_is_not_a_fixed_point_receipt :
    inferenceReceiptAdmitted 11 17
      [⟨11, 17, reasoningEdges, closureStep reasoningEdges reasoningEdges,
        .budgetExhausted, 71⟩]
      ⟨11, 17, reasoningEdges, closureStep reasoningEdges reasoningEdges,
        .budgetExhausted, 71⟩ = false := by decide

theorem stable_candidate_with_fixed_point_terminal_is_admitted :
    let closure := closureWithin 2 reasoningEdges
    inferenceReceiptAdmitted 11 17
      [⟨11, 17, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩]
      ⟨11, 17, closure, closureStep reasoningEdges closure,
        .fixedPoint, 71⟩ = true := by decide

theorem unadmitted_fixed_point_receipt_is_rejected :
    let closure := closureWithin 2 reasoningEdges
    inferenceReceiptAdmitted 11 17 []
      ⟨11, 17, closure, closureStep reasoningEdges closure,
        .fixedPoint, 71⟩ = false := by decide

theorem foreign_program_fixed_point_receipt_is_rejected :
    let closure := closureWithin 2 reasoningEdges
    inferenceReceiptAdmitted 11 17
      [⟨11, 99, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩]
      ⟨11, 99, closure, closureStep reasoningEdges closure,
        .fixedPoint, 71⟩ = false := by decide

theorem admitted_identity_collision_cannot_authorize_forged_inference_fields :
    let closure := closureWithin 2 reasoningEdges
    inferenceReceiptAdmitted 11 17
      [⟨90, 91, closure, closureStep reasoningEdges closure, .fixedPoint, 71⟩]
      ⟨11, 17, closure, closureStep reasoningEdges closure,
        .fixedPoint, 71⟩ = false := by decide

/- A MaterializationSet is one caller-ordered execution sequence. Duplicate
   selectors are rejected without sorting or otherwise rewriting that order. -/
def materializationSetAdmitted (available selected : List Nat) : Bool :=
  selected.eraseDups == selected && selected.all available.contains

theorem caller_ordered_available_materialization_set_is_admitted :
    materializationSetAdmitted [10, 20, 30] [30, 10] = true := by decide

theorem duplicate_or_unavailable_materialization_is_rejected :
    materializationSetAdmitted [10, 20, 30] [10, 10] = false ∧
      materializationSetAdmitted [10, 20, 30] [10, 40] = false := by decide

/- Query is selector-native and independent of Search. Runtime injects one exact
   execution binding and independently admits every canonical selector. -/
structure ProjectWorkspaceBindingIdentity where
  identity : Nat
  workspaceRoot : Nat
  portability : Nat
  repositoryAliases : List Nat
  deriving DecidableEq, Repr

structure RuntimeExecutionBindingIdentity where
  projectWorkspace : ProjectWorkspaceBindingIdentity
  worktreeInstance : Nat
  publicationNonce : Nat
  contentBinding : Nat
  sourceSnapshot : Nat
  runtimeArtifact : Nat
  providerCatalog : Nat
  evaluatorPolicy : Nat
  activeArtifactReceipt : Nat
  evaluatorAbi : Nat
  deriving DecidableEq, Repr

/- Workspace initialization admits Project Workspace and worktree identity as
   one Host-owned product. Runtime cannot manufacture the worktree component
   from another ClientFrame or filesystem identity. -/
structure HostWorkspaceInitializationBindingIdentity where
  projectWorkspace : ProjectWorkspaceBindingIdentity
  worktreeInstance : Nat
  deriving DecidableEq, Repr

def hostWorkspaceInitializationBindingAdmitted
    (manifest : ProjectWorkspaceBindingIdentity)
    (host candidate : HostWorkspaceInitializationBindingIdentity) : Bool :=
  host.projectWorkspace == manifest &&
    host.worktreeInstance != 0 &&
    candidate == host

def currentHostWorkspaceInitialization : HostWorkspaceInitializationBindingIdentity :=
  ⟨⟨1, 3, 4, [21, 22]⟩, 2⟩

theorem exact_host_workspace_initialization_is_admitted :
    hostWorkspaceInitializationBindingAdmitted
      currentHostWorkspaceInitialization.projectWorkspace
      currentHostWorkspaceInitialization
      currentHostWorkspaceInitialization = true := by decide

theorem workspace_or_session_identity_cannot_replace_host_worktree
    (clientIdentity : Nat)
    (h : clientIdentity ≠ currentHostWorkspaceInitialization.worktreeInstance) :
    hostWorkspaceInitializationBindingAdmitted
      currentHostWorkspaceInitialization.projectWorkspace
      currentHostWorkspaceInitialization
      { currentHostWorkspaceInitialization with worktreeInstance := clientIdentity } = false := by
  have h' : clientIdentity ≠ 2 := by
    simpa [currentHostWorkspaceInitialization] using h
  simp [hostWorkspaceInitializationBindingAdmitted, currentHostWorkspaceInitialization, h']

theorem filesystem_path_hash_cannot_replace_host_worktree
    (pathHash : Nat)
    (h : pathHash ≠ currentHostWorkspaceInitialization.worktreeInstance) :
    hostWorkspaceInitializationBindingAdmitted
      currentHostWorkspaceInitialization.projectWorkspace
      currentHostWorkspaceInitialization
      { currentHostWorkspaceInitialization with worktreeInstance := pathHash } = false := by
  have h' : pathHash ≠ 2 := by
    simpa [currentHostWorkspaceInitialization] using h
  simp [hostWorkspaceInitializationBindingAdmitted, currentHostWorkspaceInitialization, h']

structure RuntimeArtifactBundleBindingIdentity where
  providerRegistration : Nat
  providerArtifactSet : Nat
  evaluatorPolicy : Nat
  evaluatorAbi : Nat
  schemaBundle : Nat
  deriving DecidableEq, Repr

inductive RuntimeArtifactExecutionClosureMemberKind
  | providerRegistration
  | providerArtifactSet
  | evaluatorPolicy
  | evaluatorAbi
  | schemaBundle
  deriving DecidableEq, Repr

structure RuntimeArtifactExecutionClosureMemberIdentity where
  kind : RuntimeArtifactExecutionClosureMemberKind
  digest : Nat
  entryKeys : List Nat
  deriving DecidableEq, Repr

def runtimeArtifactExecutionClosureAdmitted
    (members : List RuntimeArtifactExecutionClosureMemberIdentity) : Bool :=
  match members with
  | [registration, artifacts, policy, abi, schemas] =>
      registration.kind == .providerRegistration &&
      artifacts.kind == .providerArtifactSet &&
      policy.kind == .evaluatorPolicy &&
      abi.kind == .evaluatorAbi &&
      schemas.kind == .schemaBundle &&
      [registration, artifacts, policy, abi, schemas].all
        (fun member => member.digest != 0) &&
      !policy.entryKeys.isEmpty && !abi.entryKeys.isEmpty && !schemas.entryKeys.isEmpty &&
      registration.entryKeys == artifacts.entryKeys
  | _ => false

def completeRuntimeArtifactExecutionClosure :
    List RuntimeArtifactExecutionClosureMemberIdentity :=
  [ ⟨.providerRegistration, 11, [1, 2]⟩
  , ⟨.providerArtifactSet, 12, [1, 2]⟩
  , ⟨.evaluatorPolicy, 13, [3]⟩
  , ⟨.evaluatorAbi, 14, [4]⟩
  , ⟨.schemaBundle, 15, [5, 6]⟩ ]

theorem exact_five_member_runtime_execution_closure_is_admitted :
    runtimeArtifactExecutionClosureAdmitted completeRuntimeArtifactExecutionClosure = true := by
  decide

theorem explicit_empty_provider_pair_is_a_complete_bootstrap_closure :
    runtimeArtifactExecutionClosureAdmitted
      [ ⟨.providerRegistration, 11, []⟩
      , ⟨.providerArtifactSet, 12, []⟩
      , ⟨.evaluatorPolicy, 13, [3]⟩
      , ⟨.evaluatorAbi, 14, [4]⟩
      , ⟨.schemaBundle, 15, [5, 6]⟩ ] = true := by decide

theorem missing_runtime_execution_closure_member_is_rejected :
    runtimeArtifactExecutionClosureAdmitted
      completeRuntimeArtifactExecutionClosure.dropLast = false := by decide

theorem provider_registration_and_artifact_coverage_must_match :
    let drifted :=
      [ ⟨RuntimeArtifactExecutionClosureMemberKind.providerRegistration, 11, [1, 2]⟩
      , ⟨RuntimeArtifactExecutionClosureMemberKind.providerArtifactSet, 12, [1]⟩
      , ⟨RuntimeArtifactExecutionClosureMemberKind.evaluatorPolicy, 13, [3]⟩
      , ⟨RuntimeArtifactExecutionClosureMemberKind.evaluatorAbi, 14, [4]⟩
      , ⟨RuntimeArtifactExecutionClosureMemberKind.schemaBundle, 15, [5, 6]⟩ ]
    runtimeArtifactExecutionClosureAdmitted drifted = false := by decide

theorem activation_generation_cannot_fill_a_missing_closure_member
    (activationGeneration : Nat) :
    runtimeArtifactExecutionClosureAdmitted
      completeRuntimeArtifactExecutionClosure.dropLast = false := by
  cases activationGeneration <;> decide

structure ProviderRegistrationClosureIdentity where
  providerKey : Nat
  canonicalRegistrationDigest : Nat
  deriving DecidableEq, Repr

def providerRegistrationClosureAdmitted
    (canonical : ProviderRegistrationClosureIdentity)
    (candidate : ProviderRegistrationClosureIdentity) : Bool :=
  canonical.providerKey != 0 && canonical == candidate

theorem self_consistent_digest_cannot_forge_provider_registration_authority :
    providerRegistrationClosureAdmitted ⟨1, 11⟩ ⟨1, 12⟩ = false := by decide

/- The resident register is a projection of the bound provider keys, not a
   second mutation authority. A persisted or streamed mutation cannot add,
   remove, or replace one key while retaining the same Runtime product. -/
def boundProviderRegisterProjectionAdmitted
    (closureKeys projectedKeys : List Nat) (mutationEnabled : Bool) : Bool :=
  closureKeys == projectedKeys && !mutationEnabled

theorem exact_read_only_provider_projection_is_admitted :
    boundProviderRegisterProjectionAdmitted [1, 2] [1, 2] false = true := by decide

theorem persisted_provider_cannot_supplement_the_bound_projection :
    boundProviderRegisterProjectionAdmitted [1, 2] [1, 2, 3] false = false := by decide

theorem live_register_mutation_cannot_preserve_bound_bundle_authority :
    boundProviderRegisterProjectionAdmitted [1, 2] [1, 2] true = false := by decide

structure ProviderArtifactClosureIdentity where
  providerKey : Nat
  artifactContentDigest : Nat
  deriving DecidableEq, Repr

def providerArtifactExecutionIdentity
    (entry : ProviderArtifactClosureIdentity) : Nat × Nat :=
  (entry.providerKey, entry.artifactContentDigest)

theorem filesystem_metadata_is_not_a_copy_stable_artifact_identity :
    let entry : ProviderArtifactClosureIdentity := ⟨1, 21⟩
    providerArtifactExecutionIdentity entry = (1, 21) ∧
      ((1, 100) : Nat × Nat) ≠ ((1, 101) : Nat × Nat) := by decide

theorem schema_bundle_cannot_substitute_for_evaluator_abi :
    let baseline : RuntimeArtifactBundleBindingIdentity := ⟨1, 2, 3, 4, 5⟩
    let changedAbi : RuntimeArtifactBundleBindingIdentity := ⟨1, 2, 3, 9, 5⟩
    baseline.schemaBundle = changedAbi.schemaBundle ∧ baseline ≠ changedAbi := by decide

theorem activation_generation_cannot_fill_artifact_bundle_identity
    (activationGeneration : Nat) :
    let baseline : RuntimeArtifactBundleBindingIdentity := ⟨1, 2, 3, 4, 5⟩
    baseline = ⟨1, 2, 3, 4, 5⟩ := by
  cases activationGeneration <;> decide

/- A publication commit is a linearization record, not a synonym for the
   content identity.  Its digest binds the full authority-bearing binding and
   the predecessor fence.  Durability remains an independently admitted writer
   postcondition and is deliberately absent from this self-describing value. -/
structure ContentPublicationCommitIdentity where
  contentBinding : Nat
  predecessor : Option Nat
  commitDigest : Nat
  deriving DecidableEq, Repr

def contentPublicationCommitAdmitted
    (expectedBinding : Nat)
    (expectedPredecessor : Option Nat)
    (expectedCommitDigest : Nat)
    (candidate : ContentPublicationCommitIdentity) : Bool :=
  expectedCommitDigest != 0 &&
    candidate.contentBinding == expectedBinding &&
    candidate.predecessor == expectedPredecessor &&
    candidate.commitDigest == expectedCommitDigest

theorem exact_authority_and_fence_commit_is_admitted :
    contentPublicationCommitAdmitted 11 none 31 ⟨11, none, 31⟩ = true := by decide

theorem equal_content_under_another_authority_is_not_the_same_commit :
    contentPublicationCommitAdmitted 12 none 32 ⟨11, none, 31⟩ = false := by decide

theorem equal_binding_after_another_predecessor_is_not_the_same_commit :
    contentPublicationCommitAdmitted 11 (some 31) 32 ⟨11, none, 31⟩ = false := by decide

theorem segment_durability_digest_cannot_substitute_for_content_commit
    (segmentDurabilityDigest : Nat) :
    contentPublicationCommitAdmitted 11 none 31 ⟨11, none, segmentDurabilityDigest⟩ =
      (segmentDurabilityDigest == 31) := by
  simp [contentPublicationCommitAdmitted]

theorem activation_generation_cannot_authorize_a_different_content_commit
    (activationGeneration : Nat) :
    contentPublicationCommitAdmitted 11 none 32 ⟨11, none, 31⟩ = false := by
  cases activationGeneration <;> decide

structure QueryPlaybookRequest where
  requestIdentity : Nat
  projectWorkspace : Nat
  worktreeInstance : Nat
  runtimeBinding : RuntimeExecutionBindingIdentity
  runtimeBundleDigest : Nat
  executionPublicationDigest : Nat
  selectors : List Nat
  projection : Nat
  deriving DecidableEq, Repr

def queryPlaybookRequestAdmitted
    (expectedBinding : RuntimeExecutionBindingIdentity)
    (expectedRuntimeBundleDigest expectedExecutionPublicationDigest : Nat)
    (runtimeAdmittedSelectors : List Nat)
    (request : QueryPlaybookRequest) : Bool :=
  request.requestIdentity != 0 &&
    request.runtimeBinding == expectedBinding &&
    request.runtimeBundleDigest == expectedRuntimeBundleDigest &&
    request.executionPublicationDigest == expectedExecutionPublicationDigest &&
    request.projectWorkspace == expectedBinding.projectWorkspace.identity &&
    request.worktreeInstance == expectedBinding.worktreeInstance &&
    materializationSetAdmitted runtimeAdmittedSelectors request.selectors

def currentProjectWorkspace : ProjectWorkspaceBindingIdentity :=
  ⟨1, 3, 4, [21, 22]⟩

def currentRuntimeBinding : RuntimeExecutionBindingIdentity :=
  ⟨currentProjectWorkspace, 2, 3, 4, 11, 12, 13, 14, 15, 16⟩

/- The durable source generation and the Runtime execution product are joined by
   one small immutable publication.  Source/index CAS identity is deliberately
   separate from Runtime artifact and evaluator identity: refreshing the latter
   publishes a new sidecar, not a new parser generation.  A canonical pointer
   may admit the candidate only by exact product equality. -/
structure RuntimeWorkspaceExecutionPublicationIdentity where
  workspaceIdentity : Nat
  generationDigest : Nat
  sourceRootDigest : Nat
  contentCommit : ContentPublicationCommitIdentity
  runtimeBinding : RuntimeExecutionBindingIdentity
  runtimeBundleDigest : Nat
  publicationDigest : Nat
  deriving DecidableEq, Repr

def runtimeWorkspaceExecutionPublicationAdmitted
    (expectedWorkspace expectedGeneration expectedSourceRoot : Nat)
    (expectedContentCommit : ContentPublicationCommitIdentity)
    (expectedRuntime : RuntimeExecutionBindingIdentity)
    (expectedRuntimeBundleDigest : Nat)
    (expectedPublicationDigest : Nat)
    (candidate : RuntimeWorkspaceExecutionPublicationIdentity) : Bool :=
  expectedRuntimeBundleDigest != 0 && expectedPublicationDigest != 0 &&
    candidate.workspaceIdentity == expectedWorkspace &&
    candidate.generationDigest == expectedGeneration &&
    candidate.sourceRootDigest == expectedSourceRoot &&
    candidate.contentCommit == expectedContentCommit &&
    candidate.contentCommit.contentBinding == candidate.runtimeBinding.contentBinding &&
    candidate.runtimeBinding == expectedRuntime &&
    candidate.runtimeBundleDigest == expectedRuntimeBundleDigest &&
    candidate.publicationDigest == expectedPublicationDigest

def currentWorkspaceExecutionPublication : RuntimeWorkspaceExecutionPublicationIdentity :=
  ⟨1, 31, 41, ⟨4, none, 31⟩, currentRuntimeBinding, 61, 51⟩

theorem exact_source_and_runtime_product_is_admitted :
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 ⟨4, none, 31⟩ currentRuntimeBinding 61 51
      currentWorkspaceExecutionPublication = true := by decide

theorem runtime_artifact_refresh_does_not_require_a_new_source_generation :
    let refreshedRuntime := { currentRuntimeBinding with runtimeArtifact := 99 }
    let refreshedPublication : RuntimeWorkspaceExecutionPublicationIdentity :=
      ⟨1, 31, 41, ⟨4, none, 31⟩, refreshedRuntime, 62, 52⟩
    refreshedPublication.generationDigest =
        currentWorkspaceExecutionPublication.generationDigest ∧
      refreshedPublication.sourceRootDigest =
        currentWorkspaceExecutionPublication.sourceRootDigest ∧
      refreshedPublication.runtimeBinding ≠
        currentWorkspaceExecutionPublication.runtimeBinding ∧
      runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 ⟨4, none, 31⟩ refreshedRuntime 62 52
        refreshedPublication = true := by decide

theorem stale_runtime_sidecar_is_rejected_for_refreshed_runtime :
    let refreshedRuntime := { currentRuntimeBinding with runtimeArtifact := 99 }
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 ⟨4, none, 31⟩ refreshedRuntime 62 52
      currentWorkspaceExecutionPublication = false := by decide

theorem stale_source_sidecar_is_rejected_for_a_new_source_generation :
    runtimeWorkspaceExecutionPublicationAdmitted 1 32 42 ⟨4, none, 31⟩ currentRuntimeBinding 61 53
      currentWorkspaceExecutionPublication = false := by decide

theorem activation_generation_cannot_substitute_for_publication_identity
    (activationGeneration : Nat) :
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 ⟨4, none, 31⟩ currentRuntimeBinding 61 52
      currentWorkspaceExecutionPublication = false := by
  cases activationGeneration <;> decide

/- Matching inner member digests do not identify the outer Runtime bundle.
   The applied activation and the verified bundle must name the same outer
   content-addressed product before a workspace execution sidecar can exist. -/
theorem equal_inner_runtime_binding_cannot_authorize_a_foreign_outer_bundle :
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 ⟨4, none, 31⟩
      currentRuntimeBinding 62 51 currentWorkspaceExecutionPublication = false := by decide

/- Client-side timing exists before Runtime admission, so it carries only the
   Host-bound session/request correlation and measurements.  The Runtime joins
   it to the independently admitted execution publication.  A client timing
   witness can neither name nor choose the resulting content identity. -/
structure ClientSearchTimingWitness where
  sessionIdentity : Nat
  requestIdentity : Nat
  phaseDurations : List Nat
  deriving DecidableEq, Repr

structure SettledSearchTelemetryIdentity where
  sessionIdentity : Nat
  requestIdentity : Nat
  publication : RuntimeWorkspaceExecutionPublicationIdentity
  deriving DecidableEq, Repr

def settleClientSearchTiming
    (expectedSession expectedRequest : Nat)
    (expectedPublication : RuntimeWorkspaceExecutionPublicationIdentity)
    (witness : ClientSearchTimingWitness) : Option SettledSearchTelemetryIdentity :=
  if witness.sessionIdentity == expectedSession &&
      witness.requestIdentity == expectedRequest &&
      runtimeWorkspaceExecutionPublicationAdmitted
        expectedPublication.workspaceIdentity
        expectedPublication.generationDigest
        expectedPublication.sourceRootDigest
        expectedPublication.contentCommit
        expectedPublication.runtimeBinding
        expectedPublication.runtimeBundleDigest
        expectedPublication.publicationDigest
        expectedPublication
  then some ⟨expectedSession, expectedRequest, expectedPublication⟩
  else none

theorem client_timing_is_enriched_only_with_the_server_admitted_publication
    (durations : List Nat) :
    settleClientSearchTiming 7 8 currentWorkspaceExecutionPublication ⟨7, 8, durations⟩ =
      some ⟨7, 8, currentWorkspaceExecutionPublication⟩ := by
  simp [settleClientSearchTiming, currentWorkspaceExecutionPublication,
    runtimeWorkspaceExecutionPublicationAdmitted, currentRuntimeBinding,
    currentProjectWorkspace]

theorem foreign_client_request_cannot_receive_runtime_identity :
    settleClientSearchTiming 7 8 currentWorkspaceExecutionPublication ⟨7, 9, [1, 2, 3]⟩ =
      none := by decide

theorem timing_values_cannot_select_a_different_runtime_publication
    (left right : List Nat) :
    (settleClientSearchTiming 7 8 currentWorkspaceExecutionPublication ⟨7, 8, left⟩).map
        SettledSearchTelemetryIdentity.publication =
      (settleClientSearchTiming 7 8 currentWorkspaceExecutionPublication ⟨7, 8, right⟩).map
        SettledSearchTelemetryIdentity.publication := by
  simp [settleClientSearchTiming, currentWorkspaceExecutionPublication,
    runtimeWorkspaceExecutionPublicationAdmitted, currentRuntimeBinding,
    currentProjectWorkspace]

/- The ten-phase trace crosses seven execution authorities.  The phase owner is
   part of the contract: a convenient downstream layer cannot manufacture a
   measurement that belongs to an upstream or provider-owned boundary. -/
inductive SearchTelemetryPhaseOwner where
  | client
  | admission
  | runtimeRoute
  | searchExecution
  | searchProjection
  | responseService
  | transportWriter
  deriving DecidableEq, Repr

inductive SearchTelemetryPhase where
  | launcher
  | clientFrameEncode
  | ipcConnect
  | serverAdmissionQueue
  | snapshotResolve
  | providerDispatch
  | parseIndexQuery
  | projectionRank
  | schemaValidateSerialize
  | terminalEgress
  deriving DecidableEq, Repr

def searchTelemetryPhaseOwner : SearchTelemetryPhase → SearchTelemetryPhaseOwner
  | .launcher | .clientFrameEncode | .ipcConnect => .client
  | .serverAdmissionQueue => .admission
  | .snapshotResolve | .providerDispatch => .runtimeRoute
  | .parseIndexQuery => .searchExecution
  | .projectionRank => .searchProjection
  | .schemaValidateSerialize => .responseService
  | .terminalEgress => .transportWriter

def searchTelemetryPhaseOrdinal : SearchTelemetryPhase → Nat
  | .launcher => 0
  | .clientFrameEncode => 1
  | .ipcConnect => 2
  | .serverAdmissionQueue => 3
  | .snapshotResolve => 4
  | .providerDispatch => 5
  | .parseIndexQuery => 6
  | .projectionRank => 7
  | .schemaValidateSerialize => 8
  | .terminalEgress => 9

def phaseMayBeRecordedBy
    (owner : SearchTelemetryPhaseOwner) (phase : SearchTelemetryPhase) : Bool :=
  searchTelemetryPhaseOwner phase == owner

theorem runtime_route_cannot_synthesize_search_execution_measurement :
    phaseMayBeRecordedBy .runtimeRoute .parseIndexQuery = false := by decide

theorem search_execution_cannot_claim_projection_rank :
    phaseMayBeRecordedBy .searchExecution .projectionRank = false := by decide

theorem search_projection_cannot_claim_response_serialization :
    phaseMayBeRecordedBy .searchProjection .schemaValidateSerialize = false := by decide

theorem only_transport_writer_can_record_terminal_egress
    (owner : SearchTelemetryPhaseOwner) :
    phaseMayBeRecordedBy owner .terminalEgress = true → owner = .transportWriter := by
  cases owner <;> simp [phaseMayBeRecordedBy, searchTelemetryPhaseOwner]

theorem canonical_search_telemetry_ordinals_are_strict :
    [SearchTelemetryPhase.launcher, .clientFrameEncode, .ipcConnect,
      .serverAdmissionQueue, .snapshotResolve, .providerDispatch,
      .parseIndexQuery, .projectionRank, .schemaValidateSerialize,
      .terminalEgress].map searchTelemetryPhaseOrdinal =
      [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] := by decide

/- An owner tag is not measurement authority. The trace admits the complete
   boundary receipt only when that exact record was independently admitted by
   the execution boundary. Retaining a receipt identity while changing the
   request, phase, owner, or elapsed measurement cannot authorize the mutation. -/
structure SearchTelemetryAuthorityReceipt where
  requestIdentity : Nat
  phase : SearchTelemetryPhase
  owner : SearchTelemetryPhaseOwner
  elapsedMicros : Nat
  boundaryIdentity : Nat
  deriving DecidableEq, Repr

def phaseAuthorityReceiptAdmitted
    (expectedRequest : Nat)
    (independentlyAdmitted : List SearchTelemetryAuthorityReceipt)
    (receipt : SearchTelemetryAuthorityReceipt) : Bool :=
  independentlyAdmitted.contains receipt &&
    receipt.requestIdentity == expectedRequest &&
    phaseMayBeRecordedBy receipt.owner receipt.phase &&
    receipt.boundaryIdentity != 0

def providerParseReceipt : SearchTelemetryAuthorityReceipt :=
  ⟨41, .parseIndexQuery, .searchExecution, 19, 73⟩

theorem caller_supplied_provider_tag_is_not_measurement_authority :
    phaseAuthorityReceiptAdmitted 41 [] providerParseReceipt = false := by decide

theorem independently_admitted_provider_measurement_is_accepted :
    phaseAuthorityReceiptAdmitted 41 [providerParseReceipt] providerParseReceipt = true := by decide

theorem mutated_elapsed_measurement_cannot_reuse_an_admitted_receipt_identity :
    phaseAuthorityReceiptAdmitted 41 [providerParseReceipt]
      { providerParseReceipt with elapsedMicros := 20 } = false := by decide

theorem foreign_request_cannot_replay_an_admitted_provider_measurement :
    phaseAuthorityReceiptAdmitted 42 [providerParseReceipt] providerParseReceipt = false := by decide

theorem predecessor_fence_changes_workspace_execution_publication :
    let successorCommit : ContentPublicationCommitIdentity := ⟨4, some 31, 32⟩
    let successorPublication : RuntimeWorkspaceExecutionPublicationIdentity :=
      ⟨1, 31, 41, successorCommit, currentRuntimeBinding, 61, 52⟩
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 successorCommit
      currentRuntimeBinding 61 52 successorPublication = true ∧
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 successorCommit
      currentRuntimeBinding 61 52 currentWorkspaceExecutionPublication = false := by decide

theorem content_commit_and_runtime_binding_must_share_one_authority :
    let foreignCommit : ContentPublicationCommitIdentity := ⟨12, none, 32⟩
    runtimeWorkspaceExecutionPublicationAdmitted 1 31 41 foreignCommit
      currentRuntimeBinding 61 52
      ⟨1, 31, 41, foreignCommit, currentRuntimeBinding, 61, 52⟩ = false := by decide

structure RuntimeWorkspaceExecutionPointerIdentity where
  workspaceIdentity : Nat
  generationDigest : Nat
  sourceRootDigest : Nat
  executionPublicationDigest : Nat
  deriving DecidableEq, Repr

def runtimeWorkspaceExecutionPointerAdmitted
    (publication : RuntimeWorkspaceExecutionPublicationIdentity)
    (pointer : RuntimeWorkspaceExecutionPointerIdentity) : Bool :=
  runtimeWorkspaceExecutionPublicationAdmitted
      pointer.workspaceIdentity pointer.generationDigest pointer.sourceRootDigest
      publication.contentCommit publication.runtimeBinding
      publication.runtimeBundleDigest
      pointer.executionPublicationDigest publication &&
    pointer.workspaceIdentity == publication.workspaceIdentity &&
    pointer.generationDigest == publication.generationDigest &&
    pointer.sourceRootDigest == publication.sourceRootDigest &&
    pointer.executionPublicationDigest == publication.publicationDigest

def currentWorkspaceExecutionPointer : RuntimeWorkspaceExecutionPointerIdentity :=
  ⟨1, 31, 41, 51⟩

theorem canonical_pointer_binds_both_durable_halves :
    runtimeWorkspaceExecutionPointerAdmitted currentWorkspaceExecutionPublication
      currentWorkspaceExecutionPointer = true := by decide

theorem pointer_to_stale_sidecar_is_not_partially_ready :
    let refreshedRuntime := { currentRuntimeBinding with runtimeArtifact := 99 }
    let refreshedPublication : RuntimeWorkspaceExecutionPublicationIdentity :=
      ⟨1, 31, 41, ⟨4, none, 31⟩, refreshedRuntime, 62, 52⟩
    runtimeWorkspaceExecutionPointerAdmitted refreshedPublication
      currentWorkspaceExecutionPointer = false := by decide

theorem source_pointer_drift_rejects_an_otherwise_valid_sidecar :
    runtimeWorkspaceExecutionPointerAdmitted currentWorkspaceExecutionPublication
      ⟨1, 32, 42, 51⟩ = false := by decide

theorem smallest_runtime_admitted_selector_subset_is_queryable :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, currentRuntimeBinding, 61, 51, [10], 7⟩ = true := by decide

theorem selectors_learned_across_searches_may_form_one_query :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, currentRuntimeBinding, 61, 51, [30, 10], 7⟩ = true := by decide

theorem runtime_binding_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, ⟨currentProjectWorkspace, 2, 3, 4, 11, 99, 13, 14, 15, 16⟩, 61, 51,
        [10], 7⟩ = false := by decide

theorem publication_nonce_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, { currentRuntimeBinding with publicationNonce := 99 }, 61, 51, [10], 7⟩ = false := by
  decide

theorem content_binding_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, { currentRuntimeBinding with contentBinding := 99 }, 61, 51, [10], 7⟩ = false := by
  decide

theorem evaluator_policy_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, { currentRuntimeBinding with evaluatorPolicy := 99 }, 61, 51, [10], 7⟩ = false := by
  decide

theorem active_artifact_receipt_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, { currentRuntimeBinding with activeArtifactReceipt := 99 }, 61, 51, [10], 7⟩ = false := by
  decide

theorem evaluator_abi_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, { currentRuntimeBinding with evaluatorAbi := 99 }, 61, 51, [10], 7⟩ = false := by
  decide

theorem workspace_root_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, ⟨⟨1, 99, 4, [21, 22]⟩, 2, 3, 4, 11, 12, 13, 14, 15, 16⟩,
        61, 51, [10], 7⟩ = false := by decide

theorem equal_request_and_runtime_cannot_self_authorize_a_foreign_workspace :
    let foreignWorkspace : ProjectWorkspaceBindingIdentity := ⟨9, 3, 4, [21, 22]⟩
    let foreignRuntime : RuntimeExecutionBindingIdentity :=
      ⟨foreignWorkspace, 2, 3, 4, 11, 12, 13, 14, 15, 16⟩
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 9, 2, foreignRuntime, 61, 51, [10], 7⟩ = false := by decide

theorem repository_alias_drift_is_rejected_before_materialization :
    let aliasDrift : ProjectWorkspaceBindingIdentity := ⟨1, 3, 4, [21, 99]⟩
    let driftedRuntime : RuntimeExecutionBindingIdentity :=
      ⟨aliasDrift, 2, 3, 4, 11, 12, 13, 14, 15, 16⟩
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, driftedRuntime, 61, 51, [10], 7⟩ = false := by decide

theorem worktree_context_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 99, currentRuntimeBinding, 61, 51, [10], 7⟩ = false := by decide

theorem outer_runtime_bundle_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, currentRuntimeBinding, 62, 51, [10], 7⟩ = false := by decide

theorem execution_publication_drift_is_rejected_before_materialization :
    queryPlaybookRequestAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      ⟨101, 1, 2, currentRuntimeBinding, 61, 52, [10], 7⟩ = false := by decide

inductive QueryPlaybookTerminalState where
  | ready | failed
  deriving DecidableEq, Repr

structure QueryPlaybookReceipt where
  requestIdentity : Nat
  projectWorkspace : Nat
  worktreeInstance : Nat
  runtimeBinding : RuntimeExecutionBindingIdentity
  runtimeBundleDigest : Nat
  executionPublicationDigest : Nat
  requestedSelectors : List Nat
  materializedSelectors : List Nat
  projection : Nat
  terminal : QueryPlaybookTerminalState
  terminalCount : Nat
  deriving DecidableEq, Repr

def queryPlaybookReceiptAdmitted
    (expectedBinding : RuntimeExecutionBindingIdentity)
    (expectedRuntimeBundleDigest expectedExecutionPublicationDigest : Nat)
    (runtimeAdmittedSelectors : List Nat)
    (request : QueryPlaybookRequest)
    (receipt : QueryPlaybookReceipt) : Bool :=
  queryPlaybookRequestAdmitted expectedBinding
      expectedRuntimeBundleDigest expectedExecutionPublicationDigest
      runtimeAdmittedSelectors request &&
    receipt.requestIdentity == request.requestIdentity &&
    receipt.projectWorkspace == request.projectWorkspace &&
    receipt.worktreeInstance == request.worktreeInstance &&
    receipt.runtimeBinding == request.runtimeBinding &&
    receipt.runtimeBundleDigest == request.runtimeBundleDigest &&
    receipt.executionPublicationDigest == request.executionPublicationDigest &&
    receipt.requestedSelectors == request.selectors &&
    receipt.projection == request.projection &&
    receipt.terminalCount == 1 &&
    match receipt.terminal with
    | .ready => receipt.materializedSelectors == request.selectors
    | .failed => receipt.materializedSelectors.isEmpty

def twoSelectorQuery : QueryPlaybookRequest :=
  ⟨101, 1, 2, currentRuntimeBinding, 61, 51, [30, 10], 7⟩

def completeQueryReceipt : QueryPlaybookReceipt :=
  ⟨101, 1, 2, currentRuntimeBinding, 61, 51, [30, 10], [30, 10], 7, .ready, 1⟩

theorem one_runtime_bound_terminal_preserves_the_complete_request_order :
    queryPlaybookReceiptAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      twoSelectorQuery completeQueryReceipt = true := by decide

theorem a_partial_ready_receipt_is_rejected :
    queryPlaybookReceiptAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      twoSelectorQuery
      { completeQueryReceipt with materializedSelectors := [10] } = false := by decide

theorem a_failed_receipt_cannot_expose_partial_materialization :
    queryPlaybookReceiptAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      twoSelectorQuery
      { { completeQueryReceipt with terminal := .failed } with
          materializedSelectors := [10] } = false := by decide

theorem completion_order_cannot_replace_request_order :
    queryPlaybookReceiptAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      twoSelectorQuery
      { completeQueryReceipt with materializedSelectors := [10, 30] } = false := by decide

theorem more_than_one_query_terminal_is_rejected :
    queryPlaybookReceiptAdmitted currentRuntimeBinding 61 51 [10, 20, 30]
      twoSelectorQuery
      { completeQueryReceipt with terminalCount := 2 } = false := by decide

end ASPProof.SearchEvidenceDerivation
