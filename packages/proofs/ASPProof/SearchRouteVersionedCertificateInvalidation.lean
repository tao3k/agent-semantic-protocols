-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteVersionedCertificateInvalidation

structure ArtifactKey where
  artifactId : Nat
  revision : Nat
  deriving DecidableEq, Repr

abbrev ArtifactSet := ArtifactKey → Prop
abbrev DependsOn := ArtifactKey → ArtifactKey → Prop

inductive DependsTransitively
    (dependsOn : DependsOn) : ArtifactKey → ArtifactKey → Prop
  | direct {artifact prerequisite} :
      dependsOn artifact prerequisite →
      DependsTransitively dependsOn artifact prerequisite
  | step {artifact intermediate prerequisite} :
      dependsOn artifact intermediate →
      DependsTransitively dependsOn intermediate prerequisite →
      DependsTransitively dependsOn artifact prerequisite

def Invalidated
    (dependsOn : DependsOn)
    (correctedRoots : ArtifactSet)
    (artifact : ArtifactKey) : Prop :=
  correctedRoots artifact
    ∨ ∃ root,
        correctedRoots root
          ∧ DependsTransitively dependsOn artifact root

def ClosedUnderDependence
    (dependsOn : DependsOn)
    (artifacts : ArtifactSet) : Prop :=
  ∀ {artifact prerequisite},
    artifacts prerequisite →
    dependsOn artifact prerequisite →
    artifacts artifact

theorem corrected_root_is_invalidated
    (dependsOn : DependsOn)
    (correctedRoots : ArtifactSet)
    (root : ArtifactKey)
    (corrected : correctedRoots root) :
    Invalidated dependsOn correctedRoots root :=
  Or.inl corrected

theorem direct_dependent_is_invalidated
    (dependsOn : DependsOn)
    (correctedRoots : ArtifactSet)
    (artifact root : ArtifactKey)
    (corrected : correctedRoots root)
    (depends : dependsOn artifact root) :
    Invalidated dependsOn correctedRoots artifact :=
  Or.inr ⟨root, corrected, .direct depends⟩

theorem invalidation_is_dependency_closed
    (dependsOn : DependsOn)
    (correctedRoots : ArtifactSet)
    {artifact prerequisite : ArtifactKey}
    (invalidated : Invalidated dependsOn correctedRoots prerequisite)
    (depends : dependsOn artifact prerequisite) :
    Invalidated dependsOn correctedRoots artifact := by
  rcases invalidated with corrected | ⟨root, corrected, reaches⟩
  · exact Or.inr ⟨prerequisite, corrected, .direct depends⟩
  · exact Or.inr ⟨root, corrected, .step depends reaches⟩

theorem invalidation_is_monotone_in_roots
    (dependsOn : DependsOn)
    (smaller larger : ArtifactSet)
    (contains : ∀ artifact, smaller artifact → larger artifact)
    (artifact : ArtifactKey)
    (invalidated : Invalidated dependsOn smaller artifact) :
    Invalidated dependsOn larger artifact := by
  rcases invalidated with corrected | ⟨root, corrected, reaches⟩
  · exact Or.inl (contains artifact corrected)
  · exact Or.inr ⟨root, contains root corrected, reaches⟩

theorem transitive_dependence_lifts_closed_sets
    (dependsOn : DependsOn)
    (artifacts : ArtifactSet)
    (closed : ClosedUnderDependence dependsOn artifacts)
    {artifact root : ArtifactKey}
    (reaches : DependsTransitively dependsOn artifact root)
    (containsRoot : artifacts root) :
    artifacts artifact := by
  induction reaches with
  | direct depends =>
      exact closed containsRoot depends
  | step depends _ inductionHypothesis =>
      exact closed (inductionHypothesis containsRoot) depends

theorem invalidation_is_minimal
    (dependsOn : DependsOn)
    (correctedRoots artifacts : ArtifactSet)
    (containsRoots : ∀ root, correctedRoots root → artifacts root)
    (closed : ClosedUnderDependence dependsOn artifacts)
    (artifact : ArtifactKey)
    (invalidated : Invalidated dependsOn correctedRoots artifact) :
    artifacts artifact := by
  rcases invalidated with corrected | ⟨root, corrected, reaches⟩
  · exact containsRoots artifact corrected
  · exact transitive_dependence_lifts_closed_sets
      dependsOn artifacts closed reaches (containsRoots root corrected)

theorem reachability_preserves_revision
    (dependsOn : DependsOn)
    (edgePreservesRevision :
      ∀ {artifact prerequisite},
        dependsOn artifact prerequisite →
        artifact.revision = prerequisite.revision)
    {artifact prerequisite : ArtifactKey}
    (reaches : DependsTransitively dependsOn artifact prerequisite) :
    artifact.revision = prerequisite.revision := by
  induction reaches with
  | direct depends =>
      exact edgePreservesRevision depends
  | step depends _ inductionHypothesis =>
      exact (edgePreservesRevision depends).trans inductionHypothesis

def oldEffect : ArtifactKey := ⟨0, 7⟩
def oldFeasibility : ArtifactKey := ⟨1, 7⟩
def oldRanking : ArtifactKey := ⟨2, 7⟩
def oldCompletion : ArtifactKey := ⟨3, 7⟩
def refreshedEffect : ArtifactKey := ⟨0, 8⟩
def refreshedFeasibility : ArtifactKey := ⟨1, 8⟩
def unrelatedCapability : ArtifactKey := ⟨4, 21⟩

inductive ExampleDepends : DependsOn
  | oldFeasibilityOnEffect :
      ExampleDepends oldFeasibility oldEffect
  | oldRankingOnFeasibility :
      ExampleDepends oldRanking oldFeasibility
  | oldCompletionOnRanking :
      ExampleDepends oldCompletion oldRanking
  | refreshedFeasibilityOnEffect :
      ExampleDepends refreshedFeasibility refreshedEffect

def correctedRoots : ArtifactSet :=
  fun artifact => artifact = oldEffect

theorem example_edges_preserve_revision
    {artifact prerequisite : ArtifactKey}
    (depends : ExampleDepends artifact prerequisite) :
    artifact.revision = prerequisite.revision := by
  cases depends <;> rfl

theorem old_completion_is_invalidated :
    Invalidated ExampleDepends correctedRoots oldCompletion := by
  exact Or.inr
    ⟨ oldEffect
    , rfl
    , .step .oldCompletionOnRanking
        (.step .oldRankingOnFeasibility
          (.direct .oldFeasibilityOnEffect))
    ⟩

theorem refreshed_certificate_is_not_invalidated :
    ¬ Invalidated ExampleDepends correctedRoots refreshedFeasibility := by
  intro invalidated
  rcases invalidated with corrected | ⟨root, corrected, reaches⟩
  · simp [correctedRoots, refreshedFeasibility, oldEffect] at corrected
  · have rootIdentity : root = oldEffect := corrected
    subst root
    have revisionIdentity :=
      reachability_preserves_revision
        ExampleDepends example_edges_preserve_revision reaches
    simp [refreshedFeasibility, oldEffect] at revisionIdentity

theorem unrelated_artifact_is_not_invalidated :
    ¬ Invalidated ExampleDepends correctedRoots unrelatedCapability := by
  intro invalidated
  rcases invalidated with corrected | ⟨root, corrected, reaches⟩
  · simp [correctedRoots, unrelatedCapability, oldEffect] at corrected
  · have rootIdentity : root = oldEffect := corrected
    subst root
    have revisionIdentity :=
      reachability_preserves_revision
        ExampleDepends example_edges_preserve_revision reaches
    simp [unrelatedCapability, oldEffect] at revisionIdentity

end ASPProof.SearchRouteVersionedCertificateInvalidation
