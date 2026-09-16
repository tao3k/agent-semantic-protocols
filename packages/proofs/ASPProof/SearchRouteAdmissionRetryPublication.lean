-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionRetryRootBarrier

namespace ASPProof.SearchRouteAdmissionRetryPublication

universe u v

/--
Certificate durability, publication durability, and acknowledgment durability
are separate crash-recovery facts. Visibility is owned by publication
durability, never by the acknowledgment channel.
-/
structure RetryPublicationState
    (RetryKey : Type u)
    (Certificate : Type v) where
  retryKey : RetryKey
  durableCertificate : Option Certificate
  publicationDurable : Bool
  acknowledgmentDurable : Bool
deriving DecidableEq, Repr

def DurableValidCertificate
    {RetryKey : Type u}
    {Certificate : Type v}
    (validate : Certificate → Prop)
    (state : RetryPublicationState RetryKey Certificate) : Prop :=
  ∃ certificate,
    state.durableCertificate = some certificate
      ∧ validate certificate

def Visible
    {RetryKey : Type u}
    {Certificate : Type v}
    (state : RetryPublicationState RetryKey Certificate) : Prop :=
  state.publicationDurable = true

def Acknowledged
    {RetryKey : Type u}
    {Certificate : Type v}
    (state : RetryPublicationState RetryKey Certificate) : Prop :=
  state.acknowledgmentDurable = true

def WellFormed
    {RetryKey : Type u}
    {Certificate : Type v}
    (validate : Certificate → Prop)
    (state : RetryPublicationState RetryKey Certificate) : Prop :=
  (Visible state → DurableValidCertificate validate state)
    ∧ (Acknowledged state → Visible state)

theorem visible_retry_has_durable_valid_certificate
    {RetryKey : Type u}
    {Certificate : Type v}
    {validate : Certificate → Prop}
    {state : RetryPublicationState RetryKey Certificate}
    (wellFormed : WellFormed validate state)
    (visible : Visible state) :
    DurableValidCertificate validate state :=
  wellFormed.1 visible

theorem durable_acknowledgment_requires_visible_publication
    {RetryKey : Type u}
    {Certificate : Type v}
    {validate : Certificate → Prop}
    {state : RetryPublicationState RetryKey Certificate}
    (wellFormed : WellFormed validate state)
    (acknowledged : Acknowledged state) :
    Visible state :=
  wellFormed.2 acknowledged

def validateUnitCertificate (_certificate : Unit) : Prop :=
  True

def beforeCertificateState : RetryPublicationState Nat Unit :=
  { retryKey := 7
    durableCertificate := none
    publicationDurable := false
    acknowledgmentDurable := false }

def certificateDurableState : RetryPublicationState Nat Unit :=
  { retryKey := 7
    durableCertificate := some ()
    publicationDurable := false
    acknowledgmentDurable := false }

def publicationDurableAcknowledgmentLostState :
    RetryPublicationState Nat Unit :=
  { retryKey := 7
    durableCertificate := some ()
    publicationDurable := true
    acknowledgmentDurable := false }

def publicationWithoutCertificateState :
    RetryPublicationState Nat Unit :=
  { retryKey := 7
    durableCertificate := none
    publicationDurable := true
    acknowledgmentDurable := false }

def acknowledgmentWithoutPublicationState :
    RetryPublicationState Nat Unit :=
  { retryKey := 7
    durableCertificate := some ()
    publicationDurable := false
    acknowledgmentDurable := true }

theorem crash_before_certificate_is_unpublished_and_unrecoverable :
    WellFormed validateUnitCertificate beforeCertificateState
      ∧ ¬ DurableValidCertificate
        validateUnitCertificate
        beforeCertificateState
      ∧ ¬ Visible beforeCertificateState
      ∧ ¬ Acknowledged beforeCertificateState := by
  simp
    [WellFormed,
      DurableValidCertificate,
      Visible,
      Acknowledged,
      validateUnitCertificate,
      beforeCertificateState]

theorem durable_certificate_before_publication_is_recoverable_but_invisible :
    WellFormed validateUnitCertificate certificateDurableState
      ∧ DurableValidCertificate
        validateUnitCertificate
        certificateDurableState
      ∧ ¬ Visible certificateDurableState
      ∧ ¬ Acknowledged certificateDurableState := by
  simp
    [WellFormed,
      DurableValidCertificate,
      Visible,
      Acknowledged,
      validateUnitCertificate,
      certificateDurableState]

theorem lost_acknowledgment_does_not_revoke_durable_publication :
    WellFormed
        validateUnitCertificate
        publicationDurableAcknowledgmentLostState
      ∧ Visible publicationDurableAcknowledgmentLostState
      ∧ ¬ Acknowledged publicationDurableAcknowledgmentLostState := by
  simp
    [WellFormed,
      DurableValidCertificate,
      Visible,
      Acknowledged,
      validateUnitCertificate,
      publicationDurableAcknowledgmentLostState]

theorem publication_without_certificate_is_not_well_formed :
    Visible publicationWithoutCertificateState
      ∧ ¬ WellFormed
        validateUnitCertificate
        publicationWithoutCertificateState := by
  simp
    [WellFormed,
      DurableValidCertificate,
      Visible,
      Acknowledged,
      validateUnitCertificate,
      publicationWithoutCertificateState]

theorem acknowledgment_without_publication_is_not_well_formed :
    Acknowledged acknowledgmentWithoutPublicationState
      ∧ ¬ WellFormed
        validateUnitCertificate
        acknowledgmentWithoutPublicationState := by
  simp
    [WellFormed,
      DurableValidCertificate,
      Visible,
      Acknowledged,
      validateUnitCertificate,
      acknowledgmentWithoutPublicationState]

end ASPProof.SearchRouteAdmissionRetryPublication
