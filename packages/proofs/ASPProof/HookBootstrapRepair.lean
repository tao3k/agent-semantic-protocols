-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookBootstrapRepair

inductive EventPlane where
  | enforcement
  | observational
  deriving DecidableEq, Repr

inductive Decision where
  | allow
  | deny
  deriving DecidableEq, Repr

inductive ArtifactState where
  | coherent
  | drifted
  | repairing
  | published
  deriving DecidableEq, Repr

inductive HookSurface where
  | aspHook
  | internalBootstrap
  deriving DecidableEq, Repr

def publicSurface : HookSurface := .aspHook

structure PublishedPair where
  binaryGeneration : Nat
  configGeneration : Nat
  deriving DecidableEq, Repr

def PairCoherent (pair : PublishedPair) : Prop :=
  pair.binaryGeneration = pair.configGeneration

def publishPair (generation : Nat) : PublishedPair :=
  { binaryGeneration := generation, configGeneration := generation }

def canParseFullConfig : ArtifactState → Bool
  | .coherent | .published => true
  | .drifted | .repairing => false

def claimRepair (candidate : Nat) : Option Nat → Option Nat
  | none => some candidate
  | some owner => some owner

def replayCount : ArtifactState → Nat
  | .coherent => 0
  | .drifted => 0
  | .repairing => 0
  | .published => 1

def ReplayBounded (state : ArtifactState) : Prop :=
  replayCount state = 0 ∨ replayCount state = 1

def failureDecision : EventPlane → Decision
  | .enforcement => .deny
  | .observational => .allow

structure WarmCost where
  hostProcessStarts : Nat
  aspChildProcessStarts : Nat
  serverGenerationAdmissions : Nat
  deriving DecidableEq, Repr

def warmCost : WarmCost :=
  { hostProcessStarts := 1
    aspChildProcessStarts := 0
    serverGenerationAdmissions := 0 }

def serverGenerationBuilds (_warmEvents : Nat) : Nat := 0

inductive RecoveryAction where
  | doctor
  | canonicalBinaryInstall
  | arbitraryCommand
  | chainedCommand
  deriving DecidableEq, Repr

def recoveryAdmitted : RecoveryAction → Bool
  | .doctor | .canonicalBinaryInstall => true
  | .arbitraryCommand | .chainedCommand => false

def managedAutoSync (canonical ownershipProven : Bool) : Bool :=
  canonical && ownershipProven

def installGeneration (generation : Nat) : PublishedPair :=
  publishPair generation

theorem publish_pair_is_coherent (generation : Nat) :
    PairCoherent (publishPair generation) := by
  rfl

theorem drift_cannot_parse_full_config :
    canParseFullConfig .drifted = false := by
  rfl

theorem public_hook_surface_is_stable : publicSurface = .aspHook := by
  rfl

theorem existing_repair_owner_is_preserved (candidate owner : Nat) :
    claimRepair candidate (some owner) = some owner := by
  rfl

theorem repair_replays_at_most_once (state : ArtifactState) :
    ReplayBounded state := by
  cases state with
  | coherent => exact Or.inl rfl
  | drifted => exact Or.inl rfl
  | repairing => exact Or.inl rfl
  | published => exact Or.inr rfl

theorem enforcement_failure_is_denied :
    failureDecision .enforcement = .deny := by
  rfl

theorem observational_failure_is_allowed :
    failureDecision .observational = .allow := by
  rfl

theorem warm_path_has_no_second_process :
    warmCost.hostProcessStarts = 1 ∧ warmCost.aspChildProcessStarts = 0 := by
  exact ⟨rfl, rfl⟩

theorem server_warm_generation_is_not_per_hook (warmEvents : Nat) :
    serverGenerationBuilds warmEvents = 0 := by
  rfl

theorem drift_preserves_configuration_independent_repair_edge :
    canParseFullConfig .drifted = false ∧
      recoveryAdmitted .canonicalBinaryInstall = true ∧
      recoveryAdmitted .chainedCommand = false := by
  decide

theorem automatic_sync_requires_canonical_managed_ownership :
    managedAutoSync true true = true ∧
      managedAutoSync true false = false ∧
      managedAutoSync false true = false := by
  decide

theorem canonical_install_republishes_a_coherent_pair (generation : Nat) :
    PairCoherent (installGeneration generation) := by
  exact publish_pair_is_coherent generation

end ASPProof.HookBootstrapRepair
