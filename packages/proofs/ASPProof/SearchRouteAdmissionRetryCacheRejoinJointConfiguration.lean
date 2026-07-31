import ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration

open ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

structure ReconfigurationSystem where
  Node : Type
  oldQuorum : List Node → Prop
  newQuorum : List Node → Prop
  oldIntersects :
    ∀ left right, oldQuorum left → oldQuorum right →
      ∃ node, node ∈ left ∧ node ∈ right
  newIntersects :
    ∀ left right, newQuorum left → newQuorum right →
      ∃ node, node ∈ left ∧ node ∈ right

abbrev ReconfigurationVoteLedger
    (system : ReconfigurationSystem) (PolicyDigest : Type) :=
  system.Node → ConsensusTerm → Nat →
    Option (FenceCommitReceipt PolicyDigest)

structure ConfigurationCertificate
    (system : ReconfigurationSystem) (PolicyDigest : Type) where
  term : ConsensusTerm
  index : Nat
  receipt : FenceCommitReceipt PolicyDigest
  signers : List system.Node

def CertificateVotesMatch
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : ConfigurationCertificate system PolicyDigest) : Prop :=
  ∀ node, node ∈ certificate.signers →
    ledger node certificate.term certificate.index =
      some certificate.receipt

def OldCertificateValid
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : ConfigurationCertificate system PolicyDigest) : Prop :=
  system.oldQuorum certificate.signers ∧
  CertificateVotesMatch ledger certificate

def NewCertificateValid
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : ConfigurationCertificate system PolicyDigest) : Prop :=
  system.newQuorum certificate.signers ∧
  CertificateVotesMatch ledger certificate

structure JointConfigurationCertificate
    (system : ReconfigurationSystem) (PolicyDigest : Type) where
  term : ConsensusTerm
  index : Nat
  receipt : FenceCommitReceipt PolicyDigest
  oldSigners : List system.Node
  newSigners : List system.Node

def JointCertificateValid
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : JointConfigurationCertificate system PolicyDigest) : Prop :=
  system.oldQuorum certificate.oldSigners ∧
  system.newQuorum certificate.newSigners ∧
  (∀ node, node ∈ certificate.oldSigners →
    ledger node certificate.term certificate.index =
      some certificate.receipt) ∧
  (∀ node, node ∈ certificate.newSigners →
    ledger node certificate.term certificate.index =
      some certificate.receipt)

theorem old_certificate_and_joint_certificate_same_slot_agree
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (old : ConfigurationCertificate system PolicyDigest)
    (joint : JointConfigurationCertificate system PolicyDigest)
    (sameTerm : old.term = joint.term)
    (sameIndex : old.index = joint.index)
    (oldValid : OldCertificateValid system ledger old)
    (jointValid : JointCertificateValid system ledger joint) :
    old.receipt = joint.receipt := by
  rcases system.oldIntersects
      old.signers joint.oldSigners oldValid.1 jointValid.1 with
    ⟨node, inOld, inJoint⟩
  have oldVote := oldValid.2 node inOld
  have jointVote := jointValid.2.2.1 node inJoint
  rw [← sameTerm, ← sameIndex, oldVote] at jointVote
  exact Option.some.inj jointVote

theorem new_certificate_and_joint_certificate_same_slot_agree
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (new : ConfigurationCertificate system PolicyDigest)
    (joint : JointConfigurationCertificate system PolicyDigest)
    (sameTerm : new.term = joint.term)
    (sameIndex : new.index = joint.index)
    (newValid : NewCertificateValid system ledger new)
    (jointValid : JointCertificateValid system ledger joint) :
    new.receipt = joint.receipt := by
  rcases system.newIntersects
      new.signers joint.newSigners newValid.1 jointValid.2.1 with
    ⟨node, inNew, inJoint⟩
  have newVote := newValid.2 node inNew
  have jointVote := jointValid.2.2.2 node inJoint
  rw [← sameTerm, ← sameIndex, newVote] at jointVote
  exact Option.some.inj jointVote

theorem missing_new_quorum_cannot_finalize_reconfiguration
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : JointConfigurationCertificate system PolicyDigest)
    (missing : ¬ system.newQuorum certificate.newSigners) :
    ¬ JointCertificateValid system ledger certificate := by
  intro valid
  exact missing valid.2.1

theorem missing_old_quorum_cannot_finalize_reconfiguration
    (system : ReconfigurationSystem)
    (ledger : ReconfigurationVoteLedger system PolicyDigest)
    (certificate : JointConfigurationCertificate system PolicyDigest)
    (missing : ¬ system.oldQuorum certificate.oldSigners) :
    ¬ JointCertificateValid system ledger certificate := by
  intro valid
  exact missing valid.1

end ASPProof.SearchRouteAdmissionRetryCacheRejoinJointConfiguration
