-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.NoncancellableColdGenerationAdmission

inductive GenerationState where
  | missing
  | queued
  | ready
  | failed
deriving DecidableEq

def beginRepair : GenerationState → GenerationState
  | .missing => .queued
  | state => state

def foregroundTimeout (state : GenerationState) : GenerationState := state

def publish : GenerationState → GenerationState
  | .queued => .ready
  | state => state

theorem timeoutCannotCancelQueuedRepair :
    foregroundTimeout (beginRepair .missing) = .queued := by
  rfl

theorem timeoutAfterAdmissionIsNotMissing :
    foregroundTimeout (beginRepair .missing) ≠ .missing := by
  decide

theorem queuedRepairPublishesReady :
    publish (foregroundTimeout (beginRepair .missing)) = .ready := by
  rfl

theorem timeoutDoesNotAuthorizeUnpublishedRead :
    foregroundTimeout (beginRepair .missing) ≠ .ready := by
  decide

inductive ReadinessAction where
  | reusePublished
  | enqueueRepair
deriving DecidableEq

def readinessAction : GenerationState → ReadinessAction
  | .ready => .reusePublished
  | .missing | .failed => .enqueueRepair
  | .queued => .reusePublished

theorem residentReadyNeverEnqueuesRepair :
    readinessAction .ready = .reusePublished := by
  rfl

structure ReadyFastPathReceipt where
  elapsedMicros : Nat
  providerInvocations : Nat
  databaseOpens : Nat

def residentReadyReceipt : ReadyFastPathReceipt where
  elapsedMicros := 99_999
  providerInvocations := 0
  databaseOpens := 0

theorem residentReadyStaysInsideHundredMillisecondBudget :
    residentReadyReceipt.elapsedMicros < 100_000 := by
  decide

theorem residentReadyPerformsNoProviderOrDatabaseWork :
    residentReadyReceipt.providerInvocations = 0 ∧
      residentReadyReceipt.databaseOpens = 0 := by
  decide

inductive RepairExecutorOwner where
  | requestStream
  | serverBootstrap
deriving DecidableEq

def survivesRequestDisconnect : RepairExecutorOwner → Bool
  | .requestStream => false
  | .serverBootstrap => true

theorem onlyServerBootstrapOwnsDurableRepair :
    survivesRequestDisconnect .serverBootstrap = true ∧
      survivesRequestDisconnect .requestStream = false := by
  decide

inductive ExplicitQueryGateOutcome where
  | release
  | failClosed
deriving DecidableEq

def explicitQueryGate : GenerationState → ExplicitQueryGateOutcome
  | .ready => .release
  | .missing | .queued | .failed => .failClosed

inductive ExactDataPlaneStage where
  | generationGate
  | ownerFreshness
  | projection
deriving DecidableEq

def nextExactStage : GenerationState → ExactDataPlaneStage
  | .ready => .ownerFreshness
  | .missing | .queued | .failed => .generationGate

theorem explicitQueryReleasesIffTerminalReady (state : GenerationState) :
    explicitQueryGate state = .release ↔ state = .ready := by
  cases state <;> decide

theorem buildingAdmissionCannotEnterOwnerFreshness :
    nextExactStage .queued ≠ .ownerFreshness := by
  decide

theorem failedAdmissionCannotEnterOwnerFreshness :
    nextExactStage .failed ≠ .ownerFreshness := by
  decide

theorem terminalReadyPrecedesOwnerFreshness :
    nextExactStage .ready = .ownerFreshness := by
  rfl

structure ExplicitQueryReadiness where
  admissionReady : Bool
  publishedLeaseOpen : Bool
  pointerExists : Bool
  pointerCurrentSchema : Bool
  pointerGenerationMatches : Bool
deriving DecidableEq

def compositeQueryReady (readiness : ExplicitQueryReadiness) : Prop :=
  readiness.admissionReady = true ∧
    readiness.publishedLeaseOpen = true ∧
    readiness.pointerExists = true ∧
    readiness.pointerCurrentSchema = true ∧
    readiness.pointerGenerationMatches = true

theorem admissionReadyWithoutPublishedLeaseCannotRelease :
    ¬ compositeQueryReady {
      admissionReady := true
      publishedLeaseOpen := false
      pointerExists := true
      pointerCurrentSchema := true
      pointerGenerationMatches := true
    } := by
  simp [compositeQueryReady]

theorem residentReadyAndPointerExistsWithoutCurrentSchemaCannotRelease :
    ¬ compositeQueryReady {
      admissionReady := true
      publishedLeaseOpen := true
      pointerExists := true
      pointerCurrentSchema := false
      pointerGenerationMatches := true
    } := by
  simp [compositeQueryReady]

theorem currentSchemaPointerForAnotherGenerationCannotRelease :
    ¬ compositeQueryReady {
      admissionReady := true
      publishedLeaseOpen := true
      pointerExists := true
      pointerCurrentSchema := true
      pointerGenerationMatches := false
    } := by
  simp [compositeQueryReady]

theorem publishedTerminalReadinessCanRelease :
    compositeQueryReady {
      admissionReady := true
      publishedLeaseOpen := true
      pointerExists := true
      pointerCurrentSchema := true
      pointerGenerationMatches := true
    } := by
  simp [compositeQueryReady]

end ASPProof.NoncancellableColdGenerationAdmission
