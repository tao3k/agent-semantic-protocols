-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.LiveCorpusPredicateRoute

/-!
Live Corpus V1 chooses one primary acquisition route from the predicate.
Composition is optional: an intersection is admitted only when both operands
are complete sets and the user intent is an explicit conjunction.
-/

inductive PrimaryRoute where
  | regexTruth
  | rankedText
  | structuralSyntax
  | exactQuery
  deriving DecidableEq

structure SearchIntent where
  primary : PrimaryRoute
  deriving DecidableEq

def RequiresPartnerEngine (_intent : SearchIntent) : Bool := false

theorem primary_route_never_requires_a_partner (intent : SearchIntent) :
    RequiresPartnerEngine intent = false := by
  rfl

theorem every_intent_has_one_primary_route (intent : SearchIntent) :
    ∃ route, intent.primary = route := by
  exact ⟨intent.primary, rfl⟩

structure SetEvidence where
  ownerCount : Nat
  complete : Bool
  deriving DecidableEq

def AdmitIntersection
    (explicitConjunction : Bool)
    (left right : SetEvidence) : Bool :=
  explicitConjunction && left.complete && right.complete

theorem conjunction_requires_both_complete_witnesses
    (explicitConjunction : Bool)
    (left right : SetEvidence)
    (admitted : AdmitIntersection explicitConjunction left right = true) :
    explicitConjunction = true ∧ left.complete = true ∧ right.complete = true := by
  simp [AdmitIntersection] at admitted
  exact ⟨admitted.1.1, admitted.1.2, admitted.2⟩

theorem overlapping_engines_without_explicit_conjunction_are_rejected
    (left right : SetEvidence) :
    AdmitIntersection false left right = false := by
  rfl

theorem truncated_left_input_cannot_prove_intersection
    (right : SetEvidence) :
    AdmitIntersection true { ownerCount := 0, complete := false } right = false := by
  rfl

theorem truncated_right_input_cannot_prove_intersection
    (left : SetEvidence) :
    AdmitIntersection true left { ownerCount := 0, complete := false } = false := by
  simp [AdmitIntersection]

end ASPProof.LiveCorpusPredicateRoute
