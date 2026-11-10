-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheReplicaRejoin

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery

inductive RejoinPhase where
  | notStarted
  | stateInstalled
  | tokenIssued
  | membershipPublished
  | readsEnabled
deriving DecidableEq, Repr

def RejoinPhase.rank : RejoinPhase → Nat
  | .notStarted => 0
  | .stateInstalled => 1
  | .tokenIssued => 2
  | .membershipPublished => 3
  | .readsEnabled => 4

structure RejoinRecoveryState where
  phase : RejoinPhase
  tokenIssueCount : Nat
  readsEnabled : Bool
deriving DecidableEq, Repr

def ExpectedTokenIssueCount (phase : RejoinPhase) : Nat :=
  if RejoinPhase.rank phase < 2 then 0 else 1

def RecoveryStateWellFormed (state : RejoinRecoveryState) : Prop :=
  state.tokenIssueCount = ExpectedTokenIssueCount state.phase
    ∧ (state.readsEnabled = true ↔ state.phase = RejoinPhase.readsEnabled)

instance recoveryStateWellFormedDecidable
    (state : RejoinRecoveryState) :
    Decidable (RecoveryStateWellFormed state) := by
  unfold RecoveryStateWellFormed
  infer_instance

/--
Only the state-installed to token-issued edge creates a token. Resuming any
later phase reuses the durable token receipt.
-/
def resumeRejoin (state : RejoinRecoveryState) : RejoinRecoveryState :=
  match state.phase with
  | .notStarted =>
      { state with phase := .stateInstalled }
  | .stateInstalled =>
      { state with
        phase := .tokenIssued
        tokenIssueCount := state.tokenIssueCount + 1 }
  | .tokenIssued =>
      { state with phase := .membershipPublished }
  | .membershipPublished =>
      { state with
        phase := .readsEnabled
        readsEnabled := true }
  | .readsEnabled =>
      state

theorem resume_rejoin_never_decreases_phase
    (state : RejoinRecoveryState) :
    RejoinPhase.rank state.phase ≤
      RejoinPhase.rank (resumeRejoin state).phase := by
  rcases state with ⟨phase, count, reads⟩
  cases phase <;> simp [resumeRejoin, RejoinPhase.rank]

theorem resume_preserves_well_formed_rejoin_state
    (state : RejoinRecoveryState)
    (wellFormed : RecoveryStateWellFormed state) :
    RecoveryStateWellFormed (resumeRejoin state) := by
  rcases state with ⟨phase, count, reads⟩
  cases phase <;>
    simp
      [RecoveryStateWellFormed,
        ExpectedTokenIssueCount,
        RejoinPhase.rank,
        resumeRejoin] at wellFormed ⊢ <;>
    simp_all

def initialRecoveryState : RejoinRecoveryState :=
  { phase := .notStarted
    tokenIssueCount := 0
    readsEnabled := false }

def afterStateInstall : RejoinRecoveryState :=
  resumeRejoin initialRecoveryState

def afterTokenIssue : RejoinRecoveryState :=
  resumeRejoin afterStateInstall

def afterMembershipPublication : RejoinRecoveryState :=
  resumeRejoin afterTokenIssue

def afterReadsEnabled : RejoinRecoveryState :=
  resumeRejoin afterMembershipPublication

theorem complete_recovery_issues_one_token_and_enables_reads_last :
    RecoveryStateWellFormed initialRecoveryState
      ∧ RecoveryStateWellFormed afterStateInstall
      ∧ RecoveryStateWellFormed afterTokenIssue
      ∧ RecoveryStateWellFormed afterMembershipPublication
      ∧ RecoveryStateWellFormed afterReadsEnabled
      ∧ afterTokenIssue.tokenIssueCount = 1
      ∧ afterMembershipPublication.tokenIssueCount = 1
      ∧ afterReadsEnabled.tokenIssueCount = 1
      ∧ afterStateInstall.readsEnabled = false
      ∧ afterTokenIssue.readsEnabled = false
      ∧ afterMembershipPublication.readsEnabled = false
      ∧ afterReadsEnabled.readsEnabled = true := by
  decide

def prematureReadsState : RejoinRecoveryState :=
  { phase := .tokenIssued
    tokenIssueCount := 1
    readsEnabled := true }

theorem enabling_reads_from_partial_rejoin_is_not_well_formed :
    prematureReadsState.tokenIssueCount = 1
      ∧ prematureReadsState.readsEnabled = true
      ∧ ¬ RecoveryStateWellFormed prematureReadsState := by
  decide

def unsafeReissueTokenAfterCrash
    (state : RejoinRecoveryState) : RejoinRecoveryState :=
  { state with tokenIssueCount := state.tokenIssueCount + 1 }

theorem reissuing_token_after_token_phase_breaks_exactly_once_recovery :
    RecoveryStateWellFormed afterTokenIssue
      ∧ (unsafeReissueTokenAfterCrash afterTokenIssue).tokenIssueCount = 2
      ∧ ¬ RecoveryStateWellFormed
        (unsafeReissueTokenAfterCrash afterTokenIssue) := by
  decide

end ASPProof.SearchRouteAdmissionRetryCacheRejoinRecovery
