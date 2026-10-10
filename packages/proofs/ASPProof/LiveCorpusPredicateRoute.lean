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
  generation : String
  ownerUniverse : String
  resultCount : Nat
  complete : Bool
  truncated : Bool
  deriving DecidableEq

def AdmitIntersection
    (explicitConjunction : Bool)
    (left right : SetEvidence) : Prop :=
  explicitConjunction = true ∧
    left.complete = true ∧ right.complete = true ∧
    left.truncated = false ∧ right.truncated = false ∧
    left.generation = right.generation ∧
    left.ownerUniverse = right.ownerUniverse

theorem conjunction_requires_both_complete_witnesses
    (explicitConjunction : Bool)
    (left right : SetEvidence)
    (admitted : AdmitIntersection explicitConjunction left right) :
  explicitConjunction = true ∧
      left.complete = true ∧ right.complete = true ∧
      left.truncated = false ∧ right.truncated = false := by
  exact ⟨admitted.1, admitted.2.1, admitted.2.2.1,
    admitted.2.2.2.1, admitted.2.2.2.2.1⟩

theorem conjunction_requires_one_generation
    (left right : SetEvidence)
    (admitted : AdmitIntersection true left right) :
    left.generation = right.generation := by
  exact admitted.2.2.2.2.2.1

theorem conjunction_requires_one_owner_universe
    (left right : SetEvidence)
    (admitted : AdmitIntersection true left right) :
    left.ownerUniverse = right.ownerUniverse := by
  exact admitted.2.2.2.2.2.2

theorem overlapping_engines_without_explicit_conjunction_are_rejected
    (left right : SetEvidence) :
    ¬ AdmitIntersection false left right := by
  intro admitted
  exact Bool.noConfusion admitted.1

theorem truncated_left_input_cannot_prove_intersection
    (right : SetEvidence) :
    ¬ AdmitIntersection true {
      generation := right.generation
      ownerUniverse := right.ownerUniverse
      resultCount := 0
      complete := true
      truncated := true
    } right := by
  intro admitted
  exact Bool.noConfusion admitted.2.2.2.1

theorem truncated_right_input_cannot_prove_intersection
    (left : SetEvidence) :
    ¬ AdmitIntersection true left {
      generation := left.generation
      ownerUniverse := left.ownerUniverse
      resultCount := 0
      complete := true
      truncated := true
    } := by
  intro admitted
  exact Bool.noConfusion admitted.2.2.2.2.1

theorem different_generations_cannot_prove_intersection
    (left right : SetEvidence)
    (generationDrift : left.generation ≠ right.generation) :
    ¬ AdmitIntersection true left right := by
  intro admitted
  exact generationDrift admitted.2.2.2.2.2.1

theorem different_owner_universes_cannot_prove_intersection
    (left right : SetEvidence)
    (universeDrift : left.ownerUniverse ≠ right.ownerUniverse) :
    ¬ AdmitIntersection true left right := by
  intro admitted
  exact universeDrift admitted.2.2.2.2.2.2

def RankedTopK (generation ownerUniverse : String) (resultCount : Nat) : SetEvidence := {
  generation := generation
  ownerUniverse := ownerUniverse
  resultCount := resultCount
  complete := false
  truncated := true
}

theorem ranked_top_k_cannot_prove_set_intersection
    (other : SetEvidence) :
    ¬ AdmitIntersection true
      (RankedTopK other.generation other.ownerUniverse other.resultCount)
      other := by
  intro admitted
  exact Bool.noConfusion admitted.2.1

end ASPProof.LiveCorpusPredicateRoute
