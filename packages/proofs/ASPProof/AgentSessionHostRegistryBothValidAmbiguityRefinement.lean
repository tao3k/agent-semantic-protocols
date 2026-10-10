-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryAdjudicationDecisionCertificate

namespace ASPProof.AgentSessionHostRegistryBothValidAmbiguityRefinement

open ASPProof.AgentSessionHostRegistryAdjudicationDecisionCertificate

structure TerminalSemantics where
  outcomeDigest : Nat
  effectDigest : Nat
  deriving DecidableEq, Repr

structure SemanticEquivalenceWitness where
  acceptedProvenanceRoot : Nat
  conflictingProvenanceRoot : Nat
  acceptedSemantics : TerminalSemantics
  conflictingSemantics : TerminalSemantics
  witnessDigest : Nat
  deriving DecidableEq, Repr

def ValidEquivalenceWitness
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticEquivalenceWitness) : Prop :=
  witness.acceptedProvenanceRoot = acceptedRoot ∧
  witness.conflictingProvenanceRoot = conflictingRoot ∧
  witness.acceptedSemantics = witness.conflictingSemantics

inductive PreferredTerminal where
  | accepted
  | conflicting
  deriving DecidableEq, Repr

structure SemanticIncompatibilityWitness where
  acceptedProvenanceRoot : Nat
  conflictingProvenanceRoot : Nat
  acceptedSemantics : TerminalSemantics
  conflictingSemantics : TerminalSemantics
  discriminatorDigest : Nat
  preferred : PreferredTerminal
  deriving DecidableEq, Repr

def ValidIncompatibilityWitness
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticIncompatibilityWitness) : Prop :=
  witness.acceptedProvenanceRoot = acceptedRoot ∧
  witness.conflictingProvenanceRoot = conflictingRoot ∧
  witness.acceptedSemantics ≠ witness.conflictingSemantics ∧
  witness.discriminatorDigest ≠ 0

inductive SemanticResolutionOutcome where
  | coalescedEquivalent
  | preferAccepted
  | preferConflicting
  deriving DecidableEq, Repr

def EquivalenceEntails
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticEquivalenceWitness)
    (outcome : SemanticResolutionOutcome) : Prop :=
  ValidEquivalenceWitness acceptedRoot conflictingRoot witness ∧
  outcome = .coalescedEquivalent

def IncompatibilityEntails
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticIncompatibilityWitness)
    (outcome : SemanticResolutionOutcome) : Prop :=
  ValidIncompatibilityWitness acceptedRoot conflictingRoot witness ∧
  outcome = match witness.preferred with
    | .accepted => .preferAccepted
    | .conflicting => .preferConflicting

structure AmbiguityRefinementState where
  acceptedProvenanceRoot : Nat
  conflictingProvenanceRoot : Nat
  ambiguityRank : Nat
  evidenceBudget : Nat
  deriving DecidableEq, Repr

def AmbiguityResolved (state : AmbiguityRefinementState) : Prop :=
  state.ambiguityRank = 0

def RefinementStep
    (current next : AmbiguityRefinementState) : Prop :=
  next.acceptedProvenanceRoot = current.acceptedProvenanceRoot ∧
  next.conflictingProvenanceRoot = current.conflictingProvenanceRoot ∧
  next.ambiguityRank < current.ambiguityRank ∧
  next.evidenceBudget < current.evidenceBudget

inductive RefinementRun :
    AmbiguityRefinementState → AmbiguityRefinementState → Nat → Prop where
  | done (state : AmbiguityRefinementState) :
      RefinementRun state state 0
  | step
      {current next final : AmbiguityRefinementState}
      {steps : Nat}
      (progress : RefinementStep current next)
      (rest : RefinementRun next final steps) :
      RefinementRun current final (Nat.succ steps)

theorem valid_equivalence_binds_accepted_root
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticEquivalenceWitness}
    (valid : ValidEquivalenceWitness acceptedRoot conflictingRoot witness) :
    witness.acceptedProvenanceRoot = acceptedRoot := by
  exact valid.1

theorem valid_equivalence_binds_conflicting_root
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticEquivalenceWitness}
    (valid : ValidEquivalenceWitness acceptedRoot conflictingRoot witness) :
    witness.conflictingProvenanceRoot = conflictingRoot := by
  exact valid.2.1

theorem valid_equivalence_requires_equal_semantics
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticEquivalenceWitness}
    (valid : ValidEquivalenceWitness acceptedRoot conflictingRoot witness) :
    witness.acceptedSemantics = witness.conflictingSemantics := by
  exact valid.2.2

theorem false_equivalence_witness_is_rejected
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticEquivalenceWitness)
    (different : witness.acceptedSemantics ≠ witness.conflictingSemantics) :
    ¬ ValidEquivalenceWitness acceptedRoot conflictingRoot witness := by
  intro valid
  exact different valid.2.2

theorem valid_incompatibility_binds_both_roots
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticIncompatibilityWitness}
    (valid : ValidIncompatibilityWitness acceptedRoot conflictingRoot witness) :
    witness.acceptedProvenanceRoot = acceptedRoot ∧
    witness.conflictingProvenanceRoot = conflictingRoot := by
  exact ⟨valid.1, valid.2.1⟩

theorem valid_incompatibility_requires_different_semantics
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticIncompatibilityWitness}
    (valid : ValidIncompatibilityWitness acceptedRoot conflictingRoot witness) :
    witness.acceptedSemantics ≠ witness.conflictingSemantics := by
  exact valid.2.2.1

theorem missing_discriminator_rejects_incompatibility
    (acceptedRoot conflictingRoot : Nat)
    (witness : SemanticIncompatibilityWitness)
    (missing : witness.discriminatorDigest = 0) :
    ¬ ValidIncompatibilityWitness acceptedRoot conflictingRoot witness := by
  intro valid
  exact valid.2.2.2 missing

theorem equivalence_entails_only_coalescence
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticEquivalenceWitness}
    {outcome : SemanticResolutionOutcome}
    (entailed : EquivalenceEntails acceptedRoot conflictingRoot witness outcome) :
    outcome = .coalescedEquivalent := by
  exact entailed.2

theorem incompatibility_entails_preferred_outcome
    {acceptedRoot conflictingRoot : Nat}
    {witness : SemanticIncompatibilityWitness}
    {outcome : SemanticResolutionOutcome}
    (entailed : IncompatibilityEntails acceptedRoot conflictingRoot witness outcome) :
    outcome = match witness.preferred with
      | .accepted => .preferAccepted
      | .conflicting => .preferConflicting := by
  exact entailed.2

theorem refinement_preserves_provenance_roots
    {current next : AmbiguityRefinementState}
    (step : RefinementStep current next) :
    next.acceptedProvenanceRoot = current.acceptedProvenanceRoot ∧
    next.conflictingProvenanceRoot = current.conflictingProvenanceRoot := by
  exact ⟨step.1, step.2.1⟩

theorem refinement_strictly_reduces_ambiguity
    {current next : AmbiguityRefinementState}
    (step : RefinementStep current next) :
    next.ambiguityRank < current.ambiguityRank := by
  exact step.2.2.1

theorem refinement_consumes_evidence_budget
    {current next : AmbiguityRefinementState}
    (step : RefinementStep current next) :
    next.evidenceBudget < current.evidenceBudget := by
  exact step.2.2.2

theorem accepted_root_drift_rejects_refinement
    (current next : AmbiguityRefinementState)
    (drift : next.acceptedProvenanceRoot ≠ current.acceptedProvenanceRoot) :
    ¬ RefinementStep current next := by
  intro step
  exact drift step.1

theorem conflicting_root_drift_rejects_refinement
    (current next : AmbiguityRefinementState)
    (drift : next.conflictingProvenanceRoot ≠ current.conflictingProvenanceRoot) :
    ¬ RefinementStep current next := by
  intro step
  exact drift step.2.1

theorem unchanged_ambiguity_rejects_refinement
    (current next : AmbiguityRefinementState)
    (unchanged : next.ambiguityRank = current.ambiguityRank) :
    ¬ RefinementStep current next := by
  intro step
  exact (Nat.ne_of_lt step.2.2.1) unchanged

theorem refinement_self_loop_is_rejected
    (state : AmbiguityRefinementState) :
    ¬ RefinementStep state state := by
  intro step
  exact (Nat.lt_irrefl state.ambiguityRank) step.2.2.1

theorem resolved_ambiguity_has_no_refinement
    (state next : AmbiguityRefinementState)
    (resolved : AmbiguityResolved state) :
    ¬ RefinementStep state next := by
  intro step
  have belowZero : next.ambiguityRank < 0 := by
    rw [← resolved]
    exact step.2.2.1
  exact (Nat.not_lt_zero next.ambiguityRank) belowZero

theorem refinement_run_length_bounded_by_initial_ambiguity
    {initial final : AmbiguityRefinementState}
    {steps : Nat}
    (run : RefinementRun initial final steps) :
    steps ≤ initial.ambiguityRank := by
  induction run with
  | done state =>
      exact Nat.zero_le state.ambiguityRank
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2.1)

theorem refinement_run_length_bounded_by_evidence_budget
    {initial final : AmbiguityRefinementState}
    {steps : Nat}
    (run : RefinementRun initial final steps) :
    steps ≤ initial.evidenceBudget := by
  induction run with
  | done state =>
      exact Nat.zero_le state.evidenceBudget
  | step progress rest inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (Nat.succ_le_of_lt progress.2.2.2)

theorem provenance_interpretation_refines_without_root_rewrite
    {current next : AmbiguityRefinementState}
    (step : RefinementStep current next) :
    next.acceptedProvenanceRoot = current.acceptedProvenanceRoot ∧
    next.conflictingProvenanceRoot = current.conflictingProvenanceRoot ∧
    next.ambiguityRank < current.ambiguityRank := by
  exact ⟨step.1, step.2.1, step.2.2.1⟩

end ASPProof.AgentSessionHostRegistryBothValidAmbiguityRefinement
