-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

/-!
# Org-native Relationship Contract

This module is deliberately artifact-neutral. RFC sections, Org documents,
source files, proof modules, schemas, rendered documents, and receipts are all
instances of the same artifact model. Org Contract supplies structural query
evidence; this module states the admission boundary for relationships between
the resulting artifact observations.
-/

namespace ASPProof.RelationshipContract

inductive ArtifactKind where
  | orgDocument
  | orgSection
  | orgContract
  | sourceFile
  | proofModule
  | schema
  | receipt
  | renderedDocument
  deriving DecidableEq, Repr

inductive Predicate where
  | governedBy
  | dependsOn
  | references
  | realizes
  | proves
  | audits
  | conformsTo
  | renders
  deriving DecidableEq, Repr

inductive Impact where
  | invalidatesSubject
  | invalidatesObject
  | informational
  deriving DecidableEq, Repr

structure ArtifactDeclaration where
  id : String
  kind : ArtifactKind
  path : String
  canonicalization : String
  deriving DecidableEq, Repr

structure ArtifactObservation where
  declaration : ArtifactDeclaration
  observedDigest : String
  exactIdentity : Bool
  deriving DecidableEq, Repr

structure RelationshipDeclaration where
  id : String
  subject : String
  predicate : Predicate
  object : String
  impact : Impact
  evidenceGate : String
  deriving DecidableEq, Repr

structure RelationshipObservation where
  declaration : RelationshipDeclaration
  subject : ArtifactObservation
  object : ArtifactObservation
  exactSubject : Bool
  exactObject : Bool
  exactPredicate : Bool
  gatePassed : Bool
  deriving DecidableEq, Repr

/-- Admission is evidence-bearing. Matching names or digests are not gates. -/
def Admitted (observation : RelationshipObservation) : Prop :=
  observation.exactSubject = true ∧
    observation.exactObject = true ∧
    observation.exactPredicate = true ∧
    observation.subject.exactIdentity = true ∧
    observation.object.exactIdentity = true ∧
    observation.gatePassed = true

theorem admitted_requires_exact_subject
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.exactSubject = true :=
  h.1

theorem admitted_requires_exact_object
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.exactObject = true :=
  h.2.1

theorem admitted_requires_exact_predicate
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.exactPredicate = true :=
  h.2.2.1

theorem admitted_requires_subject_identity
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.subject.exactIdentity = true :=
  h.2.2.2.1

theorem admitted_requires_object_identity
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.object.exactIdentity = true :=
  h.2.2.2.2.1

theorem admitted_requires_evidence_gate
    (observation : RelationshipObservation)
    (h : Admitted observation) : observation.gatePassed = true :=
  h.2.2.2.2.2

/-- A digest match without a predicate-specific evidence gate is not admitted. -/
theorem digest_identity_without_gate_is_not_admitted
    (observation : RelationshipObservation)
    (_sameDigest : observation.subject.observedDigest = observation.object.observedDigest)
    (hGate : observation.gatePassed = false) : ¬ Admitted observation := by
  intro h
  have hPassed : observation.gatePassed = true := admitted_requires_evidence_gate observation h
  simp [hGate] at hPassed

/-- A structural Org Contract pass cannot substitute for exact endpoint identity. -/
theorem gate_without_exact_subject_is_not_admitted
    (observation : RelationshipObservation)
    (hSubject : observation.exactSubject = false) : ¬ Admitted observation := by
  intro h
  have hExact : observation.exactSubject = true := admitted_requires_exact_subject observation h
  simp [hSubject] at hExact

/-- Informational references do not enter the invalidation graph. -/
def EntersInvalidationGraph (relationship : RelationshipDeclaration) : Prop :=
  relationship.impact ≠ .informational

def InvalidatingStep
    (relationships : List RelationshipDeclaration)
    (changed affected : String) : Prop :=
  ∃ relationship ∈ relationships,
    (relationship.impact = .invalidatesSubject ∧
        relationship.object = changed ∧
        relationship.subject = affected) ∨
      (relationship.impact = .invalidatesObject ∧
        relationship.subject = changed ∧
        relationship.object = affected)

inductive InvalidatingPath
    (relationships : List RelationshipDeclaration) : String → String → Prop where
  | single :
      InvalidatingStep relationships subject object →
      InvalidatingPath relationships subject object
  | trans :
      InvalidatingPath relationships subject middle →
      InvalidatingStep relationships middle object →
      InvalidatingPath relationships subject object

def InvalidatingAcyclic (relationships : List RelationshipDeclaration) : Prop :=
  ∀ artifactId, ¬ InvalidatingPath relationships artifactId artifactId

/-- Reciprocal invalidating edges make reverse-frontier reuse ill-defined. -/
theorem reciprocal_invalidating_edges_violate_acyclic
    (relationships : List RelationshipDeclaration)
    (left right : String)
    (hForward : InvalidatingStep relationships left right)
    (hBackward : InvalidatingStep relationships right left) :
    ¬ InvalidatingAcyclic relationships := by
  intro hAcyclic
  exact hAcyclic left (.trans (.single hForward) hBackward)

theorem informational_reference_does_not_invalidate
    (relationship : RelationshipDeclaration)
    (_hPredicate : relationship.predicate = .references)
    (hImpact : relationship.impact = .informational) :
    ¬ EntersInvalidationGraph relationship := by
  intro h
  simp [EntersInvalidationGraph, hImpact] at h

/-- RFC section commitments are an application profile, not the root model. -/
structure SectionCommitmentProfile where
  document : ArtifactDeclaration
  sections : List ArtifactDeclaration
  relationships : List RelationshipDeclaration
  deriving Repr

end ASPProof.RelationshipContract
