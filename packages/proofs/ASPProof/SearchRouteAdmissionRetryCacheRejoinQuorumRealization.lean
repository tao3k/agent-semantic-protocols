-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

abbrev SignerSet := List NodeId

def signerSelected (signers : SignerSet) (nodeId : NodeId) : Bool :=
  signers.any (fun signer => signer == nodeId)

def selectedMembers
    (policy : MembershipPolicy)
    (signers : SignerSet) : List MembershipEntry :=
  policy.members.filter (fun member =>
    signerSelected signers member.nodeId)

def projectedVotingWeight
    (members : List MembershipEntry)
    (signers : SignerSet) : VotingWeight :=
  members.foldr
    (fun member total =>
      if signerSelected signers member.nodeId
      then member.votingWeight + total
      else total)
    0

def realizedVotingWeight
    (policy : MembershipPolicy)
    (signers : SignerSet) : VotingWeight :=
  projectedVotingWeight policy.members signers

def totalVotingWeightForMembers
    (members : List MembershipEntry) : VotingWeight :=
  members.foldr
    (fun member total => member.votingWeight + total)
    0

def totalVotingWeight (policy : MembershipPolicy) : VotingWeight :=
  totalVotingWeightForMembers policy.members

def AvailableDomainWitnessValid
    (policy : MembershipPolicy)
    (domains : List FailureDomainId) : Prop :=
  domains.Nodup ∧
  ∀ domain, domain ∈ domains →
    ∃ member,
      member ∈ policy.members ∧
      member.failureDomainId = domain

def MembershipPolicyFeasible (policy : MembershipPolicy) : Prop :=
  MembershipPolicyWellFormed policy ∧
  policy.quorumWeight ≤ totalVotingWeight policy ∧
  ∃ domains,
    AvailableDomainWitnessValid policy domains ∧
    policy.minimumFailureDomains ≤ domains.length

def SignerSetValid
    (policy : MembershipPolicy)
    (signers : SignerSet) : Prop :=
  signers.Nodup ∧
  ∀ signer, signer ∈ signers →
    ∃ member, member ∈ policy.members ∧ member.nodeId = signer

def FailureDomainWitnessValid
    (policy : MembershipPolicy)
    (signers : SignerSet)
    (domains : List FailureDomainId) : Prop :=
  domains.Nodup ∧
  ∀ domain, domain ∈ domains →
    ∃ member,
      member ∈ policy.members ∧
      member.nodeId ∈ signers ∧
      member.failureDomainId = domain

def RealizesQuorum
    (policy : MembershipPolicy)
    (signers : SignerSet)
    (domains : List FailureDomainId) : Prop :=
  SignerSetValid policy signers ∧
  policy.quorumWeight ≤ realizedVotingWeight policy signers ∧
  FailureDomainWitnessValid policy signers domains ∧
  policy.minimumFailureDomains ≤ domains.length

structure QuorumCertificate
    (scheme : CanonicalMembershipScheme) where
  committedMembership : scheme.Digest
  signers : SignerSet
  failureDomains : List FailureDomainId

def AcceptsQuorumCertificate
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme) : Prop :=
  MembershipPolicyFeasible policy ∧
  certificate.committedMembership = membershipDigest scheme policy ∧
  RealizesQuorum
    policy certificate.signers certificate.failureDomains

theorem accepted_certificate_realizes_weight_threshold
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (accepted : AcceptsQuorumCertificate scheme policy certificate) :
    policy.quorumWeight ≤
      realizedVotingWeight policy certificate.signers :=
  accepted.2.2.2.1

theorem accepted_certificate_realizes_failure_domain_threshold
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (accepted : AcceptsQuorumCertificate scheme policy certificate) :
    policy.minimumFailureDomains ≤
      certificate.failureDomains.length :=
  accepted.2.2.2.2.2

theorem insufficient_voting_weight_rejects_certificate
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (insufficient :
      realizedVotingWeight policy certificate.signers <
        policy.quorumWeight) :
    ¬ AcceptsQuorumCertificate scheme policy certificate := by
  intro accepted
  exact Nat.not_le_of_lt insufficient
    (accepted_certificate_realizes_weight_threshold
      scheme policy certificate accepted)

theorem insufficient_failure_domain_coverage_rejects_certificate
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (insufficient :
      certificate.failureDomains.length <
        policy.minimumFailureDomains) :
    ¬ AcceptsQuorumCertificate scheme policy certificate := by
  intro accepted
  exact Nat.not_le_of_lt insufficient
    (accepted_certificate_realizes_failure_domain_threshold
      scheme policy certificate accepted)

theorem unknown_signer_rejects_certificate
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (signer : NodeId)
    (signed : signer ∈ certificate.signers)
    (unknown :
      ¬ ∃ member, member ∈ policy.members ∧ member.nodeId = signer) :
    ¬ AcceptsQuorumCertificate scheme policy certificate := by
  intro accepted
  exact unknown (accepted.2.2.1.2 signer signed)

theorem duplicate_signer_rejects_certificate
    (scheme : CanonicalMembershipScheme)
    (policy : MembershipPolicy)
    (certificate : QuorumCertificate scheme)
    (duplicate : ¬ certificate.signers.Nodup) :
    ¬ AcceptsQuorumCertificate scheme policy certificate := by
  intro accepted
  exact duplicate accepted.2.2.1.1

def infeasiblePolicy : MembershipPolicy where
  members := [{
    nodeId := 1
    votingWeight := 1
    failureDomainId := 1
  }]
  quorumWeight := 2
  minimumFailureDomains := 1

theorem canonical_membership_well_formedness_does_not_imply_feasibility :
    MembershipPolicyWellFormed infeasiblePolicy ∧
    ¬ MembershipPolicyFeasible infeasiblePolicy := by
  constructor
  · constructor
    · change [1].Nodup
      apply List.Pairwise.cons
      · intro other impossible
        cases impossible
      · exact List.Pairwise.nil
    · constructor
      · intro member present
        change member ∈ [{
          nodeId := 1
          votingWeight := 1
          failureDomainId := 1
        }] at present
        cases present with
        | head => exact Nat.zero_lt_succ 0
        | tail _ impossible => cases impossible
      · exact ⟨Nat.zero_lt_succ 1, Nat.zero_lt_succ 0⟩
  · intro feasible
    have threshold := feasible.2.1
    change 2 ≤ 1 at threshold
    exact (Nat.not_succ_le_self 1) threshold

end ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization
