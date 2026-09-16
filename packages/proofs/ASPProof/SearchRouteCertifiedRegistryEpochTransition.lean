-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteCertifiedRegistryEpochTransition

abbrev Digest := Nat

structure ClockDomain where
  registrySnapshotDigest : Digest
  authorityDigest : Digest
  epoch : Nat
deriving DecidableEq, Repr

structure DecisionReceipt where
  domain : ClockDomain
  executionSequence : Nat
deriving DecidableEq, Repr

structure TransitionCertificate where
  fromDomain : ClockDomain
  toDomain : ClockDomain
  oldDomainCutoff : Nat
  certificateAuthorityDigest : Digest
  oldSnapshotArchived : Bool
deriving DecidableEq, Repr

def ValidTransition
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (certificate : TransitionCertificate) : Prop :=
  authorizedSuccessor certificate.fromDomain certificate.toDomain ∧
    certificate.certificateAuthorityDigest =
      certificate.fromDomain.authorityDigest ∧
    certificate.fromDomain.registrySnapshotDigest ≠
      certificate.toDomain.registrySnapshotDigest ∧
    certificate.fromDomain.epoch + 1 = certificate.toDomain.epoch

def CoveredOldExecution
    (certificate : TransitionCertificate)
    (receipt : DecisionReceipt) : Prop :=
  receipt.domain = certificate.fromDomain ∧
    receipt.executionSequence ≤ certificate.oldDomainCutoff

def DirectlySequenceComparable
    (leftDomain rightDomain : ClockDomain) : Prop :=
  leftDomain = rightDomain

def HistoricalAuditPreserved
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (certificate : TransitionCertificate)
    (receipt : DecisionReceipt) : Prop :=
  ValidTransition authorizedSuccessor certificate ∧
    CoveredOldExecution certificate receipt

def HistoricalReplayAvailable
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (certificate : TransitionCertificate)
    (receipt : DecisionReceipt) : Prop :=
  HistoricalAuditPreserved authorizedSuccessor certificate receipt ∧
    certificate.oldSnapshotArchived = true

def AuthorizedForNewDecision
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (certificate : TransitionCertificate)
    (receipt : DecisionReceipt) : Prop :=
  ValidTransition authorizedSuccessor certificate ∧
    receipt.domain = certificate.toDomain

def FunctionalSuccessor
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop) : Prop :=
  ∀ fromDomain leftSuccessor rightSuccessor,
    authorizedSuccessor fromDomain leftSuccessor →
    authorizedSuccessor fromDomain rightSuccessor →
    leftSuccessor = rightSuccessor

theorem valid_transition_is_registry_authorized
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    authorizedSuccessor certificate.fromDomain certificate.toDomain :=
  valid.1

theorem valid_transition_is_old_authority_signed
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    certificate.certificateAuthorityDigest =
      certificate.fromDomain.authorityDigest :=
  valid.2.1

theorem valid_transition_changes_snapshot
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    certificate.fromDomain.registrySnapshotDigest ≠
      certificate.toDomain.registrySnapshotDigest :=
  valid.2.2.1

theorem valid_transition_changes_domain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    certificate.fromDomain ≠ certificate.toDomain := by
  intro domainsEqual
  exact valid.2.2.1 (congrArg ClockDomain.registrySnapshotDigest domainsEqual)

theorem transition_does_not_create_sequence_comparability
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    ¬DirectlySequenceComparable
      certificate.fromDomain certificate.toDomain :=
  valid_transition_changes_domain valid

theorem covered_old_execution_preserves_audit
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (valid : ValidTransition authorizedSuccessor certificate)
    (covered : CoveredOldExecution certificate receipt) :
    HistoricalAuditPreserved authorizedSuccessor certificate receipt :=
  ⟨valid, covered⟩

theorem late_old_execution_is_not_covered
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (late : certificate.oldDomainCutoff < receipt.executionSequence) :
    ¬CoveredOldExecution certificate receipt := by
  intro covered
  exact (Nat.not_le_of_gt late) covered.2

theorem missing_archive_blocks_replay
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (missing : certificate.oldSnapshotArchived = false) :
    ¬HistoricalReplayAvailable authorizedSuccessor certificate receipt := by
  intro replay
  have impossible : false = true := missing.symm.trans replay.2
  cases impossible

theorem archived_snapshot_enables_replay
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (audit : HistoricalAuditPreserved authorizedSuccessor certificate receipt)
    (archived : certificate.oldSnapshotArchived = true) :
    HistoricalReplayAvailable authorizedSuccessor certificate receipt :=
  ⟨audit, archived⟩

theorem old_domain_rejected_for_new_decision
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (valid : ValidTransition authorizedSuccessor certificate)
    (oldDomain : receipt.domain = certificate.fromDomain) :
    ¬AuthorizedForNewDecision authorizedSuccessor certificate receipt := by
  intro authorized
  apply valid_transition_changes_domain valid
  calc
    certificate.fromDomain = receipt.domain := oldDomain.symm
    _ = certificate.toDomain := authorized.2

theorem successor_domain_authorizes_new_decision
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    {receipt : DecisionReceipt}
    (valid : ValidTransition authorizedSuccessor certificate)
    (newDomain : receipt.domain = certificate.toDomain) :
    AuthorizedForNewDecision authorizedSuccessor certificate receipt :=
  ⟨valid, newDomain⟩

theorem invalid_transition_authorizes_no_new_decision
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (invalid : ¬ValidTransition authorizedSuccessor certificate) :
    ∀ receipt, ¬AuthorizedForNewDecision authorizedSuccessor certificate receipt := by
  intro receipt authorized
  exact invalid authorized.1

theorem functional_registry_rejects_successor_fork
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    (functional : FunctionalSuccessor authorizedSuccessor)
    {leftCertificate rightCertificate : TransitionCertificate}
    (sameOrigin :
      leftCertificate.fromDomain = rightCertificate.fromDomain)
    (leftValid : ValidTransition authorizedSuccessor leftCertificate)
    (rightValid : ValidTransition authorizedSuccessor rightCertificate) :
    leftCertificate.toDomain = rightCertificate.toDomain := by
  apply functional leftCertificate.fromDomain
  · exact leftValid.1
  · rw [sameOrigin]
    exact rightValid.1

theorem valid_transition_is_exactly_one_epoch
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {certificate : TransitionCertificate}
    (valid : ValidTransition authorizedSuccessor certificate) :
    certificate.fromDomain.epoch + 1 = certificate.toDomain.epoch :=
  valid.2.2.2

end ASPProof.SearchRouteCertifiedRegistryEpochTransition
