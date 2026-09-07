-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDecisionSufficientInspect

namespace ASPProof.SearchRouteCertifiedAmbiguity

open ASPProof.SearchRouteDecisionSufficientInspect

def ActionConflict
    {World Projection Action : Type}
    (project : World → Projection)
    (requiredAction : World → Action)
    (pair : World × World) : Prop :=
  project pair.1 = project pair.2 ∧
  requiredAction pair.1 ≠ requiredAction pair.2

structure ConflictCertificate
    (World Projection Action : Type)
    [DecidableEq World]
    (project : World → Projection)
    (requiredAction : World → Action) where
  decisionScopeDigest : Nat
  evidenceGeneration : Nat
  pairs : List (World × World)
  sound :
    ∀ pair, pair ∈ pairs →
      ActionConflict project requiredAction pair
  complete :
    ∀ pair, ActionConflict project requiredAction pair →
      pair ∈ pairs

def CertifiedAmbiguity
    {World Projection Action : Type}
    [DecidableEq World]
    {project : World → Projection}
    {requiredAction : World → Action}
    (certificate :
      ConflictCertificate
        World Projection Action project requiredAction) : Nat :=
  certificate.pairs.length

def DecisionSufficient
    {World Projection Action : Type}
    (project : World → Projection)
    (requiredAction : World → Action) : Prop :=
  ∀ left right,
    project left = project right →
    requiredAction left = requiredAction right

theorem empty_complete_certificate_implies_decision_sufficient
    {World Projection Action : Type}
    [DecidableEq World]
    (project : World → Projection)
    (requiredAction : World → Action)
    (certificate :
      ConflictCertificate
        World Projection Action project requiredAction)
    (empty : certificate.pairs = []) :
    DecisionSufficient project requiredAction := by
  intro left right sameProjection
  by_cases sameAction : requiredAction left = requiredAction right
  · exact sameAction
  · have listed :=
      certificate.complete
        (left, right)
        ⟨sameProjection, sameAction⟩
    rw [empty] at listed
    simp at listed

def tinyConflictPairs : List (ExampleWorld × ExampleWorld) :=
  [
    (.selectorReady, .selectorMissing),
    (.selectorMissing, .selectorReady)
  ]

def tinyConflictCertificate :
    ConflictCertificate
      ExampleWorld
      TinyProjection
      NextAction
      tinyProject
      requiredAction :=
  {
    decisionScopeDigest := 51
    evidenceGeneration := 7
    pairs := tinyConflictPairs
    sound := by
      intro pair member
      rcases pair with ⟨left, right⟩
      cases left <;> cases right <;>
        simp [
          tinyConflictPairs,
          ActionConflict,
          tinyProject,
          requiredAction
        ] at member ⊢
    complete := by
      intro pair conflict
      rcases pair with ⟨left, right⟩
      cases left <;> cases right <;>
        simp [
          tinyConflictPairs,
          ActionConflict,
          tinyProject,
          requiredAction
        ] at conflict ⊢
  }

def statusConflictCertificate :
    ConflictCertificate
      ExampleWorld
      StatusProjection
      NextAction
      statusProject
      requiredAction :=
  {
    decisionScopeDigest := 51
    evidenceGeneration := 7
    pairs := []
    sound := by
      intro pair member
      simp at member
    complete := by
      intro pair conflict
      rcases pair with ⟨left, right⟩
      cases left <;> cases right <;>
        simp [
          ActionConflict,
          statusProject,
          requiredAction
        ] at conflict
  }

theorem tiny_certificate_is_sound
    (pair : ExampleWorld × ExampleWorld)
    (member : pair ∈ tinyConflictCertificate.pairs) :
    ActionConflict tinyProject requiredAction pair :=
  tinyConflictCertificate.sound pair member

theorem tiny_certificate_is_complete
    (pair : ExampleWorld × ExampleWorld)
    (conflict : ActionConflict tinyProject requiredAction pair) :
    pair ∈ tinyConflictCertificate.pairs :=
  tinyConflictCertificate.complete pair conflict

theorem status_certificate_is_sound
    (pair : ExampleWorld × ExampleWorld)
    (member : pair ∈ statusConflictCertificate.pairs) :
    ActionConflict statusProject requiredAction pair :=
  statusConflictCertificate.sound pair member

theorem status_certificate_is_complete
    (pair : ExampleWorld × ExampleWorld)
    (conflict : ActionConflict statusProject requiredAction pair) :
    pair ∈ statusConflictCertificate.pairs :=
  statusConflictCertificate.complete pair conflict

theorem tiny_projection_is_not_decision_sufficient :
    ¬ DecisionSufficient tinyProject requiredAction := by
  intro sufficient
  have impossible :=
    sufficient .selectorReady .selectorMissing rfl
  cases impossible

theorem tiny_certificate_has_ambiguity_two :
    CertifiedAmbiguity tinyConflictCertificate = 2 := by
  decide

theorem status_certificate_has_ambiguity_zero :
    CertifiedAmbiguity statusConflictCertificate = 0 := by
  decide

theorem status_certificate_proves_decision_sufficiency :
    DecisionSufficient statusProject requiredAction :=
  empty_complete_certificate_implies_decision_sufficient
    statusProject
    requiredAction
    statusConflictCertificate
    rfl

def CertifiedRefinement
    {World Action ProjectionA ProjectionB : Type}
    [DecidableEq World]
    {projectA : World → ProjectionA}
    {projectB : World → ProjectionB}
    {requiredAction : World → Action}
    (before :
      ConflictCertificate
        World ProjectionA Action projectA requiredAction)
    (after :
      ConflictCertificate
        World ProjectionB Action projectB requiredAction) : Prop :=
  before.decisionScopeDigest = after.decisionScopeDigest ∧
  before.evidenceGeneration = after.evidenceGeneration ∧
  CertifiedAmbiguity after < CertifiedAmbiguity before

theorem status_is_strict_refinement_of_tiny :
    CertifiedRefinement
      tinyConflictCertificate
      statusConflictCertificate := by
  unfold CertifiedRefinement
  decide

def driftedStatusConflictCertificate :
    ConflictCertificate
      ExampleWorld
      StatusProjection
      NextAction
      statusProject
      requiredAction :=
  { statusConflictCertificate with decisionScopeDigest := 52 }

theorem scope_drift_rejects_alleged_refinement :
    ¬ CertifiedRefinement
      tinyConflictCertificate
      driftedStatusConflictCertificate := by
  unfold CertifiedRefinement
  decide

def generationDriftedStatusConflictCertificate :
    ConflictCertificate
      ExampleWorld
      StatusProjection
      NextAction
      statusProject
      requiredAction :=
  { statusConflictCertificate with evidenceGeneration := 8 }

theorem generation_drift_rejects_alleged_refinement :
    ¬ CertifiedRefinement
      tinyConflictCertificate
      generationDriftedStatusConflictCertificate := by
  unfold CertifiedRefinement
  decide

end ASPProof.SearchRouteCertifiedAmbiguity
