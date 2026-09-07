-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionHostRegistryCrashSafeMaterializationRecovery

namespace ASPProof.AgentSessionHostRegistryQuarantineAdjudication

open ASPProof.AgentSessionHostRegistryCrashSafeMaterializationRecovery

structure QuarantineCase where
  key : StableInvocationKey
  accepted : TerminalCapture
  conflicting : TerminalCapture
  claimantExecutorDigest : Nat
  challengerExecutorDigest : Nat
  quarantineDigest : Nat
  deriving DecidableEq, Repr

structure AdjudicationEvidence where
  quarantineDigest : Nat
  accepted : TerminalCapture
  conflicting : TerminalCapture
  evidenceDigest : Nat
  complete : Bool
  deriving DecidableEq, Repr

inductive AdjudicationDecision where
  | upholdAccepted
  | replaceWithConflicting
  | rejectBoth
  deriving DecidableEq, Repr

structure AdjudicationAuthority where
  authorityDigest : Nat
  scopeDigest : Nat
  revision : Nat
  deriving DecidableEq, Repr

structure AdjudicationRequest where
  authority : AdjudicationAuthority
  adjudicatorDigest : Nat
  evidence : AdjudicationEvidence
  decision : AdjudicationDecision
  deriving DecidableEq, Repr

def EvidenceBindsCase
    (quarantine : QuarantineCase)
    (evidence : AdjudicationEvidence) : Prop :=
  evidence.complete = true ∧
  evidence.quarantineDigest = quarantine.quarantineDigest ∧
  evidence.accepted = quarantine.accepted ∧
  evidence.conflicting = quarantine.conflicting

def AuthorizedAdjudication
    (expectedAuthority : AdjudicationAuthority)
    (quarantine : QuarantineCase)
    (request : AdjudicationRequest) : Prop :=
  request.authority = expectedAuthority ∧
  request.adjudicatorDigest ≠ quarantine.claimantExecutorDigest ∧
  request.adjudicatorDigest ≠ quarantine.challengerExecutorDigest ∧
  EvidenceBindsCase quarantine request.evidence

structure AdjudicationRecord where
  quarantineDigest : Nat
  authorityDigest : Nat
  adjudicatorDigest : Nat
  evidenceDigest : Nat
  decision : AdjudicationDecision
  oldGeneration : Nat
  newGeneration : Nat
  deriving DecidableEq, Repr

abbrev AdjudicationHistory := List AdjudicationRecord

inductive AdjudicationHistoryStep :
    AdjudicationHistory → AdjudicationRecord → AdjudicationHistory → Prop where
  | append
      (before : AdjudicationHistory)
      (record : AdjudicationRecord) :
      AdjudicationHistoryStep before record (before ++ [record])

def advanceGeneration (key : StableInvocationKey) : StableInvocationKey :=
  { key with generation := key.generation + 1 }

inductive AdjudicationSlot where
  | quarantined (quarantine : QuarantineCase)
  | resolved
      (key : StableInvocationKey)
      (decision : AdjudicationDecision)
      (recordDigest : Nat)
  deriving DecidableEq, Repr

inductive QuarantineResolution
    (expectedAuthority : AdjudicationAuthority) :
    AdjudicationHistory → AdjudicationSlot →
    AdjudicationHistory → AdjudicationSlot → Prop where
  | resolve
      (before : AdjudicationHistory)
      (quarantine : QuarantineCase)
      (request : AdjudicationRequest)
      (recordDigest : Nat)
      (authorized : AuthorizedAdjudication expectedAuthority quarantine request)
      (history : AdjudicationHistoryStep before {
        quarantineDigest := quarantine.quarantineDigest
        authorityDigest := request.authority.authorityDigest
        adjudicatorDigest := request.adjudicatorDigest
        evidenceDigest := request.evidence.evidenceDigest
        decision := request.decision
        oldGeneration := quarantine.key.generation
        newGeneration := quarantine.key.generation + 1
      } (before ++ [{
        quarantineDigest := quarantine.quarantineDigest
        authorityDigest := request.authority.authorityDigest
        adjudicatorDigest := request.adjudicatorDigest
        evidenceDigest := request.evidence.evidenceDigest
        decision := request.decision
        oldGeneration := quarantine.key.generation
        newGeneration := quarantine.key.generation + 1
      }])) :
      QuarantineResolution expectedAuthority
        before (.quarantined quarantine)
        (before ++ [{
          quarantineDigest := quarantine.quarantineDigest
          authorityDigest := request.authority.authorityDigest
          adjudicatorDigest := request.adjudicatorDigest
          evidenceDigest := request.evidence.evidenceDigest
          decision := request.decision
          oldGeneration := quarantine.key.generation
          newGeneration := quarantine.key.generation + 1
        }])
        (.resolved (advanceGeneration quarantine.key) request.decision recordDigest)

def GenerationAuthorized
    (currentGeneration : Nat)
    (key : StableInvocationKey) : Prop :=
  key.generation = currentGeneration

theorem evidence_binding_requires_complete
    {quarantine : QuarantineCase}
    {evidence : AdjudicationEvidence}
    (bound : EvidenceBindsCase quarantine evidence) :
    evidence.complete = true := by
  exact bound.1

theorem evidence_binding_includes_accepted_terminal
    {quarantine : QuarantineCase}
    {evidence : AdjudicationEvidence}
    (bound : EvidenceBindsCase quarantine evidence) :
    evidence.accepted = quarantine.accepted := by
  exact bound.2.2.1

theorem evidence_binding_includes_conflicting_terminal
    {quarantine : QuarantineCase}
    {evidence : AdjudicationEvidence}
    (bound : EvidenceBindsCase quarantine evidence) :
    evidence.conflicting = quarantine.conflicting := by
  exact bound.2.2.2

theorem incomplete_evidence_cannot_bind
    (quarantine : QuarantineCase)
    (evidence : AdjudicationEvidence)
    (incomplete : evidence.complete = false) :
    ¬ EvidenceBindsCase quarantine evidence := by
  intro bound
  exact Bool.false_ne_true (incomplete.symm.trans bound.1)

theorem one_sided_accepted_evidence_cannot_bind
    (quarantine : QuarantineCase)
    (evidence : AdjudicationEvidence)
    (wrong : evidence.accepted ≠ quarantine.accepted) :
    ¬ EvidenceBindsCase quarantine evidence := by
  intro bound
  exact wrong bound.2.2.1

theorem one_sided_conflicting_evidence_cannot_bind
    (quarantine : QuarantineCase)
    (evidence : AdjudicationEvidence)
    (wrong : evidence.conflicting ≠ quarantine.conflicting) :
    ¬ EvidenceBindsCase quarantine evidence := by
  intro bound
  exact wrong bound.2.2.2

theorem authorized_adjudication_binds_authority
    {expected : AdjudicationAuthority}
    {quarantine : QuarantineCase}
    {request : AdjudicationRequest}
    (authorized : AuthorizedAdjudication expected quarantine request) :
    request.authority = expected := by
  exact authorized.1

theorem claimant_cannot_self_adjudicate
    (expected : AdjudicationAuthority)
    (quarantine : QuarantineCase)
    (request : AdjudicationRequest)
    (self : request.adjudicatorDigest = quarantine.claimantExecutorDigest) :
    ¬ AuthorizedAdjudication expected quarantine request := by
  intro authorized
  exact authorized.2.1 self

theorem challenger_cannot_self_adjudicate
    (expected : AdjudicationAuthority)
    (quarantine : QuarantineCase)
    (request : AdjudicationRequest)
    (self : request.adjudicatorDigest = quarantine.challengerExecutorDigest) :
    ¬ AuthorizedAdjudication expected quarantine request := by
  intro authorized
  exact authorized.2.2.1 self

theorem authority_mismatch_rejects_adjudication
    (expected : AdjudicationAuthority)
    (quarantine : QuarantineCase)
    (request : AdjudicationRequest)
    (mismatch : request.authority ≠ expected) :
    ¬ AuthorizedAdjudication expected quarantine request := by
  intro authorized
  exact mismatch authorized.1

theorem history_step_appends_exact_record
    (before : AdjudicationHistory)
    (record : AdjudicationRecord) :
    AdjudicationHistoryStep before record (before ++ [record]) := by
  exact AdjudicationHistoryStep.append before record

theorem history_step_cannot_erase_single_record
    (record : AdjudicationRecord) :
    ¬ AdjudicationHistoryStep [record] record [] := by
  intro step
  cases step

theorem resolution_advances_generation
    {expected : AdjudicationAuthority}
    {before after : AdjudicationHistory}
    {quarantine : QuarantineCase}
    {resolved : AdjudicationSlot}
    (transition : QuarantineResolution expected before (.quarantined quarantine)
      after resolved) :
    ∃ decision recordDigest,
      resolved = .resolved (advanceGeneration quarantine.key) decision recordDigest := by
  cases transition with
  | resolve _ request recordDigest _ _ =>
      exact ⟨request.decision, recordDigest, rfl⟩

theorem resolved_slot_cannot_be_resolved_again
    (expected : AdjudicationAuthority)
    (before : AdjudicationHistory)
    (key : StableInvocationKey)
    (decision : AdjudicationDecision)
    (recordDigest : Nat) :
    ¬ ∃ after next,
      QuarantineResolution expected before (.resolved key decision recordDigest)
        after next := by
  intro witness
  obtain ⟨after, next, transition⟩ := witness
  cases transition

theorem advanced_key_has_successor_generation
    (key : StableInvocationKey) :
    (advanceGeneration key).generation = key.generation + 1 := by
  rfl

theorem stale_generation_cannot_resume_after_resolution
    (key : StableInvocationKey) :
    ¬ GenerationAuthorized (advanceGeneration key).generation key := by
  intro authorized
  exact (Nat.ne_of_lt (Nat.lt_succ_self key.generation)) authorized

theorem advanced_generation_is_authorized
    (key : StableInvocationKey) :
    GenerationAuthorized (key.generation + 1) (advanceGeneration key) := by
  rfl

theorem adjudicator_is_distinct_from_both_workers
    {expected : AdjudicationAuthority}
    {quarantine : QuarantineCase}
    {request : AdjudicationRequest}
    (authorized : AuthorizedAdjudication expected quarantine request) :
    request.adjudicatorDigest ≠ quarantine.claimantExecutorDigest ∧
    request.adjudicatorDigest ≠ quarantine.challengerExecutorDigest := by
  exact ⟨authorized.2.1, authorized.2.2.1⟩

theorem authorized_adjudication_binds_both_terminals
    {expected : AdjudicationAuthority}
    {quarantine : QuarantineCase}
    {request : AdjudicationRequest}
    (authorized : AuthorizedAdjudication expected quarantine request) :
    request.evidence.accepted = quarantine.accepted ∧
    request.evidence.conflicting = quarantine.conflicting := by
  exact ⟨authorized.2.2.2.2.2.1, authorized.2.2.2.2.2.2⟩

end ASPProof.AgentSessionHostRegistryQuarantineAdjudication
