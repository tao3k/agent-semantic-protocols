import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

open ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority
open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

structure ConfigurationIdentity (MembershipDigest : Type) where
  epoch : Nat
  oldMembershipDigest : MembershipDigest
  newMembershipDigest : MembershipDigest

abbrev IdentityVoteLedger
    (system : ReconfigurationSystem)
    (MembershipDigest PolicyDigest : Type) :=
  system.Node → ConfigurationIdentity MembershipDigest →
    ConsensusTerm → Nat → Option (FenceCommitReceipt PolicyDigest)

structure IdentityBoundJointCertificate
    (system : ReconfigurationSystem)
    (MembershipDigest PolicyDigest : Type) where
  identity : ConfigurationIdentity MembershipDigest
  term : ConsensusTerm
  index : Nat
  receipt : FenceCommitReceipt PolicyDigest
  oldSigners : List system.Node
  newSigners : List system.Node

def IdentityBoundJointCertificateValid
    (system : ReconfigurationSystem)
    (ledger : IdentityVoteLedger system MembershipDigest PolicyDigest)
    (certificate :
      IdentityBoundJointCertificate system MembershipDigest PolicyDigest) : Prop :=
  system.oldQuorum certificate.oldSigners ∧
  system.newQuorum certificate.newSigners ∧
  (∀ node, node ∈ certificate.oldSigners →
    ledger node certificate.identity certificate.term certificate.index =
      some certificate.receipt) ∧
  (∀ node, node ∈ certificate.newSigners →
    ledger node certificate.identity certificate.term certificate.index =
      some certificate.receipt)

theorem vote_from_other_configuration_cannot_validate_certificate
    (system : ReconfigurationSystem)
    (ledger : IdentityVoteLedger system MembershipDigest PolicyDigest)
    (certificate :
      IdentityBoundJointCertificate system MembershipDigest PolicyDigest)
    (node : system.Node)
    (signed : node ∈ certificate.oldSigners)
    (wrongConfigurationVote :
      ledger node certificate.identity certificate.term certificate.index ≠
        some certificate.receipt) :
    ¬ IdentityBoundJointCertificateValid system ledger certificate := by
  intro valid
  exact wrongConfigurationVote (valid.2.2.1 node signed)

theorem valid_same_configuration_certificates_bind_same_receipt
    (system : ReconfigurationSystem)
    (ledger : IdentityVoteLedger system MembershipDigest PolicyDigest)
    (left right :
      IdentityBoundJointCertificate system MembershipDigest PolicyDigest)
    (sameIdentity : left.identity = right.identity)
    (sameTerm : left.term = right.term)
    (sameIndex : left.index = right.index)
    (leftValid : IdentityBoundJointCertificateValid system ledger left)
    (rightValid : IdentityBoundJointCertificateValid system ledger right) :
    left.receipt = right.receipt := by
  rcases system.oldIntersects
      left.oldSigners right.oldSigners leftValid.1 rightValid.1 with
    ⟨node, inLeft, inRight⟩
  have leftVote := leftValid.2.2.1 node inLeft
  have rightVote := rightValid.2.2.1 node inRight
  rw [← sameIdentity, ← sameTerm, ← sameIndex, leftVote] at rightVote
  exact Option.some.inj rightVote

end ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity
