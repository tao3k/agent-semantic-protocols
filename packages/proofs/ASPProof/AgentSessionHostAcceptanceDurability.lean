-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionDispatchLinearization

namespace ASPProof.AgentSessionHostAcceptanceDurability

open ASPProof.AgentSessionDispatchTransaction

abbrev HostAuthorityId := Nat
abbrev HostAuthorityEpoch := Nat
abbrev RegistryGeneration := Nat
abbrev EffectId := Nat

structure DurableHostReceipt
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration) where
  hostReceipt : HostReceipt intent.key intent.payloadDigest
  effectId : EffectId
  deriving DecidableEq, Repr

inductive DurableHostSlot
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration) where
  | empty
  | accepted
      (receipt : DurableHostReceipt intent authority epoch generation)
  deriving DecidableEq, Repr

structure DurableRegistry (intent : DispatchIntent) where
  authority : HostAuthorityId
  epoch : HostAuthorityEpoch
  generation : RegistryGeneration
  slot : DurableHostSlot intent authority epoch generation
  deriving DecidableEq, Repr

def firstDurableReceipt
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (effectId : EffectId) :
    DurableHostReceipt intent authority epoch generation :=
  {
    hostReceipt := firstHostReceipt intent.key intent.payloadDigest
    effectId := effectId
  }

inductive DurableHostAccepts
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration) :
    DurableHostSlot intent authority epoch generation →
    EffectId →
    DurableHostSlot intent authority epoch generation → Prop where
  | first (effectId : EffectId) :
      DurableHostAccepts intent authority epoch generation
        .empty
        effectId
        (.accepted
          (firstDurableReceipt intent authority epoch generation effectId))
  | replay
      (receipt : DurableHostReceipt intent authority epoch generation) :
      DurableHostAccepts intent authority epoch generation
        (.accepted receipt)
        receipt.effectId
        (.accepted receipt)

inductive CrashRestartStep {intent : DispatchIntent} :
    DurableRegistry intent → DurableRegistry intent → Prop where
  | preserve (registry : DurableRegistry intent) :
      CrashRestartStep registry registry

inductive VolatileResetStep
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration) :
    DurableHostSlot intent authority epoch generation →
    DurableHostSlot intent authority epoch generation → Prop where
  | forget
      (receipt : DurableHostReceipt intent authority epoch generation) :
      VolatileResetStep intent authority epoch generation
        (.accepted receipt)
        .empty

def migrateReceipt
    {intent : DispatchIntent}
    {oldAuthority : HostAuthorityId}
    {oldEpoch : HostAuthorityEpoch}
    {oldGeneration : RegistryGeneration}
    (newAuthority : HostAuthorityId)
    (receipt : DurableHostReceipt
      intent oldAuthority oldEpoch oldGeneration) :
    DurableHostReceipt
      intent newAuthority (oldEpoch + 1) (oldGeneration + 1) :=
  {
    hostReceipt := receipt.hostReceipt
    effectId := receipt.effectId
  }

inductive AuthorityRotationStep (intent : DispatchIntent) :
    DurableRegistry intent → DurableRegistry intent → Prop where
  | migrateEmpty
      (oldAuthority newAuthority : HostAuthorityId)
      (oldEpoch : HostAuthorityEpoch)
      (oldGeneration : RegistryGeneration) :
      AuthorityRotationStep intent
        {
          authority := oldAuthority
          epoch := oldEpoch
          generation := oldGeneration
          slot := .empty
        }
        {
          authority := newAuthority
          epoch := oldEpoch + 1
          generation := oldGeneration + 1
          slot := .empty
        }
  | migrateAccepted
      (oldAuthority newAuthority : HostAuthorityId)
      (oldEpoch : HostAuthorityEpoch)
      (oldGeneration : RegistryGeneration)
      (receipt : DurableHostReceipt
        intent oldAuthority oldEpoch oldGeneration) :
      AuthorityRotationStep intent
        {
          authority := oldAuthority
          epoch := oldEpoch
          generation := oldGeneration
          slot := .accepted receipt
        }
        {
          authority := newAuthority
          epoch := oldEpoch + 1
          generation := oldGeneration + 1
          slot := .accepted (migrateReceipt newAuthority receipt)
        }

inductive RecoveryDisposition where
  | resume
  | quarantine
  deriving DecidableEq, Repr

structure RotationCertificate
    {intent : DispatchIntent}
    (before after : DurableRegistry intent) : Type where
  continuity : AuthorityRotationStep intent before after

inductive RotationAdmission
    {intent : DispatchIntent}
    (before after : DurableRegistry intent) :
    Option (RotationCertificate before after) →
    RecoveryDisposition → Prop where
  | continuous
      (continuity : AuthorityRotationStep intent before after) :
      RotationAdmission before after
        (some { continuity := continuity }) .resume
  | missing :
      RotationAdmission before after none .quarantine

inductive RegistryContainsReceipt {intent : DispatchIntent} :
    (registry : DurableRegistry intent) →
    DurableHostReceipt intent
      registry.authority registry.epoch registry.generation → Prop where
  | accepted
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (receipt : DurableHostReceipt intent authority epoch generation) :
      RegistryContainsReceipt
        {
          authority := authority
          epoch := epoch
          generation := generation
          slot := .accepted receipt
        }
        receipt

inductive VerifiedDurableDeliveredCasStep
    (intent : DispatchIntent) :
    (registry : DurableRegistry intent) →
    Revision →
    DispatchTransaction intent →
    DurableHostReceipt intent
      registry.authority registry.epoch registry.generation →
    DispatchTransaction intent → Prop where
  | commit
      (authority : HostAuthorityId)
      (epoch : HostAuthorityEpoch)
      (generation : RegistryGeneration)
      (revision : Revision)
      (receipt : DurableHostReceipt intent authority epoch generation) :
      VerifiedDurableDeliveredCasStep intent
        {
          authority := authority
          epoch := epoch
          generation := generation
          slot := .accepted receipt
        }
        revision
        ({
          revision := revision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        receipt
        ({
          revision := revision + 1
          phase := TransactionPhase.delivered receipt.hostReceipt
        } : DispatchTransaction intent)

theorem durable_first_accept_constructible
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (effectId : EffectId) :
    DurableHostAccepts intent authority epoch generation
      .empty effectId
      (.accepted
        (firstDurableReceipt intent authority epoch generation effectId)) := by
  exact DurableHostAccepts.first effectId

theorem durable_retry_preserves_effect
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    DurableHostAccepts intent authority epoch generation
      (.accepted receipt) receipt.effectId (.accepted receipt) := by
  exact DurableHostAccepts.replay receipt

theorem crash_restart_preserves_registry
    {intent : DispatchIntent}
    {before after : DurableRegistry intent}
    (step : CrashRestartStep before after) :
    after = before := by
  cases step
  rfl

theorem accepted_registry_cannot_restart_empty
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ¬ CrashRestartStep
      ({
        authority := authority
        epoch := epoch
        generation := generation
        slot := .accepted receipt
      } : DurableRegistry intent)
      ({
        authority := authority
        epoch := epoch
        generation := generation
        slot := .empty
      } : DurableRegistry intent) := by
  intro step
  cases step

theorem volatile_reset_allows_duplicate_effect_trace
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration) :
    ∃ firstSlot emptySlot secondSlot,
      DurableHostAccepts intent authority epoch generation
        .empty 0 firstSlot ∧
      VolatileResetStep intent authority epoch generation
        firstSlot emptySlot ∧
      DurableHostAccepts intent authority epoch generation
        emptySlot 1 secondSlot ∧
      (0 : EffectId) ≠ 1 := by
  let firstReceipt :=
    firstDurableReceipt intent authority epoch generation 0
  let secondReceipt :=
    firstDurableReceipt intent authority epoch generation 1
  exact ⟨
    .accepted firstReceipt,
    .empty,
    .accepted secondReceipt,
    DurableHostAccepts.first 0,
    VolatileResetStep.forget firstReceipt,
    DurableHostAccepts.first 1,
    by decide
  ⟩

theorem accepted_rotation_preserves_effect
    (intent : DispatchIntent)
    (oldAuthority newAuthority : HostAuthorityId)
    (oldEpoch : HostAuthorityEpoch)
    (oldGeneration : RegistryGeneration)
    (receipt : DurableHostReceipt
      intent oldAuthority oldEpoch oldGeneration) :
    (migrateReceipt newAuthority receipt).effectId = receipt.effectId := by
  rfl

theorem accepted_rotation_preserves_host_sequence
    (intent : DispatchIntent)
    (oldAuthority newAuthority : HostAuthorityId)
    (oldEpoch : HostAuthorityEpoch)
    (oldGeneration : RegistryGeneration)
    (receipt : DurableHostReceipt
      intent oldAuthority oldEpoch oldGeneration) :
    (migrateReceipt newAuthority receipt).hostReceipt.sequence =
      receipt.hostReceipt.sequence := by
  rfl

theorem accepted_rotation_cannot_become_empty
    (intent : DispatchIntent)
    (oldAuthority newAuthority : HostAuthorityId)
    (oldEpoch : HostAuthorityEpoch)
    (oldGeneration : RegistryGeneration)
    (receipt : DurableHostReceipt
      intent oldAuthority oldEpoch oldGeneration) :
    ¬ AuthorityRotationStep intent
      ({
        authority := oldAuthority
        epoch := oldEpoch
        generation := oldGeneration
        slot := .accepted receipt
      } : DurableRegistry intent)
      ({
        authority := newAuthority
        epoch := oldEpoch + 1
        generation := oldGeneration + 1
        slot := .empty
      } : DurableRegistry intent) := by
  intro step
  cases step

theorem missing_continuity_quarantines
    {intent : DispatchIntent}
    (before after : DurableRegistry intent) :
    RotationAdmission before after none .quarantine := by
  exact RotationAdmission.missing

theorem missing_continuity_cannot_resume
    {intent : DispatchIntent}
    (before after : DurableRegistry intent) :
    ¬ RotationAdmission before after none .resume := by
  intro admission
  cases admission

theorem accepted_registry_contains_receipt
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    RegistryContainsReceipt
      ({
        authority := authority
        epoch := epoch
        generation := generation
        slot := .accepted receipt
      } : DurableRegistry intent)
      receipt := by
  exact RegistryContainsReceipt.accepted authority epoch generation receipt

theorem empty_registry_cannot_verify_receipt
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ¬ RegistryContainsReceipt
      ({
        authority := authority
        epoch := epoch
        generation := generation
        slot := .empty
      } : DurableRegistry intent)
      receipt := by
  intro contained
  cases contained

theorem verified_receipt_can_finalize
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (revision : Revision)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    VerifiedDurableDeliveredCasStep intent
      ({
        authority := authority
        epoch := epoch
        generation := generation
        slot := .accepted receipt
      } : DurableRegistry intent)
      revision
      ({
        revision := revision
        phase := TransactionPhase.attempting
      } : DispatchTransaction intent)
      receipt
      ({
        revision := revision + 1
        phase := TransactionPhase.delivered receipt.hostReceipt
      } : DispatchTransaction intent) := by
  exact VerifiedDurableDeliveredCasStep.commit
    authority epoch generation revision receipt

theorem empty_registry_cannot_finalize
    (intent : DispatchIntent)
    (authority : HostAuthorityId)
    (epoch : HostAuthorityEpoch)
    (generation : RegistryGeneration)
    (revision : Revision)
    (receipt : DurableHostReceipt intent authority epoch generation) :
    ¬ ∃ after,
      VerifiedDurableDeliveredCasStep intent
        ({
          authority := authority
          epoch := epoch
          generation := generation
          slot := .empty
        } : DurableRegistry intent)
        revision
        ({
          revision := revision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        receipt
        after := by
  intro witness
  rcases witness with ⟨after, step⟩
  cases step

end ASPProof.AgentSessionHostAcceptanceDurability
