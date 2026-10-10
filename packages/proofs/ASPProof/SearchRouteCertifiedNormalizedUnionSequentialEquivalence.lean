-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin

namespace ASPProof.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

open ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin

/-- A certified keep mask is the finite normalization decision for each route. -/
abbrev KeepMask := CanonicalMembership

def normalizeMembership
    (keep : KeepMask)
    (candidates : CanonicalMembership) : CanonicalMembership :=
  fun canonicalId => keep canonicalId && candidates canonicalId

def insertNormalized
    (keep : KeepMask)
    (frontier incoming : CanonicalMembership) : CanonicalMembership :=
  normalizeMembership keep (mergeMembership frontier incoming)

theorem normalization_is_idempotent
    (keep : KeepMask)
    (candidates : CanonicalMembership)
    (canonicalId : Nat) :
    normalizeMembership keep (normalizeMembership keep candidates) canonicalId =
      normalizeMembership keep candidates canonicalId := by
  unfold normalizeMembership
  cases keep canonicalId <;> cases candidates canonicalId <;> rfl

theorem normalized_union_equals_sequential_insertion
    (keep : KeepMask)
    (left right : CanonicalMembership)
    (canonicalId : Nat) :
    normalizeMembership keep (mergeMembership left right) canonicalId =
      insertNormalized keep (normalizeMembership keep left) right canonicalId := by
  change
    (keep canonicalId && (left canonicalId || right canonicalId)) =
      (keep canonicalId &&
        ((keep canonicalId && left canonicalId) || right canonicalId))
  cases keep canonicalId <;>
    cases left canonicalId <;>
      cases right canonicalId <;> rfl

theorem normalized_three_way_union_equals_sequential_insertion
    (keep : KeepMask)
    (first second third : CanonicalMembership)
    (canonicalId : Nat) :
    normalizeMembership keep
        (mergeMembership (mergeMembership first second) third)
        canonicalId =
      insertNormalized keep
        (insertNormalized keep (normalizeMembership keep first) second)
        third
        canonicalId := by
  change
    (keep canonicalId &&
      ((first canonicalId || second canonicalId) || third canonicalId)) =
      (keep canonicalId &&
        ((keep canonicalId &&
          ((keep canonicalId && first canonicalId) || second canonicalId)) ||
          third canonicalId))
  cases keep canonicalId <;>
    cases first canonicalId <;>
      cases second canonicalId <;>
        cases third canonicalId <;> rfl

theorem normalized_parallel_fanin_equals_normalized_sequential_fold
    (keep : KeepMask)
    (first second third : CanonicalMembership)
    (canonicalId : Nat) :
    normalizeMembership keep
        (mergeMembership (mergeMembership first second) third)
        canonicalId =
      normalizeMembership keep
        (mergeMembership first (mergeMembership second third))
        canonicalId := by
  unfold normalizeMembership mergeMembership
  cases keep canonicalId <;>
    cases first canonicalId <;>
      cases second canonicalId <;>
        cases third canonicalId <;> rfl

/-- Provenance for one canonical route is a set and uses the same ACI union. -/
abbrev EvidenceMembership := CanonicalMembership

def mergeDuplicateEvidence
    (left right : EvidenceMembership) : EvidenceMembership :=
  mergeMembership left right

theorem duplicate_evidence_merge_is_commutative
    (left right : EvidenceMembership)
    (evidenceId : Nat) :
    mergeDuplicateEvidence left right evidenceId =
      mergeDuplicateEvidence right left evidenceId :=
  mergeMembership_commutative left right evidenceId

def emptyEvidence : EvidenceMembership :=
  fun _ => false

def earlyEvidence : EvidenceMembership
  | 0 => true
  | _ => false

def lateDuplicateEvidence : EvidenceMembership
  | 1 => true
  | _ => false

def correctDuplicateEvidence : EvidenceMembership :=
  mergeDuplicateEvidence earlyEvidence lateDuplicateEvidence

/-- This models destructive pruning that drops the early evidence set. -/
def prematurelyPrunedDuplicateEvidence : EvidenceMembership :=
  mergeDuplicateEvidence emptyEvidence lateDuplicateEvidence

theorem normalized_union_preserves_early_provenance :
    correctDuplicateEvidence 0 = true :=
  rfl

theorem premature_destructive_pruning_drops_early_provenance :
    prematurelyPrunedDuplicateEvidence 0 = false :=
  rfl

theorem destructive_pruning_is_not_extensionally_equivalent :
    correctDuplicateEvidence 0 = true ∧
      prematurelyPrunedDuplicateEvidence 0 = false := by
  constructor <;> rfl

/-- Identity of the finite normalization decision and its evidence. -/
structure NormalizationIdentity where
  evaluation : EvaluationIdentity
  candidateUniverseRoot : Nat
  costVectorRoot : Nat
  dominanceCertificateRoot : Nat
  keepMaskRoot : Nat
deriving DecidableEq

def NormalizationAuthorized
    (left right : NormalizationIdentity) : Prop :=
  left = right

theorem same_normalization_identity_authorizes_reuse
    (identity : NormalizationIdentity) :
    NormalizationAuthorized identity identity :=
  rfl

theorem changed_candidate_universe_rejects_reuse
    (left right : NormalizationIdentity)
    (changed :
      left.candidateUniverseRoot ≠ right.candidateUniverseRoot) :
    ¬ NormalizationAuthorized left right := by
  intro sameIdentity
  apply changed
  exact congrArg NormalizationIdentity.candidateUniverseRoot sameIdentity

theorem changed_dominance_certificate_rejects_reuse
    (left right : NormalizationIdentity)
    (changed :
      left.dominanceCertificateRoot ≠ right.dominanceCertificateRoot) :
    ¬ NormalizationAuthorized left right := by
  intro sameIdentity
  apply changed
  exact congrArg NormalizationIdentity.dominanceCertificateRoot sameIdentity

structure NormalizationReceiptBudget where
  fixedBytes : Nat
  bytesPerCandidate : Nat
  candidateCapacity : Nat

def explicitNormalizationReceiptUpperBound
    (budget : NormalizationReceiptBudget) : Nat :=
  budget.fixedBytes +
    budget.bytesPerCandidate * budget.candidateCapacity

theorem explicit_normalization_receipt_is_capacity_bounded
    (budget : NormalizationReceiptBudget)
    (observedBytes : Nat)
    (withinBudget :
      observedBytes ≤ explicitNormalizationReceiptUpperBound budget) :
    observedBytes ≤ explicitNormalizationReceiptUpperBound budget :=
  withinBudget

def uncappedNormalizationDisclosureBytes
    (candidateCount : Nat) : Nat :=
  candidateCount

theorem uncapped_normalization_disclosure_exceeds_every_fixed_bound
    (fixedBound : Nat) :
    fixedBound <
      uncappedNormalizationDisclosureBytes (fixedBound + 1) := by
  exact Nat.lt_succ_self fixedBound

def summarizedNormalizationReceiptBytes
    (fixedSummaryBytes _candidateCount : Nat) : Nat :=
  fixedSummaryBytes

theorem summarized_normalization_receipt_is_count_independent
    (fixedSummaryBytes firstCount secondCount : Nat) :
    summarizedNormalizationReceiptBytes fixedSummaryBytes firstCount =
      summarizedNormalizationReceiptBytes fixedSummaryBytes secondCount :=
  rfl

end ASPProof.SearchRouteCertifiedNormalizedUnionSequentialEquivalence
