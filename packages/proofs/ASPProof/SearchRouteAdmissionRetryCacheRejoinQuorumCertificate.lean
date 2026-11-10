-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate

open ASPProof.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

structure QuorumSystem where
  Node : Type
  isQuorum : List Node → Prop
  intersects :
    ∀ left right,
      isQuorum left →
      isQuorum right →
      ∃ node, node ∈ left ∧ node ∈ right

abbrev LogIndex := Nat

abbrev VoteLedger (system : QuorumSystem) (PolicyDigest : Type) :=
  system.Node →
  ConsensusTerm →
  LogIndex →
  Option (FenceCommitReceipt PolicyDigest)

structure QuorumCertificate
    (system : QuorumSystem)
    (PolicyDigest : Type) where
  term : ConsensusTerm
  index : LogIndex
  receipt : FenceCommitReceipt PolicyDigest
  signers : List system.Node

def CertificateValid
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (certificate : QuorumCertificate system PolicyDigest) : Prop :=
  system.isQuorum certificate.signers ∧
  ∀ node,
    node ∈ certificate.signers →
    ledger node certificate.term certificate.index =
      some certificate.receipt

theorem valid_same_slot_certificates_bind_same_receipt
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (left right : QuorumCertificate system PolicyDigest)
    (sameTerm : left.term = right.term)
    (sameIndex : left.index = right.index)
    (leftValid : CertificateValid system ledger left)
    (rightValid : CertificateValid system ledger right) :
    left.receipt = right.receipt := by
  rcases system.intersects
      left.signers right.signers leftValid.1 rightValid.1 with
    ⟨node, inLeft, inRight⟩
  have leftVote := leftValid.2 node inLeft
  have rightVote := rightValid.2 node inRight
  rw [← sameTerm, ← sameIndex, leftVote] at rightVote
  exact Option.some.inj rightVote

theorem nonquorum_certificate_is_not_valid
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (certificate : QuorumCertificate system PolicyDigest)
    (notQuorum : ¬ system.isQuorum certificate.signers) :
    ¬ CertificateValid system ledger certificate := by
  intro valid
  exact notQuorum valid.1

theorem inconsistent_signer_vote_invalidates_certificate
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (certificate : QuorumCertificate system PolicyDigest)
    (node : system.Node)
    (signed : node ∈ certificate.signers)
    (inconsistent :
      ledger node certificate.term certificate.index ≠
        some certificate.receipt) :
    ¬ CertificateValid system ledger certificate := by
  intro valid
  exact inconsistent (valid.2 node signed)

def RecoverableCommittedReceipt
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (term : ConsensusTerm)
    (index : LogIndex)
    (receipt : FenceCommitReceipt PolicyDigest) : Prop :=
  ∃ certificate : QuorumCertificate system PolicyDigest,
    certificate.term = term ∧
    certificate.index = index ∧
    certificate.receipt = receipt ∧
    CertificateValid system ledger certificate

theorem valid_certificate_makes_receipt_recoverable
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (certificate : QuorumCertificate system PolicyDigest)
    (valid : CertificateValid system ledger certificate) :
    RecoverableCommittedReceipt
      system ledger certificate.term certificate.index certificate.receipt := by
  exact ⟨certificate, rfl, rfl, rfl, valid⟩

theorem recoverable_same_slot_receipts_are_equal
    (system : QuorumSystem)
    (ledger : VoteLedger system PolicyDigest)
    (term : ConsensusTerm)
    (index : LogIndex)
    (leftReceipt rightReceipt : FenceCommitReceipt PolicyDigest)
    (leftRecoverable :
      RecoverableCommittedReceipt system ledger term index leftReceipt)
    (rightRecoverable :
      RecoverableCommittedReceipt system ledger term index rightReceipt) :
    leftReceipt = rightReceipt := by
  rcases leftRecoverable with
    ⟨left, leftTerm, leftIndex, leftReceiptEq, leftValid⟩
  rcases rightRecoverable with
    ⟨right, rightTerm, rightIndex, rightReceiptEq, rightValid⟩
  subst leftReceipt
  subst rightReceipt
  exact valid_same_slot_certificates_bind_same_receipt
    system ledger left right
    (leftTerm.trans rightTerm.symm)
    (leftIndex.trans rightIndex.symm)
    leftValid rightValid

end ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumCertificate
