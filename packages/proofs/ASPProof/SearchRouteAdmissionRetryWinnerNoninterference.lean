-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryAuthorizationStamp

namespace ASPProof.SearchRouteAdmissionRetryWinnerNoninterference

universe u

inductive OutcomeClass where
  | denied
  | missing
  | released
deriving DecidableEq, Repr

/--
The timing field is a protocol-visible timing class, not physical duration.
The model does not claim machine-level constant-time execution.
-/
structure ProtocolObservation (Payload : Type u) where
  outcome : OutcomeClass
  payload : Option Payload
  cacheMutation : Bool
  timingClass : Nat
deriving DecidableEq, Repr

abbrev WinnerCell (Payload : Type u) :=
  Option Payload

def UnauthorizedNoninterference
    {Payload : Type u}
    (observe : WinnerCell Payload → ProtocolObservation Payload) : Prop :=
  ∀ left right, observe left = observe right

def constantDeniedObservation
    {Payload : Type u}
    (_cell : WinnerCell Payload) :
    ProtocolObservation Payload :=
  { outcome := OutcomeClass.denied
    payload := none
    cacheMutation := false
    timingClass := 0 }

theorem constant_denial_is_independent_of_winner_occupancy
    {Payload : Type u} :
    UnauthorizedNoninterference
      (constantDeniedObservation :
        WinnerCell Payload → ProtocolObservation Payload) := by
  intro left right
  rfl

def outcomeLeakingObservation
    (cell : WinnerCell Bool) :
    ProtocolObservation Bool :=
  { outcome :=
      match cell with
      | none => OutcomeClass.missing
      | some _winner => OutcomeClass.denied
    payload := none
    cacheMutation := false
    timingClass := 0 }

theorem winner_existence_leaks_through_outcome_class :
    outcomeLeakingObservation none ≠
      outcomeLeakingObservation (some true) := by
  decide

def payloadLeakingObservation
    (cell : WinnerCell Bool) :
    ProtocolObservation Bool :=
  { outcome := OutcomeClass.denied
    payload := cell
    cacheMutation := false
    timingClass := 0 }

theorem winner_existence_leaks_through_denied_payload :
    payloadLeakingObservation none ≠
      payloadLeakingObservation (some true) := by
  decide

def cacheLeakingObservation
    (cell : WinnerCell Bool) :
    ProtocolObservation Bool :=
  { outcome := OutcomeClass.denied
    payload := none
    cacheMutation := cell.isSome
    timingClass := 0 }

theorem winner_existence_leaks_through_cache_mutation :
    cacheLeakingObservation none ≠
      cacheLeakingObservation (some true) := by
  decide

def timingLeakingObservation
    (cell : WinnerCell Bool) :
    ProtocolObservation Bool :=
  { outcome := OutcomeClass.denied
    payload := none
    cacheMutation := false
    timingClass :=
      match cell with
      | none => 0
      | some _winner => 1 }

theorem winner_existence_leaks_through_protocol_timing_class :
    timingLeakingObservation none ≠
      timingLeakingObservation (some true) := by
  decide

end ASPProof.SearchRouteAdmissionRetryWinnerNoninterference
