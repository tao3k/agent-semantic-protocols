-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAdmissionGcPublication

namespace ASPProof.SearchRouteAdmissionLedgerOrder

universe u v

/--
A ledger stamp is the ordering authority for an admission-ledger event.
Wall-clock time may be retained as diagnostic metadata, but it is not part of
the ordering relation.
-/
structure LedgerStamp (Identity : Type u) where
  ledgerIdentity : Identity
  ledgerGeneration : Nat
  sequence : Nat
deriving DecidableEq, Repr

def SameLedgerGeneration
    {Identity : Type u}
    (left right : LedgerStamp Identity) : Prop :=
  left.ledgerIdentity = right.ledgerIdentity
    ∧ left.ledgerGeneration = right.ledgerGeneration

def HappensBefore
    {Identity : Type u}
    (left right : LedgerStamp Identity) : Prop :=
  SameLedgerGeneration left right ∧ left.sequence < right.sequence

theorem happens_before_trans
    {Identity : Type u}
    {first second third : LedgerStamp Identity}
    (firstBeforeSecond : HappensBefore first second)
    (secondBeforeThird : HappensBefore second third) :
    HappensBefore first third := by
  rcases firstBeforeSecond with ⟨⟨identity₁₂, generation₁₂⟩, sequence₁₂⟩
  rcases secondBeforeThird with ⟨⟨identity₂₃, generation₂₃⟩, sequence₂₃⟩
  exact
    ⟨⟨identity₁₂.trans identity₂₃, generation₁₂.trans generation₂₃⟩,
      Nat.lt_trans sequence₁₂ sequence₂₃⟩

theorem happens_before_irreflexive
    {Identity : Type u}
    (stamp : LedgerStamp Identity) :
    ¬ HappensBefore stamp stamp := by
  intro beforeItself
  exact (Nat.lt_irrefl stamp.sequence) beforeItself.2

/--
Verifier soundness ties acceptance to the ledger's authoritative issue log.
Merely naming an implementation as the authorized verifier is insufficient.
-/
def VerifierSound
    {Token : Type v}
    (verifyToken issuedByLedger : Token → Prop) : Prop :=
  ∀ token, verifyToken token → issuedByLedger token

structure StampedToken
    (Token : Type v)
    (Identity : Type u) where
  token : Token
  stamp : LedgerStamp Identity

structure StampedPublication
    (Identity : Type u) where
  stamp : LedgerStamp Identity

/--
A publication certificate couples token authenticity with ledger-owned
happens-before evidence. Both tokens must be issued by the same ledger
generation and strictly precede the publication sequence.
-/
structure SequencedPublicationCertificate
    {Token : Type v}
    {Identity : Type u}
    (verifyToken issuedByLedger : Token → Prop)
    (baselineToken extensionToken : StampedToken Token Identity)
    (publication : StampedPublication Identity) : Prop where
  verifierSound : VerifierSound verifyToken issuedByLedger
  baselineVerified : verifyToken baselineToken.token
  extensionVerified : verifyToken extensionToken.token
  baselineBeforePublication :
    HappensBefore baselineToken.stamp publication.stamp
  extensionBeforePublication :
    HappensBefore extensionToken.stamp publication.stamp

theorem sequenced_certificate_tokens_are_authoritatively_issued
    {Token : Type v}
    {Identity : Type u}
    {verifyToken issuedByLedger : Token → Prop}
    {baselineToken extensionToken : StampedToken Token Identity}
    {publication : StampedPublication Identity}
    (certificate :
      SequencedPublicationCertificate
        verifyToken
        issuedByLedger
        baselineToken
        extensionToken
        publication) :
    issuedByLedger baselineToken.token
      ∧ issuedByLedger extensionToken.token := by
  exact
    ⟨certificate.verifierSound
        baselineToken.token
        certificate.baselineVerified,
      certificate.verifierSound
        extensionToken.token
        certificate.extensionVerified⟩

theorem sequenced_certificate_orders_both_tokens_before_publication
    {Token : Type v}
    {Identity : Type u}
    {verifyToken issuedByLedger : Token → Prop}
    {baselineToken extensionToken : StampedToken Token Identity}
    {publication : StampedPublication Identity}
    (certificate :
      SequencedPublicationCertificate
        verifyToken
        issuedByLedger
        baselineToken
        extensionToken
        publication) :
    HappensBefore baselineToken.stamp publication.stamp
      ∧ HappensBefore extensionToken.stamp publication.stamp := by
  exact
    ⟨certificate.baselineBeforePublication,
      certificate.extensionBeforePublication⟩

structure TimedLedgerEvent
    (Identity : Type u) where
  stamp : LedgerStamp Identity
  wallTime : Nat

def wallClockTokenEvent : TimedLedgerEvent Nat :=
  { stamp :=
      { ledgerIdentity := 7
        ledgerGeneration := 2
        sequence := 2 }
    wallTime := 0 }

def wallClockPublicationEvent : TimedLedgerEvent Nat :=
  { stamp :=
      { ledgerIdentity := 7
        ledgerGeneration := 2
        sequence := 1 }
    wallTime := 1 }

theorem wall_clock_order_does_not_imply_ledger_order :
    wallClockTokenEvent.wallTime < wallClockPublicationEvent.wallTime
      ∧ ¬ HappensBefore
        wallClockTokenEvent.stamp
        wallClockPublicationEvent.stamp := by
  simp
    [wallClockTokenEvent,
      wallClockPublicationEvent,
      HappensBefore,
      SameLedgerGeneration]

def previousGenerationTokenStamp : LedgerStamp Nat :=
  { ledgerIdentity := 11
    ledgerGeneration := 0
    sequence := 0 }

def currentGenerationPublicationStamp : LedgerStamp Nat :=
  { ledgerIdentity := 11
    ledgerGeneration := 1
    sequence := 1 }

theorem previous_generation_sequence_does_not_imply_happens_before :
    previousGenerationTokenStamp.sequence
        < currentGenerationPublicationStamp.sequence
      ∧ ¬ HappensBefore
        previousGenerationTokenStamp
        currentGenerationPublicationStamp := by
  simp
    [previousGenerationTokenStamp,
      currentGenerationPublicationStamp,
      HappensBefore,
      SameLedgerGeneration]

def allowAll (_token : Bool) : Prop :=
  True

def issuedNone (_token : Bool) : Prop :=
  False

theorem permissive_verifier_does_not_establish_authenticity :
    allowAll true
      ∧ ¬ issuedNone true
      ∧ ¬ VerifierSound allowAll issuedNone := by
  simp [allowAll, issuedNone, VerifierSound]

end ASPProof.SearchRouteAdmissionLedgerOrder
