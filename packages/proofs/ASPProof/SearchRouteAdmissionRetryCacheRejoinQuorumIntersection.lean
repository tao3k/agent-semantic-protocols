-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

structure PairWeightAccounting
    (policy : MembershipPolicy)
    (leftSigners rightSigners : SignerSet) where
  intersectionWeight : VotingWeight
  inclusionExclusionBound :
    realizedVotingWeight policy leftSigners +
        realizedVotingWeight policy rightSigners ≤
      totalVotingWeight policy + intersectionWeight

theorem accepted_certificates_establish_overlap_lower_bound
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (left right : QuorumCertificate scheme)
    (accounting :
      PairWeightAccounting policy left.signers right.signers)
    (leftAccepted : AcceptsQuorumCertificate scheme policy left)
    (rightAccepted : AcceptsQuorumCertificate scheme policy right) :
    policy.quorumWeight + policy.quorumWeight ≤
      totalVotingWeight policy + accounting.intersectionWeight := by
  exact Nat.le_trans
    (Nat.add_le_add
      (accepted_certificate_realizes_weight_threshold
        scheme policy left leftAccepted)
      (accepted_certificate_realizes_weight_threshold
        scheme policy right rightAccepted))
    accounting.inclusionExclusionBound

theorem threshold_over_fault_bound_forces_honest_overlap
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (left right : QuorumCertificate scheme)
    (accounting :
      PairWeightAccounting policy left.signers right.signers)
    (faultBound : VotingWeight)
    (threshold :
      totalVotingWeight policy + faultBound <
        policy.quorumWeight + policy.quorumWeight)
    (leftAccepted : AcceptsQuorumCertificate scheme policy left)
    (rightAccepted : AcceptsQuorumCertificate scheme policy right) :
    faultBound < accounting.intersectionWeight := by
  have overlapLowerBound :=
    accepted_certificates_establish_overlap_lower_bound
      scheme policy left right accounting leftAccepted rightAccepted
  have combined :
      totalVotingWeight policy + faultBound <
        totalVotingWeight policy + accounting.intersectionWeight :=
    Nat.lt_of_lt_of_le threshold overlapLowerBound
  exact Nat.lt_of_add_lt_add_left combined

theorem strict_weight_majority_forces_positive_overlap
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (left right : QuorumCertificate scheme)
    (accounting :
      PairWeightAccounting policy left.signers right.signers)
    (strictMajority :
      totalVotingWeight policy <
        policy.quorumWeight + policy.quorumWeight)
    (leftAccepted : AcceptsQuorumCertificate scheme policy left)
    (rightAccepted : AcceptsQuorumCertificate scheme policy right) :
    0 < accounting.intersectionWeight := by
  have threshold :
      totalVotingWeight policy + 0 <
        policy.quorumWeight + policy.quorumWeight := by
    rw [Nat.add_zero]
    exact strictMajority
  exact threshold_over_fault_bound_forces_honest_overlap
    scheme policy left right accounting 0 threshold
    leftAccepted rightAccepted

theorem half_total_threshold_allows_disjoint_realized_quorums :
    ∃ totalWeight quorumWeight leftWeight rightWeight intersectionWeight : Nat,
      quorumWeight ≤ leftWeight ∧
      quorumWeight ≤ rightWeight ∧
      leftWeight + rightWeight ≤ totalWeight + intersectionWeight ∧
      quorumWeight + quorumWeight = totalWeight ∧
      intersectionWeight = 0 := by
  exact ⟨2, 1, 1, 1, 0,
    Nat.le_refl 1,
    Nat.le_refl 1,
    Nat.le_refl 2,
    rfl,
    rfl⟩

theorem strict_majority_overlap_can_be_entirely_faulty :
    ∃ totalWeight quorumWeight leftWeight rightWeight
        intersectionWeight faultBound : Nat,
      quorumWeight ≤ leftWeight ∧
      quorumWeight ≤ rightWeight ∧
      leftWeight + rightWeight ≤ totalWeight + intersectionWeight ∧
      totalWeight < quorumWeight + quorumWeight ∧
      intersectionWeight ≤ faultBound := by
  exact ⟨3, 2, 2, 2, 1, 1,
    Nat.le_refl 2,
    Nat.le_refl 2,
    Nat.le_refl 4,
    Nat.lt_succ_self 3,
    Nat.le_refl 1⟩

end ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection
