-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSearchEvidenceClosure

inductive PredicateKind where
  | regexTruth
  | rankedProse
  | structuralFact
  | ownerMembership
  | knownSelector
  deriving DecidableEq

inductive PrimaryRoute where
  | rg
  | tantivy
  | syntax
  | topology
  | query
  deriving DecidableEq

def primaryRoute : PredicateKind → PrimaryRoute
  | .regexTruth => .rg
  | .rankedProse => .tantivy
  | .structuralFact => .syntax
  | .ownerMembership => .topology
  | .knownSelector => .query

theorem every_predicate_has_one_primary_route (predicate : PredicateKind) :
    ∃ route, primaryRoute predicate = route ∧
      ∀ candidate, primaryRoute predicate = candidate → candidate = route := by
  exact ⟨primaryRoute predicate, rfl, by
    intro candidate equal
    exact equal.symm⟩

structure EvidenceClosure where
  aspAvailable : Bool
  workspaceOwned : Bool
  coverageComplete : Bool
  deriving DecidableEq

def shellFallbackAuthorized (closure : EvidenceClosure) : Prop :=
  closure.aspAvailable = false ∨
    closure.workspaceOwned = false ∨
    closure.coverageComplete = false

instance (closure : EvidenceClosure) : Decidable (shellFallbackAuthorized closure) := by
  unfold shellFallbackAuthorized
  infer_instance

def completeWorkspaceEvidence : EvidenceClosure where
  aspAvailable := true
  workspaceOwned := true
  coverageComplete := true

theorem complete_workspace_evidence_forbids_shell_fallback :
    ¬ shellFallbackAuthorized completeWorkspaceEvidence := by
  decide

def unavailableAspEvidence : EvidenceClosure where
  aspAvailable := false
  workspaceOwned := true
  coverageComplete := false

theorem typed_unavailability_authorizes_external_recovery :
    shellFallbackAuthorized unavailableAspEvidence := by
  decide

structure SelectorHandoff where
  searchSelector : String
  querySelector : String
  sameGeneration : Bool
  deriving DecidableEq

def validHandoff (handoff : SelectorHandoff) : Prop :=
  handoff.sameGeneration = true ∧
    handoff.querySelector = handoff.searchSelector

instance (handoff : SelectorHandoff) : Decidable (validHandoff handoff) := by
  unfold validHandoff
  infer_instance

theorem valid_handoff_preserves_selector_identity
    (handoff : SelectorHandoff)
    (valid : validHandoff handoff) :
    handoff.querySelector = handoff.searchSelector := by
  exact valid.2

def reconstructedSelector : SelectorHandoff where
  searchSelector := "rust://src/lib.rs#item/function/run"
  querySelector := "rust://src/lib.rs:42"
  sameGeneration := true

theorem display_locator_reconstruction_is_not_a_valid_handoff :
    ¬ validHandoff reconstructedSelector := by
  decide

inductive SelectorEndpointKind where
  | ownerRoot
  | parserItem
  deriving DecidableEq

def requiresItemFragment : SelectorEndpointKind → Bool
  | .ownerRoot => false
  | .parserItem => true

theorem owner_root_does_not_require_a_parser_item_fragment :
    requiresItemFragment .ownerRoot = false := by
  rfl

def ownerRootHandoff : SelectorHandoff where
  searchSelector := "rust://crates/topology/src/lib.rs"
  querySelector := "rust://crates/topology/src/lib.rs"
  sameGeneration := true

theorem owner_root_is_a_valid_unchanged_query_handoff :
    validHandoff ownerRootHandoff := by
  decide

end ASPProof.AgentSearchEvidenceClosure
