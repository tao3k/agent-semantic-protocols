-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeClientServingPublication

/-- V1 keeps client bootstrap and resident serving as two explicit authorities. -/
structure RuntimeAuthority where
  clientGeneration : Nat
  servingGeneration : Nat
  pendingGeneration : Option Nat
  ownerAlive : Bool
  deriving DecidableEq

/-- The retired single-switch design leaves the client on the dead applied
owner while the only recovery implementation is in a newer pending binary. -/
def legacyBootstrapDeadlock (state : RuntimeAuthority) (newerPending : Nat) : Prop :=
  state.ownerAlive = false ∧
    state.clientGeneration = state.servingGeneration ∧
    state.servingGeneration < newerPending

theorem dead_applied_owner_and_newer_pending_form_legacy_bootstrap_deadlock :
    legacyBootstrapDeadlock
      { clientGeneration := 21
        servingGeneration := 21
        pendingGeneration := some 28
        ownerAlive := false }
      28 := by
  simp [legacyBootstrapDeadlock]

/-- One Artifacts mutation publishes the event-bound client candidate while
resident serving remains previous until the actor commits ready/applied. -/
def publishPending (state : RuntimeAuthority) (generation : Nat) : RuntimeAuthority :=
  { state with
      clientGeneration := generation
      pendingGeneration := some generation }

/-- Candidate validation and canonical client-alias replacement are one
guarded publication result.  Any partial failure exposes the previous state. -/
def publishAttempt (state : RuntimeAuthority) (generation : Nat)
    (candidateReady aliasReady : Bool) : RuntimeAuthority :=
  if candidateReady && aliasReady then publishPending state generation else state

theorem candidate_failure_preserves_previous_authorities
    (state : RuntimeAuthority) (generation : Nat) (aliasReady : Bool) :
    publishAttempt state generation false aliasReady = state := by
  simp [publishAttempt]

theorem alias_failure_preserves_previous_authorities
    (state : RuntimeAuthority) (generation : Nat) (candidateReady : Bool) :
    publishAttempt state generation candidateReady false = state := by
  cases candidateReady <;> simp [publishAttempt]

theorem publication_exposes_no_torn_authority
    (state : RuntimeAuthority) (generation : Nat)
    (candidateReady aliasReady : Bool) :
    publishAttempt state generation candidateReady aliasReady = state ∨
      publishAttempt state generation candidateReady aliasReady =
        publishPending state generation := by
  cases candidateReady <;> cases aliasReady <;> simp [publishAttempt]

theorem dual_authority_publication_is_atomic
    (state : RuntimeAuthority) (generation : Nat) :
    let visible := publishPending state generation
    visible.clientGeneration = generation ∧
      visible.pendingGeneration = some generation ∧
      visible.servingGeneration = state.servingGeneration := by
  simp [publishPending]

theorem dual_authority_publication_is_monotone
    (state : RuntimeAuthority) (generation : Nat)
    (newer : state.clientGeneration < generation) :
    (publishPending state generation).clientGeneration > state.clientGeneration := by
  simpa [publishPending] using newer

/-- A client already bound to pending does not depend on the dead applied owner
to execute stale-endpoint/no-owner recovery. -/
def clientCanExecuteRecovery (state : RuntimeAuthority) (recoveryGeneration : Nat) : Bool :=
  state.clientGeneration == recoveryGeneration

theorem pending_client_breaks_the_legacy_cycle
    (state : RuntimeAuthority) (generation : Nat) :
    clientCanExecuteRecovery (publishPending state generation) generation = true := by
  simp [clientCanExecuteRecovery, publishPending]

/-- Actor commit is admitted only for the currently pending generation. -/
def actorCommit (state : RuntimeAuthority) (generation : Nat) : RuntimeAuthority :=
  if state.pendingGeneration = some generation ∧
      state.servingGeneration ≤ generation then
    { state with
        servingGeneration := generation
        pendingGeneration := none
        ownerAlive := true }
  else
    state

theorem actor_commit_converges_current_pending
    (state : RuntimeAuthority) (generation : Nat)
    (pending : state.pendingGeneration = some generation)
    (monotone : state.servingGeneration ≤ generation) :
    let committed := actorCommit state generation
    committed.clientGeneration = state.clientGeneration ∧
      committed.servingGeneration = generation ∧
      committed.pendingGeneration = none ∧
      committed.ownerAlive = true := by
  simp [actorCommit, pending, monotone]

theorem recovery_then_current_actor_commit_converges_without_cycle
    (state : RuntimeAuthority) (generation : Nat)
    (monotone : state.servingGeneration ≤ generation) :
    let published := publishPending state generation
    clientCanExecuteRecovery published generation = true ∧
      (actorCommit published generation).clientGeneration = generation ∧
      (actorCommit published generation).servingGeneration = generation ∧
      (actorCommit published generation).pendingGeneration = none ∧
      (actorCommit published generation).ownerAlive = true := by
  simp [clientCanExecuteRecovery, publishPending, actorCommit, monotone]

theorem stale_actor_commit_preserves_newer_publication
    (state : RuntimeAuthority) (older newer : Nat)
    (pending : state.pendingGeneration = some newer)
    (distinct : older ≠ newer) :
    actorCommit state older = state := by
  have notCurrent : state.pendingGeneration ≠ some older := by
    intro current
    rw [pending] at current
    exact distinct (Option.some.inj current).symm
  simp [actorCommit, notCurrent]

/-- Both serialization orders are monotone: publication after commit keeps the
newer client pending; commit after publication converges only that generation. -/
theorem concurrent_publication_and_commit_do_not_roll_back
    (state : RuntimeAuthority) (applied pending : Nat)
    (newer : applied < pending) :
    (publishPending (actorCommit state applied) pending).clientGeneration = pending ∧
      (actorCommit (publishPending state pending) applied) = publishPending state pending := by
  constructor
  · rfl
  · exact stale_actor_commit_preserves_newer_publication
      (publishPending state pending) applied pending rfl (Nat.ne_of_lt newer)

end ASPProof.RuntimeClientServingPublication
