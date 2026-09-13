-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionGcBarrier
import ASPProof.SearchRouteAdmissionIssueLogRotation

namespace ASPProof.SearchRouteAdmissionRetryRootBarrier

open SearchRouteAdmissionGcBarrier
open SearchRouteAdmissionHistory
open ASPProof.SearchRouteAdmissionIssueLogRotation

/--
Issue-log rotation must consume both roots present at mark time and roots
installed by the concurrent write barrier. External roots cover recovery and
audit horizons owned by other protocol layers.
-/
def CompleteLiveRoot
    {Identity : Type}
    (cycle : GcCycle Identity)
    (externalRoot : Identity → Prop)
    (identity : Identity) : Prop :=
  cycle.snapshotReferenced identity
    ∨ cycle.remembered identity
    ∨ externalRoot identity

def RecordCoveredBy
    {Identity : Type}
    (liveRoot : Identity → Prop)
    (record : AdmissionRecord Identity) : Prop :=
  liveRoot record.baselineReceiptIdentity
    ∧ liveRoot record.extendedReceiptIdentity

theorem concurrent_retry_barrier_enters_complete_live_roots
    {Identity : Type}
    {cycle : GcCycle Identity}
    {commitEpoch : Nat}
    {record : AdmissionRecord Identity}
    {externalRoot : Identity → Prop}
    (concurrent :
      ConcurrentCommitCertificate cycle commitEpoch record) :
    RecordCoveredBy (CompleteLiveRoot cycle externalRoot) record := by
  have remembered :=
    publication_during_sweep_requires_both_barrier_entries concurrent
  exact
    ⟨Or.inr (Or.inl remembered.1),
      Or.inr (Or.inl remembered.2)⟩

/--
A concurrent retry is admissible across issue-log rotation only when the
write-barrier evidence and the pre-rotation authoritative issue records are
present together.
-/
structure ConcurrentRetryAdmissionCertificate
    {Identity : Type}
    (cycle : GcCycle Identity)
    (commitEpoch : Nat)
    (record : AdmissionRecord Identity)
    (beforeRotation : IssueLogState Identity) : Prop where
  concurrentCommit :
    ConcurrentCommitCertificate cycle commitEpoch record
  baselineIssuedBeforeRotation :
    beforeRotation.issued record.baselineReceiptIdentity
  extensionIssuedBeforeRotation :
    beforeRotation.issued record.extendedReceiptIdentity

theorem certified_concurrent_retry_survives_safe_issue_log_rotation
    {Identity : Type}
    {cycle : GcCycle Identity}
    {commitEpoch : Nat}
    {record : AdmissionRecord Identity}
    {beforeRotation afterRotation : IssueLogState Identity}
    {externalRoot : Identity → Prop}
    (retry :
      ConcurrentRetryAdmissionCertificate
        cycle
        commitEpoch
        record
        beforeRotation)
    (rotation :
      SafeIssueLogRotation
        beforeRotation
        afterRotation
        (CompleteLiveRoot cycle externalRoot)) :
    afterRotation.issued record.baselineReceiptIdentity
      ∧ afterRotation.issued record.extendedReceiptIdentity := by
  have roots :
      RecordCoveredBy (CompleteLiveRoot cycle externalRoot) record :=
    concurrent_retry_barrier_enters_complete_live_roots retry.concurrentCommit
  exact
    ⟨rotation.liveCoverage
        record.baselineReceiptIdentity
        roots.1
        retry.baselineIssuedBeforeRotation,
      rotation.liveCoverage
        record.extendedReceiptIdentity
        roots.2
        retry.extensionIssuedBeforeRotation⟩

def rememberedRetryCycle : GcCycle Bool :=
  { markEpoch := 0
    sweepEpoch := 2
    snapshotReferenced := fun _identity => False
    remembered := fun identity => identity = true }

def noExternalRoot (_identity : Bool) : Prop :=
  False

theorem remembered_retry_is_live_but_snapshot_only_roots_drop_it :
    rememberedRetryCycle.remembered true
      ∧ CompleteLiveRoot rememberedRetryCycle noExternalRoot true
      ∧ ¬ rememberedRetryCycle.snapshotReferenced true := by
  simp [rememberedRetryCycle, CompleteLiveRoot, noExternalRoot]

theorem rotation_over_snapshot_only_roots_is_unsafe_for_remembered_retry :
    (∀ identity,
      rememberedRetryCycle.snapshotReferenced identity →
      issuedAllState.issued identity →
      publicationOnlyState.issued identity)
      ∧ CompleteLiveRoot rememberedRetryCycle noExternalRoot true
      ∧ issuedAllState.issued true
      ∧ ¬ publicationOnlyState.issued true
      ∧ ¬ SafeIssueLogRotation
        issuedAllState
        publicationOnlyState
        (CompleteLiveRoot rememberedRetryCycle noExternalRoot) := by
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · intro identity snapshot
    exact False.elim snapshot
  · simp [rememberedRetryCycle, CompleteLiveRoot, noExternalRoot]
  · simp [issuedAllState]
  · simp [publicationOnlyState]
  · intro rotation
    have retained :=
      rotation.liveCoverage
        true
        (by
          simp [rememberedRetryCycle, CompleteLiveRoot, noExternalRoot])
        (by simp [issuedAllState])
    exact (by simp [publicationOnlyState] : ¬ publicationOnlyState.issued true) retained

theorem root_membership_without_pre_rotation_issue_does_not_create_origin :
    CompleteLiveRoot rememberedRetryCycle noExternalRoot true
      ∧ SafeIssueLogRotation
        emptyIssueLog
        emptyIssueLog
        (CompleteLiveRoot rememberedRetryCycle noExternalRoot)
      ∧ ¬ emptyIssueLog.issued true := by
  refine ⟨?_, ?_, ?_⟩
  · simp [rememberedRetryCycle, CompleteLiveRoot, noExternalRoot]
  · exact
      { noFabrication := by
          intro identity issued
          exact issued
        liveCoverage := by
          intro identity _live issued
          exact issued
        revocationMonotone := by
          intro identity revoked
          exact revoked }
  · simp [emptyIssueLog]

end ASPProof.SearchRouteAdmissionRetryRootBarrier
