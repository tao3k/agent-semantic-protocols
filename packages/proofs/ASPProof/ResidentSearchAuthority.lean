-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ResidentSearchAuthority

inductive SearchState
  | activeGenerationRequired
  | ready
  | capabilityUnavailable
  | providerContractMismatch
  | generationCorrupt
  | cancelled
  | authorityClosed
  deriving DecidableEq

def MayReturnMiss : SearchState → Prop
  | .ready => True
  | _ => False

theorem non_ready_state_cannot_be_miss
    (state : SearchState)
    (notReady : state ≠ .ready) :
    ¬ MayReturnMiss state := by
  cases state <;> simp_all [MayReturnMiss]

structure PublishedGeneration where
  epoch : Nat
  generationDigest : String
  capabilityDigest : String
  deriving DecidableEq

def MonotonicPublication
    (current candidate : PublishedGeneration) : Prop :=
  current.epoch < candidate.epoch ∨ current = candidate

theorem older_epoch_is_not_publishable
    (current candidate : PublishedGeneration)
    (older : candidate.epoch < current.epoch) :
    ¬ MonotonicPublication current candidate := by
  intro publication
  rcases publication with newer | identical
  · omega
  · subst candidate
    omega

theorem reused_epoch_requires_identical_evidence
    (current candidate : PublishedGeneration)
    (sameEpoch : current.epoch = candidate.epoch)
    (publication : MonotonicPublication current candidate) :
    current = candidate := by
  rcases publication with newer | identical
  · omega
  · exact identical

inductive ReadPlaneAction
  | cloneCommittedGeneration
  | readResidentSection
  deriving DecidableEq

def SendsWriterCommand : ReadPlaneAction → Prop
  | .cloneCommittedGeneration => False
  | .readResidentSection => False

theorem resident_read_never_sends_writer_command
    (action : ReadPlaneAction) :
    ¬ SendsWriterCommand action := by
  cases action <;> simp [SendsWriterCommand]

inductive ReadOutcome
  | immediateTerminal
  | residentRead
  deriving DecidableEq

def outcomeFor : SearchState → ReadOutcome
  | .ready => .residentRead
  | _ => .immediateTerminal

theorem non_ready_state_is_immediate_terminal
    (state : SearchState)
    (notReady : state ≠ .ready) :
    outcomeFor state = .immediateTerminal := by
  cases state <;> simp_all [outcomeFor]

inductive ReadPlaneEffect
  | residentMmapRead
  | filesystemWalk
  | databaseOpen
  | providerInvoke
  | generationAdmission
  deriving DecidableEq

def MayPerformReadPlaneEffect : SearchState → ReadPlaneEffect → Prop
  | .ready, .residentMmapRead => True
  | _, _ => False

theorem non_ready_state_performs_no_read_plane_effect
    (state : SearchState)
    (notReady : state ≠ .ready)
    (effect : ReadPlaneEffect) :
    ¬ MayPerformReadPlaneEffect state effect := by
  cases state <;> cases effect <;> simp_all [MayPerformReadPlaneEffect]

theorem ready_state_excludes_preparation_effects
    (effect : ReadPlaneEffect)
    (preparation : effect ≠ .residentMmapRead) :
    ¬ MayPerformReadPlaneEffect .ready effect := by
  cases effect <;> simp_all [MayPerformReadPlaneEffect]

end ASPProof.ResidentSearchAuthority
