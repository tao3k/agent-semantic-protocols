-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

structure FaultBoundSnapshot
    (MembershipDigest AuthorityId EpochId Attestation : Type) where
  membershipDigest : MembershipDigest
  epochId : EpochId
  authorityId : AuthorityId
  issuedAt : Nat
  expiresAt : Nat
  declaredFaultBound : VotingWeight
  attestation : Attestation

structure FaultBoundAuthorityPolicy
    (MembershipDigest AuthorityId EpochId Attestation : Type) where
  expectedMembershipDigest : MembershipDigest
  expectedEpochId : EpochId
  evaluationTime : Nat
  authorized : AuthorityId → Prop
  verify :
    FaultBoundSnapshot
      MembershipDigest AuthorityId EpochId Attestation →
      Prop
  actualFaultWeight : VotingWeight
  verificationSound :
    ∀ snapshot,
      authorized snapshot.authorityId →
      verify snapshot →
      actualFaultWeight ≤ snapshot.declaredFaultBound

def FaultBoundSnapshotValid
    {MembershipDigest AuthorityId EpochId Attestation : Type}
    (policy :
      FaultBoundAuthorityPolicy
        MembershipDigest AuthorityId EpochId Attestation)
    (snapshot :
      FaultBoundSnapshot
        MembershipDigest AuthorityId EpochId Attestation) : Prop :=
  snapshot.membershipDigest = policy.expectedMembershipDigest ∧
  snapshot.epochId = policy.expectedEpochId ∧
  policy.authorized snapshot.authorityId ∧
  policy.verify snapshot ∧
  snapshot.issuedAt ≤ policy.evaluationTime ∧
  policy.evaluationTime < snapshot.expiresAt

theorem valid_snapshot_bounds_actual_fault_weight
    {MembershipDigest AuthorityId EpochId Attestation : Type}
    (policy :
      FaultBoundAuthorityPolicy
        MembershipDigest AuthorityId EpochId Attestation)
    (snapshot :
      FaultBoundSnapshot
        MembershipDigest AuthorityId EpochId Attestation)
    (valid : FaultBoundSnapshotValid policy snapshot) :
    policy.actualFaultWeight ≤ snapshot.declaredFaultBound :=
  policy.verificationSound
    snapshot valid.2.2.1 valid.2.2.2.1

theorem valid_snapshot_matches_membership_and_epoch
    {MembershipDigest AuthorityId EpochId Attestation : Type}
    (policy :
      FaultBoundAuthorityPolicy
        MembershipDigest AuthorityId EpochId Attestation)
    (snapshot :
      FaultBoundSnapshot
        MembershipDigest AuthorityId EpochId Attestation)
    (valid : FaultBoundSnapshotValid policy snapshot) :
    snapshot.membershipDigest = policy.expectedMembershipDigest ∧
    snapshot.epochId = policy.expectedEpochId :=
  ⟨valid.1, valid.2.1⟩

theorem valid_snapshot_is_current_at_evaluation_time
    {MembershipDigest AuthorityId EpochId Attestation : Type}
    (policy :
      FaultBoundAuthorityPolicy
        MembershipDigest AuthorityId EpochId Attestation)
    (snapshot :
      FaultBoundSnapshot
        MembershipDigest AuthorityId EpochId Attestation)
    (valid : FaultBoundSnapshotValid policy snapshot) :
    snapshot.issuedAt ≤ policy.evaluationTime ∧
    policy.evaluationTime < snapshot.expiresAt :=
  ⟨valid.2.2.2.2.1, valid.2.2.2.2.2⟩

theorem authorized_current_bound_forces_actual_honest_overlap
    (scheme : CanonicalMembershipScheme)
    (membershipPolicy : MembershipPolicy)
    (left right : QuorumCertificate scheme)
    (accounting :
      PairWeightAccounting
        membershipPolicy left.signers right.signers)
    {AuthorityId EpochId Attestation : Type}
    (authorityPolicy :
      FaultBoundAuthorityPolicy
        scheme.Digest AuthorityId EpochId Attestation)
    (snapshot :
      FaultBoundSnapshot
        scheme.Digest AuthorityId EpochId Attestation)
    (sameMembership :
      authorityPolicy.expectedMembershipDigest =
        membershipDigest scheme membershipPolicy)
    (valid : FaultBoundSnapshotValid authorityPolicy snapshot)
    (threshold :
      totalVotingWeight membershipPolicy +
          snapshot.declaredFaultBound <
        membershipPolicy.quorumWeight +
          membershipPolicy.quorumWeight)
    (leftAccepted :
      AcceptsQuorumCertificate scheme membershipPolicy left)
    (rightAccepted :
      AcceptsQuorumCertificate scheme membershipPolicy right) :
    authorityPolicy.actualFaultWeight <
        accounting.intersectionWeight ∧
      snapshot.membershipDigest =
        membershipDigest scheme membershipPolicy := by
  have boundBelowIntersection :=
    threshold_over_fault_bound_forces_honest_overlap
      scheme membershipPolicy left right accounting
      snapshot.declaredFaultBound threshold
      leftAccepted rightAccepted
  have actualBelowIntersection :=
    Nat.lt_of_le_of_lt
      (valid_snapshot_bounds_actual_fault_weight
        authorityPolicy snapshot valid)
      boundBelowIntersection
  have snapshotMatches :=
    valid_snapshot_matches_membership_and_epoch
      authorityPolicy snapshot valid
  exact ⟨
    actualBelowIntersection,
    Eq.trans snapshotMatches.1 sameMembership
  ⟩

structure FaultBoundAdmissionObservation where
  totalWeight : VotingWeight
  quorumWeight : VotingWeight
  declaredFaultBound : VotingWeight
  membershipMatches : Bool
  epochMatches : Bool
  authorityAuthorized : Bool
  attestationVerified : Bool
  temporallyCurrent : Bool

def ArithmeticThresholdPasses
    (observation : FaultBoundAdmissionObservation) : Prop :=
  observation.totalWeight + observation.declaredFaultBound <
    observation.quorumWeight + observation.quorumWeight

def SnapshotAdmissionPasses
    (observation : FaultBoundAdmissionObservation) : Prop :=
  observation.membershipMatches = true ∧
  observation.epochMatches = true ∧
  observation.authorityAuthorized = true ∧
  observation.attestationVerified = true ∧
  observation.temporallyCurrent = true

def unauthorizedLowBoundObservation : FaultBoundAdmissionObservation where
  totalWeight := 3
  quorumWeight := 2
  declaredFaultBound := 0
  membershipMatches := true
  epochMatches := true
  authorityAuthorized := false
  attestationVerified := true
  temporallyCurrent := true

def staleLowBoundObservation : FaultBoundAdmissionObservation where
  totalWeight := 3
  quorumWeight := 2
  declaredFaultBound := 0
  membershipMatches := true
  epochMatches := true
  authorityAuthorized := true
  attestationVerified := true
  temporallyCurrent := false

theorem unauthorized_low_bound_can_pass_arithmetic_but_not_admission :
    ArithmeticThresholdPasses unauthorizedLowBoundObservation ∧
    ¬ SnapshotAdmissionPasses unauthorizedLowBoundObservation := by
  constructor
  · exact Nat.lt_succ_self 3
  · intro admitted
    exact Bool.noConfusion admitted.2.2.1

theorem stale_low_bound_can_pass_arithmetic_but_not_admission :
    ArithmeticThresholdPasses staleLowBoundObservation ∧
    ¬ SnapshotAdmissionPasses staleLowBoundObservation := by
  constructor
  · exact Nat.lt_succ_self 3
  · intro admitted
    exact Bool.noConfusion admitted.2.2.2.2

end ASPProof.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority
