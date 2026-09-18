-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteAuthorityScopedLogicalTime

abbrev Digest := Nat

structure ClockDomain where
  authorityDigest : Digest
  epoch : Nat
deriving DecidableEq, Repr

structure LogicalInstant where
  domain : ClockDomain
  sequence : Nat
deriving DecidableEq, Repr

def SameClock (left right : LogicalInstant) : Prop :=
  left.domain = right.domain

def BeforeOrAt (left right : LogicalInstant) : Prop :=
  SameClock left right ∧ left.sequence ≤ right.sequence

def StrictBefore (left right : LogicalInstant) : Prop :=
  SameClock left right ∧ left.sequence < right.sequence

def RawOrderOnly (left right : LogicalInstant) : Prop :=
  left.sequence < right.sequence

structure CertificateLifecycle where
  issuedAt : LogicalInstant
  executedAt : LogicalInstant
  revokedAt : LogicalInstant
  observedAt : LogicalInstant

def ValidAtExecution (lifecycle : CertificateLifecycle) : Prop :=
  BeforeOrAt lifecycle.issuedAt lifecycle.executedAt ∧
    StrictBefore lifecycle.executedAt lifecycle.revokedAt

def ValidForHistoricalAudit (lifecycle : CertificateLifecycle) : Prop :=
  ValidAtExecution lifecycle ∧
    BeforeOrAt lifecycle.executedAt lifecycle.observedAt

theorem before_or_at_requires_same_clock
    {left right : LogicalInstant}
    (ordered : BeforeOrAt left right) :
    SameClock left right :=
  ordered.1

theorem strict_before_requires_same_clock
    {left right : LogicalInstant}
    (ordered : StrictBefore left right) :
    SameClock left right :=
  ordered.1

theorem raw_order_does_not_establish_comparability :
    let left : LogicalInstant :=
      { domain := { authorityDigest := 1, epoch := 0 }, sequence := 7 }
    let right : LogicalInstant :=
      { domain := { authorityDigest := 2, epoch := 0 }, sequence := 10 }
    RawOrderOnly left right ∧ ¬SameClock left right := by
  simp [RawOrderOnly, SameClock]

theorem epoch_rollover_blocks_cross_epoch_order :
    let oldEpoch : LogicalInstant :=
      { domain := { authorityDigest := 1, epoch := 3 }, sequence := 7 }
    let newEpoch : LogicalInstant :=
      { domain := { authorityDigest := 1, epoch := 4 }, sequence := 10 }
    RawOrderOnly oldEpoch newEpoch ∧ ¬StrictBefore oldEpoch newEpoch := by
  simp [RawOrderOnly, StrictBefore, SameClock]

theorem valid_execution_is_authority_scoped
    {lifecycle : CertificateLifecycle}
    (valid : ValidAtExecution lifecycle) :
    lifecycle.issuedAt.domain = lifecycle.executedAt.domain ∧
      lifecycle.executedAt.domain = lifecycle.revokedAt.domain :=
  ⟨valid.1.1, valid.2.1⟩

theorem historical_audit_is_observation_scoped
    {lifecycle : CertificateLifecycle}
    (valid : ValidForHistoricalAudit lifecycle) :
    lifecycle.executedAt.domain = lifecycle.observedAt.domain :=
  valid.2.1

theorem revocation_precedes_observation_rejects_new_execution
    {revokedAt observedAt : LogicalInstant}
    (revoked : StrictBefore revokedAt observedAt) :
    ¬StrictBefore observedAt revokedAt := by
  intro reversed
  exact (Nat.not_lt_of_ge (Nat.le_of_lt revoked.2)) reversed.2

theorem coherent_lifecycle_is_valid
    (domain : ClockDomain)
    {issuedSequence executedSequence revokedSequence : Nat}
    (issuedBeforeExecution : issuedSequence ≤ executedSequence)
    (executionBeforeRevocation : executedSequence < revokedSequence) :
    ValidAtExecution {
      issuedAt := { domain := domain, sequence := issuedSequence }
      executedAt := { domain := domain, sequence := executedSequence }
      revokedAt := { domain := domain, sequence := revokedSequence }
      observedAt := { domain := domain, sequence := revokedSequence }
    } :=
  ⟨⟨rfl, issuedBeforeExecution⟩, ⟨rfl, executionBeforeRevocation⟩⟩

theorem authority_mismatch_rejects_execution
    {lifecycle : CertificateLifecycle}
    (mismatch : lifecycle.issuedAt.domain ≠ lifecycle.executedAt.domain) :
    ¬ValidAtExecution lifecycle := by
  intro valid
  exact mismatch valid.1.1

theorem observation_epoch_mismatch_rejects_audit
    {lifecycle : CertificateLifecycle}
    (mismatch : lifecycle.executedAt.domain ≠ lifecycle.observedAt.domain) :
    ¬ValidForHistoricalAudit lifecycle := by
  intro valid
  exact mismatch valid.2.1

end ASPProof.SearchRouteAuthorityScopedLogicalTime
