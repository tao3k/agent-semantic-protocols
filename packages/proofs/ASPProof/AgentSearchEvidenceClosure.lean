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

inductive SourceAcquisition where
  | filesystem
  | derivedOverlay
  deriving DecidableEq

structure SourceSnapshotIdentity where
  schema : String
  algorithm : String
  root : String
  leafCount : Nat
  providerScope : String
  acquisition : SourceAcquisition
  deriving DecidableEq

def sameContentIdentity
    (left right : SourceSnapshotIdentity) : Prop :=
  left.schema = right.schema ∧
    left.algorithm = right.algorithm ∧
    left.root = right.root ∧
    left.leafCount = right.leafCount ∧
    left.providerScope = right.providerScope

instance (left right : SourceSnapshotIdentity) :
    Decidable (sameContentIdentity left right) := by
  unfold sameContentIdentity
  infer_instance

def hookOverlaySnapshot : SourceSnapshotIdentity where
  schema := "asp.source-snapshot.v1"
  algorithm := "blake3-merkle-v1"
  root := "canonical-root"
  leafCount := 3
  providerScope := "provider-scope"
  acquisition := .derivedOverlay

def processColdFilesystemSnapshot : SourceSnapshotIdentity where
  schema := "asp.source-snapshot.v1"
  algorithm := "blake3-merkle-v1"
  root := "canonical-root"
  leafCount := 3
  providerScope := "provider-scope"
  acquisition := .filesystem

theorem equal_content_with_distinct_provenance_is_reusable :
    sameContentIdentity hookOverlaySnapshot processColdFilesystemSnapshot ∧
      hookOverlaySnapshot ≠ processColdFilesystemSnapshot := by
  decide

def topologyOwnerMembershipSelectorHydrations : Nat := 0

theorem topology_owner_membership_stays_on_the_locator_plane :
    topologyOwnerMembershipSelectorHydrations = 0 := by
  rfl

end ASPProof.AgentSearchEvidenceClosure
