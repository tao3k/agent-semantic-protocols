-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ParserReadAuthorityLoadOnce

structure LoadState where
  loaderTicket : Option Nat
  deriving DecidableEq

structure AcquireResult where
  state : LoadState
  performedLoad : Bool
  deriving DecidableEq

def acquire (state : LoadState) (ticket : Nat) : AcquireResult :=
  match state.loaderTicket with
  | none => {
      state := { loaderTicket := some ticket }
      performedLoad := true
    }
  | some _ => {
      state := state
      performedLoad := false
    }

def cancel (state : LoadState) (ticket : Nat) : LoadState :=
  if state.loaderTicket = some ticket then
    { loaderTicket := none }
  else
    state

def mappedBytes (result : AcquireResult) (bytes : Nat) : Nat :=
  if result.performedLoad then bytes else 0

theorem first_reader_performs_the_load (ticket : Nat) :
    (acquire { loaderTicket := none } ticket).performedLoad = true := by
  rfl

theorem resident_reader_never_performs_the_load
    (owner ticket : Nat) :
    (acquire { loaderTicket := some owner } ticket).performedLoad = false := by
  rfl

theorem first_reader_publishes_its_ticket (ticket : Nat) :
    (acquire { loaderTicket := none } ticket).state.loaderTicket = some ticket := by
  rfl

theorem second_reader_preserves_the_first_ticket
    (first second : Nat) :
    (acquire (acquire { loaderTicket := none } first).state second).state.loaderTicket =
      some first := by
  rfl

theorem two_readers_have_exactly_one_loader
    (first second : Nat) :
    (acquire { loaderTicket := none } first).performedLoad = true ∧
      (acquire (acquire { loaderTicket := none } first).state second).performedLoad = false := by
  constructor <;> rfl

theorem only_loader_accounts_mapped_bytes
    (first second bytes : Nat) :
    mappedBytes (acquire { loaderTicket := none } first) bytes +
        mappedBytes (acquire (acquire { loaderTicket := none } first).state second) bytes =
      bytes := by
  simp [mappedBytes, acquire]

theorem loader_cancellation_releases_the_cell (ticket : Nat) :
    (cancel { loaderTicket := some ticket } ticket).loaderTicket = none := by
  simp [cancel]

theorem non_owner_cancellation_cannot_release_the_cell
    (owner ticket : Nat)
    (different : owner ≠ ticket) :
    cancel { loaderTicket := some owner } ticket = { loaderTicket := some owner } := by
  simp [cancel, different]

theorem retry_after_loader_cancellation_can_load
    (ticket retryTicket : Nat) :
    (acquire (cancel { loaderTicket := some ticket } ticket) retryTicket).performedLoad = true := by
  simp [cancel, acquire]

end ASPProof.ParserReadAuthorityLoadOnce
