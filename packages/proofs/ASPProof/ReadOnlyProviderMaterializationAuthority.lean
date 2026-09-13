-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ReadOnlyProviderMaterializationAuthority

inductive Authority where
  | readOnlySearch
  | runtimeWriter
  deriving DecidableEq

inductive WorkspaceState where
  | missing
  | building
  | ready
  deriving DecidableEq

inductive Effect where
  | readPublishedGeneration
  | requestReadiness
  | materializeProvider
  | publishGeneration
  | writeCheckout
  deriving DecidableEq

def permits : Authority → Effect → Prop
  | .readOnlySearch, .readPublishedGeneration => True
  | .readOnlySearch, .requestReadiness => True
  | .runtimeWriter, .requestReadiness => True
  | .runtimeWriter, .materializeProvider => True
  | .runtimeWriter, .publishGeneration => True
  | _, _ => False

def advances : Authority → WorkspaceState → WorkspaceState → Prop
  | .runtimeWriter, .missing, .building => True
  | .runtimeWriter, .building, .ready => True
  | _, sourceState, targetState => sourceState = targetState

theorem read_only_search_cannot_materialize_provider :
    ¬ permits .readOnlySearch .materializeProvider := by
  simp [permits]

theorem read_only_search_cannot_write_checkout :
    ¬ permits .readOnlySearch .writeCheckout := by
  simp [permits]

theorem only_runtime_writer_admits_missing_workspace
    (authority : Authority)
    (h : advances authority .missing .building) :
    authority = .runtimeWriter := by
  cases authority <;> simp [advances] at h ⊢

theorem ready_publication_requires_runtime_writer
    (authority : Authority)
    (h : advances authority .building .ready) :
    authority = .runtimeWriter := by
  cases authority <;> simp [advances] at h ⊢

inductive GenerationBase where
  | missing
  | admitted
  deriving DecidableEq

inductive MutationBuildMode where
  | restoreOrBuild
  | rebuildAfterMutation
  deriving DecidableEq

def mutationBuildMode : GenerationBase → MutationBuildMode
  | .missing => .restoreOrBuild
  | .admitted => .rebuildAfterMutation

theorem first_observed_mutation_is_not_incremental :
    mutationBuildMode .missing ≠ .rebuildAfterMutation := by
  decide

theorem incremental_mutation_requires_admitted_base
    (base : GenerationBase)
    (h : mutationBuildMode base = .rebuildAfterMutation) :
    base = .admitted := by
  cases base <;> simp [mutationBuildMode] at h ⊢

inductive ResidentState where
  | absent
  | locatorSlot
  | generationReady
  deriving DecidableEq

inductive ResidentEffect where
  | restoreLocator
  | scanOwners
  | invokeProvider
  deriving DecidableEq

def residentTransition : ResidentState → ResidentEffect → ResidentState → Prop
  | .absent, .restoreLocator, .locatorSlot => True
  | .locatorSlot, .scanOwners, .generationReady => True
  | .locatorSlot, .invokeProvider, .generationReady => True
  | _, _, _ => False

theorem locator_restore_does_not_scan_owners :
    ¬ residentTransition .absent .scanOwners .locatorSlot := by
  simp [residentTransition]

theorem locator_restore_does_not_invoke_provider :
    ¬ residentTransition .absent .invokeProvider .locatorSlot := by
  simp [residentTransition]

theorem cold_read_can_restore_slot_without_materialization :
    residentTransition .absent .restoreLocator .locatorSlot := by
  simp [residentTransition]

end ASPProof.ReadOnlyProviderMaterializationAuthority
