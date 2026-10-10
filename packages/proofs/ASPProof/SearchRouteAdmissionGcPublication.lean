-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionGcBarrier

namespace SearchRouteAdmissionGcPublication

open SearchRouteAdmissionLedger
open SearchRouteAdmissionHistory
open SearchRouteAdmissionGcBarrier

structure CycleWindow (Identity : Type) where
  cycleIdentity : Identity
  markEpoch : Nat
  sweepEpoch : Nat

structure BarrierToken (Identity : Type) where
  cycleIdentity : Identity
  receiptIdentity : Identity
  installedAt : Nat

structure RecordPublication (Identity : Type) where
  cycleIdentity : Identity
  record : AdmissionRecord Identity
  publishedAt : Nat

structure ValidBarrierToken
    {Identity : Type}
    (window : CycleWindow Identity)
    (publication : RecordPublication Identity)
    (token : BarrierToken Identity)
    (expectedReceiptIdentity : Identity) : Prop where
  tokenBoundToCycle : token.cycleIdentity = window.cycleIdentity
  publicationBoundToCycle :
    publication.cycleIdentity = window.cycleIdentity
  tokenBoundToReceipt : token.receiptIdentity = expectedReceiptIdentity
  installedAfterMark : window.markEpoch < token.installedAt
  installedBeforePublication : token.installedAt ≤ publication.publishedAt
  publicationBeforeSweep : publication.publishedAt ≤ window.sweepEpoch

theorem valid_barrier_implies_well_formed_cycle_window
    {Identity : Type}
    {window : CycleWindow Identity}
    {publication : RecordPublication Identity}
    {token : BarrierToken Identity}
    {expectedReceiptIdentity : Identity}
    (valid :
      ValidBarrierToken window publication token expectedReceiptIdentity) :
    window.markEpoch ≤ window.sweepEpoch := by
  exact Nat.le_trans
    (Nat.le_of_lt valid.installedAfterMark)
    (Nat.le_trans valid.installedBeforePublication
      valid.publicationBeforeSweep)

structure PublicationBarrierCertificate
    {Identity : Type}
    (gcCycle : GcCycle Identity)
    (window : CycleWindow Identity)
    (publication : RecordPublication Identity)
    (baselineToken extensionToken : BarrierToken Identity) : Prop where
  gcMarkMatchesWindow : gcCycle.markEpoch = window.markEpoch
  gcSweepMatchesWindow : gcCycle.sweepEpoch = window.sweepEpoch
  baselineValid :
    ValidBarrierToken window publication baselineToken
      publication.record.baselineReceiptIdentity
  extensionValid :
    ValidBarrierToken window publication extensionToken
      publication.record.extendedReceiptIdentity
  baselineRemembered :
    gcCycle.remembered publication.record.baselineReceiptIdentity
  extensionRemembered :
    gcCycle.remembered publication.record.extendedReceiptIdentity

theorem publication_certificate_orders_both_barriers_before_visibility
    {Identity : Type}
    {gcCycle : GcCycle Identity}
    {window : CycleWindow Identity}
    {publication : RecordPublication Identity}
    {baselineToken extensionToken : BarrierToken Identity}
    (certificate :
      PublicationBarrierCertificate gcCycle window publication
        baselineToken extensionToken) :
    baselineToken.installedAt ≤ publication.publishedAt
      ∧ extensionToken.installedAt ≤ publication.publishedAt
      ∧ baselineToken.cycleIdentity = publication.cycleIdentity
      ∧ extensionToken.cycleIdentity = publication.cycleIdentity := by
  exact ⟨
    certificate.baselineValid.installedBeforePublication,
    certificate.extensionValid.installedBeforePublication,
    certificate.baselineValid.tokenBoundToCycle.trans
      certificate.baselineValid.publicationBoundToCycle.symm,
    certificate.extensionValid.tokenBoundToCycle.trans
      certificate.extensionValid.publicationBoundToCycle.symm
  ⟩

theorem publication_certificate_implies_concurrent_commit_certificate
    {Identity : Type}
    {gcCycle : GcCycle Identity}
    {window : CycleWindow Identity}
    {publication : RecordPublication Identity}
    {baselineToken extensionToken : BarrierToken Identity}
    (certificate :
      PublicationBarrierCertificate gcCycle window publication
        baselineToken extensionToken) :
    ConcurrentCommitCertificate gcCycle publication.publishedAt
      publication.record := by
  constructor
  · constructor
    · rw [certificate.gcMarkMatchesWindow]
      exact Nat.lt_of_lt_of_le
        certificate.baselineValid.installedAfterMark
        certificate.baselineValid.installedBeforePublication
    · rw [certificate.gcSweepMatchesWindow]
      exact certificate.baselineValid.publicationBeforeSweep
  · exact ⟨certificate.baselineRemembered, certificate.extensionRemembered⟩

def currentCycleWindow : CycleWindow Bool where
  cycleIdentity := false
  markEpoch := 0
  sweepEpoch := 2

def currentPublication : RecordPublication Bool where
  cycleIdentity := false
  record := ⟨false, true⟩
  publishedAt := 1

def staleCycleBarrierToken : BarrierToken Bool where
  cycleIdentity := true
  receiptIdentity := false
  installedAt := 1

theorem barrier_token_from_another_cycle_is_invalid :
    ¬ValidBarrierToken currentCycleWindow currentPublication
      staleCycleBarrierToken false := by
  intro valid
  exact Bool.noConfusion valid.tokenBoundToCycle

def lateBarrierToken : BarrierToken Bool where
  cycleIdentity := false
  receiptIdentity := false
  installedAt := 2

theorem barrier_installed_after_publication_is_invalid :
    ¬ValidBarrierToken currentCycleWindow currentPublication
      lateBarrierToken false := by
  intro valid
  have ordering := valid.installedBeforePublication
  simp [lateBarrierToken, currentPublication] at ordering

end SearchRouteAdmissionGcPublication
