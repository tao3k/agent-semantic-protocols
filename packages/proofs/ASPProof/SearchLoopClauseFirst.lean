-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

/-!
# SearchLoop Clause-first model

This is the first executable RFC model for:

* ASP-RFC-10.05-CFR-GOD-DECISION
* ASP-RFC-10.05-BECA-STATES
* ASP-RFC-10.05-CFR-GOD-GLOBAL-BUDGET
* ASP-RFC-10.05-RPGE-CLOSED-WORLD
* ASP-RFC-10.05-CFR-GOD-GRAPH-AUTHORITY

The model deliberately contains finite counterexamples to naive formulations
and corrected definitions that the RFC can require.
-/

namespace SearchLoopClauseFirst

abbrev ClauseId := Nat
abbrev ObligationId := Nat
abbrev SnapshotId := Nat
abbrev ProviderId := Nat
abbrev SchemaId := Nat
abbrev PolicyId := Nat
abbrev SelectorId := Nat
abbrev FactId := Nat
abbrev EntityId := Nat

structure ValidationDomain where
  snapshot : SnapshotId
  provider : ProviderId
  schema : SchemaId
  policy : PolicyId
deriving DecidableEq, Repr

inductive ArgRole where
  | caller
  | callee
  | callsite
  | guard
  | owner
  | selector
  | custom (id : Nat)
deriving DecidableEq, Repr

structure Posting where
  fact : FactId
  role : ArgRole
  entity : EntityId
  domain : ValidationDomain
deriving DecidableEq, Repr

def sameLogicalFact (left right : Posting) : Prop :=
  left.fact = right.fact ∧ left.domain = right.domain

/- CFR-R01/CFR-R14: endpoint overlap cannot establish fact identity. -/
def callerPosting : Posting where
  fact := 1
  role := .caller
  entity := 7
  domain := ⟨1, 1, 1, 1⟩

def guardPostingFromAnotherFact : Posting where
  fact := 2
  role := .guard
  entity := 7
  domain := ⟨1, 1, 1, 1⟩

theorem shared_endpoint_does_not_imply_same_fact :
    callerPosting.entity = guardPostingFromAnotherFact.entity ∧
    ¬ sameLogicalFact callerPosting guardPostingFromAnotherFact := by
  simp [callerPosting, guardPostingFromAnotherFact, sameLogicalFact]

inductive ClauseState where
  | resolved
  | searchable
  | fuzzy
  | stalled
deriving DecidableEq, Repr

structure Clause where
  id : ClauseId
  obligation : ObligationId
  domain : ValidationDomain
  hasSufficiencyWitness : Bool
  hasPostingAction : Bool
  hasDiscoveryAction : Bool
deriving DecidableEq, Repr

def exampleDomain : ValidationDomain := ⟨1, 1, 1, 1⟩

def overlappingNaiveClause : Clause where
  id := 1
  obligation := 1
  domain := exampleDomain
  hasSufficiencyWitness := true
  hasPostingAction := true
  hasDiscoveryAction := true

def naiveResolved (clause : Clause) : Prop :=
  clause.hasSufficiencyWitness = true

def naiveSearchable (clause : Clause) : Prop :=
  clause.hasPostingAction = true

/- CFR-RFC repair: independent predicates overlap and therefore are not a
   state machine. -/
theorem naive_clause_predicates_can_overlap :
    naiveResolved overlappingNaiveClause ∧
    naiveSearchable overlappingNaiveClause := by
  simp [naiveResolved, naiveSearchable, overlappingNaiveClause]

/- ASP-RFC-10.05-BECA-STATES: priority makes classification total and
   deterministic. -/
def classify (clause : Clause) : ClauseState :=
  if clause.hasSufficiencyWitness then
    .resolved
  else if clause.hasPostingAction then
    .searchable
  else if clause.hasDiscoveryAction then
    .fuzzy
  else
    .stalled

theorem sufficiency_has_classification_precedence
    (clause : Clause)
    (witness : clause.hasSufficiencyWitness = true) :
    classify clause = .resolved := by
  simp [classify, witness]

theorem posting_action_precedes_discovery
    (clause : Clause)
    (noWitness : clause.hasSufficiencyWitness = false)
    (posting : clause.hasPostingAction = true) :
    classify clause = .searchable := by
  simp [classify, noWitness, posting]

theorem discovery_without_posting_is_fuzzy
    (clause : Clause)
    (noWitness : clause.hasSufficiencyWitness = false)
    (noPosting : clause.hasPostingAction = false)
    (discovery : clause.hasDiscoveryAction = true) :
    classify clause = .fuzzy := by
  simp [classify, noWitness, noPosting, discovery]

theorem no_admitted_action_is_stalled
    (clause : Clause)
    (noWitness : clause.hasSufficiencyWitness = false)
    (noPosting : clause.hasPostingAction = false)
    (noDiscovery : clause.hasDiscoveryAction = false) :
    classify clause = .stalled := by
  simp [classify, noWitness, noPosting, noDiscovery]

inductive BudgetDisposition where
  | schedulable
  | needsBudget
deriving DecidableEq, Repr

def fairnessPossible (budget mandatoryCount : Nat) : Prop :=
  mandatoryCount ≤ budget

def budgetDisposition (budget mandatoryCount : Nat) : BudgetDisposition :=
  if mandatoryCount ≤ budget then .schedulable else .needsBudget

/- CFR-R06: the naive unconditional positive-floor requirement is false. -/
theorem zero_budget_cannot_cover_one_mandatory :
    ¬ fairnessPossible 0 1 := by
  simp [fairnessPossible]

theorem insufficient_budget_is_typed
    (budget mandatoryCount : Nat)
    (insufficient : budget < mandatoryCount) :
    budgetDisposition budget mandatoryCount = .needsBudget := by
  simp [budgetDisposition, Nat.not_le_of_gt insufficient]

def floorAllocation
    (budget mandatoryCount obligationIndex : Nat) : Nat :=
  if obligationIndex < mandatoryCount ∧ obligationIndex < budget then 1 else 0

theorem sufficient_budget_gives_positive_floor
    (budget mandatoryCount obligationIndex : Nat)
    (covered : mandatoryCount ≤ budget)
    (mandatory : obligationIndex < mandatoryCount) :
    floorAllocation budget mandatoryCount obligationIndex = 1 := by
  have withinBudget : obligationIndex < budget :=
    Nat.lt_of_lt_of_le mandatory covered
  simp [floorAllocation, mandatory, withinBudget]

structure SearchActionKey where
  clause : ClauseId
  snapshot : SnapshotId
  provider : ProviderId
  schema : SchemaId
  policy : PolicyId
  bindingDigest : Nat
  queryPackDigest : Nat
deriving DecidableEq, Repr

theorem snapshot_drift_changes_action_identity
    (clause binding queryPack provider schema policy : Nat)
    (oldSnapshot newSnapshot : SnapshotId)
    (drift : oldSnapshot ≠ newSnapshot) :
    (⟨clause, oldSnapshot, provider, schema, policy, binding, queryPack⟩ :
      SearchActionKey) ≠
    ⟨clause, newSnapshot, provider, schema, policy, binding, queryPack⟩ := by
  intro same
  exact drift (congrArg SearchActionKey.snapshot same)

structure LookupResult where
  hitCount : Nat
  completeScope : Bool
  fresh : Bool
deriving DecidableEq, Repr

def provesAbsence (result : LookupResult) : Prop :=
  result.hitCount = 0 ∧
  result.completeScope = true ∧
  result.fresh = true

def incompleteNoHit : LookupResult := ⟨0, false, true⟩
def staleCompleteNoHit : LookupResult := ⟨0, true, false⟩
def freshCompleteNoHit : LookupResult := ⟨0, true, true⟩

theorem no_hit_without_scope_does_not_prove_absence :
    ¬ provesAbsence incompleteNoHit := by
  simp [provesAbsence, incompleteNoHit]

theorem stale_no_hit_does_not_prove_absence :
    ¬ provesAbsence staleCompleteNoHit := by
  simp [provesAbsence, staleCompleteNoHit]

theorem fresh_complete_no_hit_proves_absence :
    provesAbsence freshCompleteNoHit := by
  simp [provesAbsence, freshCompleteNoHit]

structure ClosureEvidence where
  allRequiredDischarged : Bool
  contradictionFree : Bool
  fresh : Bool
  exactMaterializationSatisfied : Bool
deriving DecidableEq, Repr

def closed (evidence : ClosureEvidence) : Prop :=
  evidence.allRequiredDischarged = true ∧
  evidence.contradictionFree = true ∧
  evidence.fresh = true ∧
  evidence.exactMaterializationSatisfied = true

def highRankWithoutClosureEvidence : ClosureEvidence :=
  ⟨false, true, true, false⟩

/- Rank does not occur in the closure proposition. -/
theorem high_rank_cannot_replace_closure_evidence
    (_rank : Nat) :
    ¬ closed highRankWithoutClosureEvidence := by
  simp [closed, highRankWithoutClosureEvidence]

structure MaterializationRequest where
  boundSelector : SelectorId
  emittedSelector : SelectorId
deriving DecidableEq, Repr

def preservesSelector (request : MaterializationRequest) : Prop :=
  request.boundSelector = request.emittedSelector

theorem selector_drift_blocks_materialization
    (bound emitted : SelectorId)
    (drift : bound ≠ emitted) :
    ¬ preservesSelector ⟨bound, emitted⟩ := by
  simpa [preservesSelector] using drift

structure EscalationInput where
  enumeration : Bool
  absence : Bool
  crossNamespaceJoin : Bool
  contradiction : Bool
  unresolvedVariables : Nat
  localVariableLimit : Nat
  hopDepth : Nat
  localHopLimit : Nat
deriving DecidableEq, Repr

def escalationRequired (input : EscalationInput) : Bool :=
  input.enumeration ||
  input.absence ||
  input.crossNamespaceJoin ||
  input.contradiction ||
  decide (input.localVariableLimit < input.unresolvedVariables) ||
  decide (input.localHopLimit < input.hopDepth)

theorem enumeration_always_escalates
    (input : EscalationInput)
    (enumeration : input.enumeration = true) :
    escalationRequired input = true := by
  simp [escalationRequired, enumeration]

theorem absence_always_escalates
    (input : EscalationInput)
    (absence : input.absence = true) :
    escalationRequired input = true := by
  simp [escalationRequired, absence]

theorem cross_namespace_join_always_escalates
    (input : EscalationInput)
    (crosses : input.crossNamespaceJoin = true) :
    escalationRequired input = true := by
  simp [escalationRequired, crosses]

structure RankedEscalationView where
  input : EscalationInput
  rank : Nat
deriving DecidableEq, Repr

def mustEscalate (view : RankedEscalationView) : Bool :=
  escalationRequired view.input

theorem rank_cannot_disable_mandatory_escalation
    (input : EscalationInput)
    (leftRank rightRank : Nat) :
    mustEscalate ⟨input, leftRank⟩ =
    mustEscalate ⟨input, rightRank⟩ := by
  rfl

structure CacheKey where
  clauseDigest : Nat
  domain : ValidationDomain
  bindingDigest : Nat
  queryPackDigest : Nat
deriving DecidableEq, Repr

theorem provider_drift_changes_cache_identity
    (clause binding queryPack snapshot schema policy : Nat)
    (oldProvider newProvider : ProviderId)
    (drift : oldProvider ≠ newProvider) :
    (⟨clause, ⟨snapshot, oldProvider, schema, policy⟩, binding, queryPack⟩ :
      CacheKey) ≠
    ⟨clause, ⟨snapshot, newProvider, schema, policy⟩, binding, queryPack⟩ := by
  intro same
  exact drift (congrArg (fun key => key.domain.provider) same)

end SearchLoopClauseFirst
