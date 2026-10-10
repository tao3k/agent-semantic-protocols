-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCanonicalPayloadDigestReplay

namespace ASPProof.SearchRouteSafeReplayModeNegotiation

open ASPProof.SearchRouteCanonicalPayloadDigestReplay

inductive ReplayModeTag where
  | fullField
  | digestCompressed
  | explicitFieldFallback
  deriving DecidableEq, Repr

structure CollisionFreeEvidence
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop) : Type where
  assumptionIdentity : Nat
  collisionFree : CollisionFreeOnAdmitted digest admitted

structure CanonicalEqualityEvidence
    (left right : CanonicalShortcutPayload) : Type where
  sameEncoding : canonicalEncode left = canonicalEncode right

structure DigestEqualityEvidence
    (digest : DigestFunction)
    (left right : CanonicalShortcutPayload) : Type where
  sameDigest :
    digest (canonicalEncode left) = digest (canonicalEncode right)

inductive ReplayIdentityOutcome
    (left right : CanonicalShortcutPayload) : Type where
  | acceptedFull (payloadEqual : left = right)
  | acceptedDigest
      (assumptionIdentity : Nat)
      (payloadEqual : left = right)
  | explicitFieldFallback

def outcomeMode
    {left right : CanonicalShortcutPayload} :
    ReplayIdentityOutcome left right → ReplayModeTag
  | .acceptedFull _ => .fullField
  | .acceptedDigest _ _ => .digestCompressed
  | .explicitFieldFallback => .explicitFieldFallback

def outcomeAssumptionIdentity
    {left right : CanonicalShortcutPayload} :
    ReplayIdentityOutcome left right → Option Nat
  | .acceptedFull _ => none
  | .acceptedDigest assumptionIdentity _ => some assumptionIdentity
  | .explicitFieldFallback => none

def fullFieldOrFallback
    (left right : CanonicalShortcutPayload)
    (canonicalEquality : Option (CanonicalEqualityEvidence left right)) :
    ReplayIdentityOutcome left right :=
  match canonicalEquality with
  | some sameCanonical =>
      .acceptedFull
        (full_field_replay_needs_no_digest_assumption
          left
          right
          sameCanonical.sameEncoding)
  | none => .explicitFieldFallback

def negotiateReplayIdentity
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (collisionEvidence : Option (CollisionFreeEvidence digest admitted))
    (digestEquality : Option (DigestEqualityEvidence digest left right))
    (canonicalEquality : Option (CanonicalEqualityEvidence left right)) :
    ReplayIdentityOutcome left right :=
  match collisionEvidence with
  | none => fullFieldOrFallback left right canonicalEquality
  | some collisionFree =>
      match digestEquality with
      | none => fullFieldOrFallback left right canonicalEquality
      | some sameDigest =>
          .acceptedDigest
            collisionFree.assumptionIdentity
            (admitted_digest_equality_implies_payload_equality
              digest
              admitted
              collisionFree.collisionFree
              left
              right
              leftAdmitted
              rightAdmitted
              sameDigest.sameDigest)

theorem absent_collision_evidence_never_selects_digest
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (digestEquality : Option (DigestEqualityEvidence digest left right))
    (canonicalEquality : Option (CanonicalEqualityEvidence left right)) :
    outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          none
          digestEquality
          canonicalEquality) ≠ ReplayModeTag.digestCompressed := by
  cases canonicalEquality <;> intro impossible <;> cases impossible

theorem collision_and_digest_evidence_select_digest
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (collisionEvidence : CollisionFreeEvidence digest admitted)
    (digestEquality : DigestEqualityEvidence digest left right)
    (canonicalEquality : Option (CanonicalEqualityEvidence left right)) :
    outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          (some collisionEvidence)
          (some digestEquality)
          canonicalEquality) = ReplayModeTag.digestCompressed := by
  rfl

theorem absent_collision_with_canonical_evidence_selects_full_field
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (digestEquality : Option (DigestEqualityEvidence digest left right))
    (canonicalEquality : CanonicalEqualityEvidence left right) :
    outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          none
          digestEquality
          (some canonicalEquality)) = ReplayModeTag.fullField := by
  rfl

theorem no_identity_evidence_selects_fallback
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (collisionEvidence : Option (CollisionFreeEvidence digest admitted)) :
    outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          collisionEvidence
          none
          none) = ReplayModeTag.explicitFieldFallback := by
  cases collisionEvidence <;> rfl

theorem negotiated_digest_has_collision_evidence
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (collisionEvidence : Option (CollisionFreeEvidence digest admitted))
    (digestEquality : Option (DigestEqualityEvidence digest left right))
    (canonicalEquality : Option (CanonicalEqualityEvidence left right))
    (selected :
      outcomeMode
          (negotiateReplayIdentity
            digest
            admitted
            left
            right
            leftAdmitted
            rightAdmitted
            collisionEvidence
            digestEquality
            canonicalEquality) = ReplayModeTag.digestCompressed) :
    ∃ evidence, collisionEvidence = some evidence := by
  cases collisionEvidence with
  | none =>
      exact False.elim
        (absent_collision_evidence_never_selects_digest
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          digestEquality
          canonicalEquality
          selected)
  | some evidence => exact ⟨evidence, rfl⟩

theorem accepted_outcome_implies_payload_equality
    (left right : CanonicalShortcutPayload)
    (outcome : ReplayIdentityOutcome left right)
    (accepted : outcomeMode outcome ≠ ReplayModeTag.explicitFieldFallback) :
    left = right := by
  cases outcome with
  | acceptedFull payloadEqual => exact payloadEqual
  | acceptedDigest _ payloadEqual => exact payloadEqual
  | explicitFieldFallback => exact False.elim (accepted rfl)

theorem digest_outcome_receipt_has_assumption_identity
    (left right : CanonicalShortcutPayload)
    (assumptionIdentity : Nat)
    (payloadEqual : left = right) :
    outcomeAssumptionIdentity
        (ReplayIdentityOutcome.acceptedDigest assumptionIdentity payloadEqual) =
      some assumptionIdentity := by
  rfl

theorem full_field_outcome_has_no_assumption_identity
    (left right : CanonicalShortcutPayload)
    (outcome : ReplayIdentityOutcome left right)
    (fullSelected : outcomeMode outcome = ReplayModeTag.fullField) :
    outcomeAssumptionIdentity outcome = none := by
  cases outcome with
  | acceptedFull _ => rfl
  | acceptedDigest _ _ => cases fullSelected
  | explicitFieldFallback => rfl

theorem full_and_digest_evidence_agree_on_payload_identity
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (_collisionEvidence : CollisionFreeEvidence digest admitted)
    (left right : CanonicalShortcutPayload)
    (_leftAdmitted : admitted left)
    (_rightAdmitted : admitted right)
    (_digestEquality : DigestEqualityEvidence digest left right)
    (canonicalEquality : CanonicalEqualityEvidence left right) :
    left = right := by
  exact full_field_replay_needs_no_digest_assumption
    left
    right
    canonicalEquality.sameEncoding

theorem negotiated_full_and_digest_modes_agree_on_payload_identity
    (digest : DigestFunction)
    (admitted : CanonicalShortcutPayload → Prop)
    (collisionEvidence : CollisionFreeEvidence digest admitted)
    (left right : CanonicalShortcutPayload)
    (leftAdmitted : admitted left)
    (rightAdmitted : admitted right)
    (digestEquality : DigestEqualityEvidence digest left right)
    (canonicalEquality : CanonicalEqualityEvidence left right) :
    outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          (some collisionEvidence)
          (some digestEquality)
          none) = ReplayModeTag.digestCompressed ∧
      outcomeMode
        (negotiateReplayIdentity
          digest
          admitted
          left
          right
          leftAdmitted
          rightAdmitted
          none
          none
          (some canonicalEquality)) = ReplayModeTag.fullField ∧
      left = right := by
  exact ⟨rfl, rfl,
    full_field_replay_needs_no_digest_assumption
      left
      right
      canonicalEquality.sameEncoding⟩

def replayIdentityReceiptTokenCost : ReplayModeTag → Nat
  | .fullField => 12
  | .digestCompressed => 3
  | .explicitFieldFallback => 12

theorem digest_mode_receipt_token_cost_is_smaller :
    replayIdentityReceiptTokenCost ReplayModeTag.digestCompressed <
      replayIdentityReceiptTokenCost ReplayModeTag.fullField := by
  decide

end ASPProof.SearchRouteSafeReplayModeNegotiation
