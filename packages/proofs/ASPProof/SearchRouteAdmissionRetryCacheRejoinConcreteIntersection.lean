import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

def intersectionVotingWeightForMembers
    (members : List MembershipEntry)
    (leftSigners rightSigners : SignerSet) : VotingWeight :=
  members.foldr
    (fun member total =>
      if signerSelected leftSigners member.nodeId &&
          signerSelected rightSigners member.nodeId
      then member.votingWeight + total
      else total)
    0

def intersectionVotingWeight
    (policy : MembershipPolicy)
    (leftSigners rightSigners : SignerSet) : VotingWeight :=
  intersectionVotingWeightForMembers
    policy.members leftSigners rightSigners

theorem signer_selection_ignores_duplicate_head
    (nodeId candidate : NodeId)
    (rest : SignerSet) :
    signerSelected (nodeId :: nodeId :: rest) candidate =
      signerSelected (nodeId :: rest) candidate := by
  unfold signerSelected
  change
    ((nodeId == candidate) ||
      ((nodeId == candidate) ||
        rest.any (fun signer => signer == candidate))) =
      ((nodeId == candidate) ||
        rest.any (fun signer => signer == candidate))
  cases selected : (nodeId == candidate)
  · rfl
  · rfl

theorem projected_weight_ignores_duplicate_signer_head
    (members : List MembershipEntry)
    (nodeId : NodeId)
    (rest : SignerSet) :
    projectedVotingWeight members (nodeId :: nodeId :: rest) =
      projectedVotingWeight members (nodeId :: rest) := by
  induction members with
  | nil => rfl
  | cons member tail inductionHypothesis =>
      change
        (if signerSelected (nodeId :: nodeId :: rest) member.nodeId
          then member.votingWeight +
            projectedVotingWeight tail (nodeId :: nodeId :: rest)
          else projectedVotingWeight tail (nodeId :: nodeId :: rest)) =
        (if signerSelected (nodeId :: rest) member.nodeId
          then member.votingWeight +
            projectedVotingWeight tail (nodeId :: rest)
          else projectedVotingWeight tail (nodeId :: rest))
      rw [signer_selection_ignores_duplicate_head]
      rw [inductionHypothesis]

theorem intersection_weight_ignores_duplicate_left_signer_head
    (members : List MembershipEntry)
    (nodeId : NodeId)
    (leftRest rightSigners : SignerSet) :
    intersectionVotingWeightForMembers
        members (nodeId :: nodeId :: leftRest) rightSigners =
      intersectionVotingWeightForMembers
        members (nodeId :: leftRest) rightSigners := by
  induction members with
  | nil => rfl
  | cons member tail inductionHypothesis =>
      change
        (if signerSelected (nodeId :: nodeId :: leftRest) member.nodeId &&
              signerSelected rightSigners member.nodeId
          then member.votingWeight +
            intersectionVotingWeightForMembers
              tail (nodeId :: nodeId :: leftRest) rightSigners
          else intersectionVotingWeightForMembers
            tail (nodeId :: nodeId :: leftRest) rightSigners) =
        (if signerSelected (nodeId :: leftRest) member.nodeId &&
              signerSelected rightSigners member.nodeId
          then member.votingWeight +
            intersectionVotingWeightForMembers
              tail (nodeId :: leftRest) rightSigners
          else intersectionVotingWeightForMembers
            tail (nodeId :: leftRest) rightSigners)
      rw [signer_selection_ignores_duplicate_head]
      rw [inductionHypothesis]

theorem intersection_weight_is_bounded_by_total
    (members : List MembershipEntry)
    (leftSigners rightSigners : SignerSet) :
    intersectionVotingWeightForMembers
        members leftSigners rightSigners ≤
      totalVotingWeightForMembers members := by
  induction members with
  | nil => exact Nat.le_refl 0
  | cons member tail inductionHypothesis =>
      change
        (if signerSelected leftSigners member.nodeId &&
              signerSelected rightSigners member.nodeId
          then member.votingWeight +
            intersectionVotingWeightForMembers
              tail leftSigners rightSigners
          else intersectionVotingWeightForMembers
            tail leftSigners rightSigners) ≤
        member.votingWeight + totalVotingWeightForMembers tail
      cases leftSelected : signerSelected leftSigners member.nodeId <;>
        cases rightSelected : signerSelected rightSigners member.nodeId
      · change
          intersectionVotingWeightForMembers
              tail leftSigners rightSigners ≤
            member.votingWeight + totalVotingWeightForMembers tail
        exact Nat.le_trans inductionHypothesis
          (Nat.le_add_left _ _)
      · change
          intersectionVotingWeightForMembers
              tail leftSigners rightSigners ≤
            member.votingWeight + totalVotingWeightForMembers tail
        exact Nat.le_trans inductionHypothesis
          (Nat.le_add_left _ _)
      · change
          intersectionVotingWeightForMembers
              tail leftSigners rightSigners ≤
            member.votingWeight + totalVotingWeightForMembers tail
        exact Nat.le_trans inductionHypothesis
          (Nat.le_add_left _ _)
      · change
          member.votingWeight +
              intersectionVotingWeightForMembers
                tail leftSigners rightSigners ≤
            member.votingWeight + totalVotingWeightForMembers tail
        exact Nat.add_le_add_left
          inductionHypothesis member.votingWeight

theorem concrete_intersection_satisfies_inclusion_exclusion
    (members : List MembershipEntry)
    (leftSigners rightSigners : SignerSet) :
    projectedVotingWeight members leftSigners +
        projectedVotingWeight members rightSigners ≤
      totalVotingWeightForMembers members +
        intersectionVotingWeightForMembers
          members leftSigners rightSigners := by
  induction members with
  | nil => exact Nat.le_refl 0
  | cons member tail inductionHypothesis =>
      change
        (if signerSelected leftSigners member.nodeId
          then member.votingWeight +
            projectedVotingWeight tail leftSigners
          else projectedVotingWeight tail leftSigners) +
        (if signerSelected rightSigners member.nodeId
          then member.votingWeight +
            projectedVotingWeight tail rightSigners
          else projectedVotingWeight tail rightSigners) ≤
        member.votingWeight + totalVotingWeightForMembers tail +
        (if signerSelected leftSigners member.nodeId &&
              signerSelected rightSigners member.nodeId
          then member.votingWeight +
            intersectionVotingWeightForMembers
              tail leftSigners rightSigners
          else intersectionVotingWeightForMembers
            tail leftSigners rightSigners)
      cases leftSelected : signerSelected leftSigners member.nodeId <;>
        cases rightSelected : signerSelected rightSigners member.nodeId
      · change
          projectedVotingWeight tail leftSigners +
              projectedVotingWeight tail rightSigners ≤
            member.votingWeight +
              totalVotingWeightForMembers tail +
                intersectionVotingWeightForMembers
                  tail leftSigners rightSigners
        exact Nat.le_trans inductionHypothesis (by
          exact Nat.add_le_add_right
            (Nat.le_add_left
              (totalVotingWeightForMembers tail)
              member.votingWeight)
            (intersectionVotingWeightForMembers
              tail leftSigners rightSigners))
      · change
          projectedVotingWeight tail leftSigners +
              (member.votingWeight +
                projectedVotingWeight tail rightSigners) ≤
            member.votingWeight +
              totalVotingWeightForMembers tail +
                intersectionVotingWeightForMembers
                  tail leftSigners rightSigners
        calc
          projectedVotingWeight tail leftSigners +
                (member.votingWeight +
                  projectedVotingWeight tail rightSigners) =
              member.votingWeight +
                (projectedVotingWeight tail leftSigners +
                  projectedVotingWeight tail rightSigners) :=
            Nat.add_left_comm _ _ _
          _ ≤ member.votingWeight +
                (totalVotingWeightForMembers tail +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners) :=
            Nat.add_le_add_left inductionHypothesis member.votingWeight
          _ = member.votingWeight +
                totalVotingWeightForMembers tail +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners :=
            (Nat.add_assoc _ _ _).symm
      · change
          member.votingWeight +
              projectedVotingWeight tail leftSigners +
                projectedVotingWeight tail rightSigners ≤
            member.votingWeight +
              totalVotingWeightForMembers tail +
                intersectionVotingWeightForMembers
                  tail leftSigners rightSigners
        calc
          member.votingWeight +
                projectedVotingWeight tail leftSigners +
                  projectedVotingWeight tail rightSigners =
              member.votingWeight +
                (projectedVotingWeight tail leftSigners +
                  projectedVotingWeight tail rightSigners) :=
            Nat.add_assoc _ _ _
          _ ≤ member.votingWeight +
                (totalVotingWeightForMembers tail +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners) :=
            Nat.add_le_add_left inductionHypothesis member.votingWeight
          _ = member.votingWeight +
                totalVotingWeightForMembers tail +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners :=
            (Nat.add_assoc _ _ _).symm
      · change
          member.votingWeight +
              projectedVotingWeight tail leftSigners +
              (member.votingWeight +
                projectedVotingWeight tail rightSigners) ≤
            member.votingWeight +
              totalVotingWeightForMembers tail +
              (member.votingWeight +
                intersectionVotingWeightForMembers
                  tail leftSigners rightSigners)
        calc
          member.votingWeight +
                projectedVotingWeight tail leftSigners +
                (member.votingWeight +
                  projectedVotingWeight tail rightSigners) =
              (member.votingWeight + member.votingWeight) +
                (projectedVotingWeight tail leftSigners +
                  projectedVotingWeight tail rightSigners) := by
            calc
              member.votingWeight +
                    projectedVotingWeight tail leftSigners +
                    (member.votingWeight +
                      projectedVotingWeight tail rightSigners) =
                  member.votingWeight +
                    (projectedVotingWeight tail leftSigners +
                      (member.votingWeight +
                        projectedVotingWeight tail rightSigners)) :=
                Nat.add_assoc _ _ _
              _ = member.votingWeight +
                    (member.votingWeight +
                      (projectedVotingWeight tail leftSigners +
                        projectedVotingWeight tail rightSigners)) :=
                congrArg
                  (fun value => member.votingWeight + value)
                  (Nat.add_left_comm _ _ _)
              _ = (member.votingWeight + member.votingWeight) +
                    (projectedVotingWeight tail leftSigners +
                      projectedVotingWeight tail rightSigners) :=
                (Nat.add_assoc _ _ _).symm
          _ ≤ (member.votingWeight + member.votingWeight) +
                (totalVotingWeightForMembers tail +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners) :=
            Nat.add_le_add_left inductionHypothesis
              (member.votingWeight + member.votingWeight)
          _ = member.votingWeight +
                totalVotingWeightForMembers tail +
                (member.votingWeight +
                  intersectionVotingWeightForMembers
                    tail leftSigners rightSigners) := by
            exact (calc
              member.votingWeight +
                    totalVotingWeightForMembers tail +
                    (member.votingWeight +
                      intersectionVotingWeightForMembers
                        tail leftSigners rightSigners) =
                  member.votingWeight +
                    (totalVotingWeightForMembers tail +
                      (member.votingWeight +
                        intersectionVotingWeightForMembers
                          tail leftSigners rightSigners)) :=
                Nat.add_assoc _ _ _
              _ = member.votingWeight +
                    (member.votingWeight +
                      (totalVotingWeightForMembers tail +
                        intersectionVotingWeightForMembers
                          tail leftSigners rightSigners)) :=
                congrArg
                  (fun value => member.votingWeight + value)
                  (Nat.add_left_comm _ _ _)
              _ = (member.votingWeight + member.votingWeight) +
                    (totalVotingWeightForMembers tail +
                      intersectionVotingWeightForMembers
                        tail leftSigners rightSigners) :=
                (Nat.add_assoc _ _ _).symm).symm

def concretePairWeightAccounting
    (policy : MembershipPolicy)
    (leftSigners rightSigners : SignerSet) :
    PairWeightAccounting policy leftSigners rightSigners where
  intersectionWeight :=
    intersectionVotingWeight policy leftSigners rightSigners
  inclusionExclusionBound := by
    exact concrete_intersection_satisfies_inclusion_exclusion
      policy.members leftSigners rightSigners

theorem concrete_pair_accounting_requires_no_external_bound
    (policy : MembershipPolicy)
    (leftSigners rightSigners : SignerSet) :
    (concretePairWeightAccounting
      policy leftSigners rightSigners).intersectionWeight =
      intersectionVotingWeight policy leftSigners rightSigners :=
  rfl

end ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection
