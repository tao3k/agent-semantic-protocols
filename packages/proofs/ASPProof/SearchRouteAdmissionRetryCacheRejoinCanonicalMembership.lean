-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

abbrev NodeId := Nat
abbrev FailureDomainId := Nat
abbrev VotingWeight := Nat

structure MembershipEntry where
  nodeId : NodeId
  votingWeight : VotingWeight
  failureDomainId : FailureDomainId

structure MembershipPolicy where
  members : List MembershipEntry
  quorumWeight : VotingWeight
  minimumFailureDomains : Nat

def MembershipPolicyWellFormed
    (policy : MembershipPolicy) : Prop :=
  (policy.members.map MembershipEntry.nodeId).Nodup ∧
  (∀ member, member ∈ policy.members → 0 < member.votingWeight) ∧
  0 < policy.quorumWeight ∧
  0 < policy.minimumFailureDomains

structure CanonicalMembershipScheme where
  Canonical : Type
  Digest : Type
  canonicalize : MembershipPolicy → Canonical
  commit : Canonical → Digest
  permutationInvariant :
    ∀ left right,
      left.members.Perm right.members →
      left.quorumWeight = right.quorumWeight →
      left.minimumFailureDomains = right.minimumFailureDomains →
      canonicalize left = canonicalize right
  collisionFree : Function.Injective commit
  reflectsQuorumWeight :
    ∀ left right,
      canonicalize left = canonicalize right →
      left.quorumWeight = right.quorumWeight
  reflectsMinimumFailureDomains :
    ∀ left right,
      canonicalize left = canonicalize right →
      left.minimumFailureDomains = right.minimumFailureDomains

def membershipDigest
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy) : scheme.Digest :=
  scheme.commit (scheme.canonicalize policy)

theorem member_order_does_not_change_membership_digest
    (scheme : CanonicalMembershipScheme)
    (left right : MembershipPolicy)
    (sameMembers : left.members.Perm right.members)
    (sameQuorumWeight : left.quorumWeight = right.quorumWeight)
    (sameFailureDomains :
      left.minimumFailureDomains = right.minimumFailureDomains) :
    membershipDigest scheme left = membershipDigest scheme right := by
  unfold membershipDigest
  rw [scheme.permutationInvariant
    left right sameMembers sameQuorumWeight sameFailureDomains]

theorem equal_membership_digest_implies_equal_canonical_policy
    (scheme : CanonicalMembershipScheme)
    (left right : MembershipPolicy)
    (sameDigest :
      membershipDigest scheme left = membershipDigest scheme right) :
    scheme.canonicalize left = scheme.canonicalize right := by
  exact scheme.collisionFree sameDigest

theorem changed_quorum_weight_changes_membership_digest
    (scheme : CanonicalMembershipScheme)
    (left right : MembershipPolicy)
    (changed : left.quorumWeight ≠ right.quorumWeight) :
    membershipDigest scheme left ≠ membershipDigest scheme right := by
  intro sameDigest
  exact changed
    (scheme.reflectsQuorumWeight left right
      (equal_membership_digest_implies_equal_canonical_policy
        scheme left right sameDigest))

theorem changed_failure_domain_requirement_changes_membership_digest
    (scheme : CanonicalMembershipScheme)
    (left right : MembershipPolicy)
    (changed :
      left.minimumFailureDomains ≠ right.minimumFailureDomains) :
    membershipDigest scheme left ≠ membershipDigest scheme right := by
  intro sameDigest
  exact changed
    (scheme.reflectsMinimumFailureDomains left right
      (equal_membership_digest_implies_equal_canonical_policy
        scheme left right sameDigest))

theorem duplicate_node_identity_is_not_well_formed
    (policy : MembershipPolicy)
    (duplicate :
      ¬ (policy.members.map MembershipEntry.nodeId).Nodup) :
    ¬ MembershipPolicyWellFormed policy := by
  intro wellFormed
  exact duplicate wellFormed.1

theorem zero_weight_member_is_not_well_formed
    (policy : MembershipPolicy)
    (member : MembershipEntry)
    (present : member ∈ policy.members)
    (zeroWeight : member.votingWeight = 0) :
    ¬ MembershipPolicyWellFormed policy := by
  intro wellFormed
  have positive := wellFormed.2.1 member present
  rw [zeroWeight] at positive
  exact Nat.lt_asymm positive positive

end ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
