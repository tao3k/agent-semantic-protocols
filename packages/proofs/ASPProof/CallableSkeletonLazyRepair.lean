-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.CallableSkeletonLazyRepair

structure OwnerGeneration where
  generation : Nat
  selectors : List String
  deriving DecidableEq, Repr

structure ProjectionCandidate where
  generation : Nat
  selector : String
  projectionKind : String
  deriving DecidableEq, Repr

def ownerReady (owner : OwnerGeneration) : Prop :=
  owner.selectors ≠ []

def candidateAdmitted
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate) : Prop :=
  candidate.generation = owner.generation ∧
    candidate.selector = requestedSelector ∧
    requestedSelector ∈ owner.selectors ∧
    candidate.projectionKind = "callable-skeleton"

def publishOverlay
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate) : Option ProjectionCandidate :=
  if candidate.generation = owner.generation &&
      candidate.selector = requestedSelector &&
      requestedSelector ∈ owner.selectors &&
      candidate.projectionKind = "callable-skeleton" then
    some candidate
  else
    none

theorem ownerReadinessDoesNotRequireDerivedProjection
    (owner : OwnerGeneration)
    (h : owner.selectors ≠ []) : ownerReady owner :=
  h

theorem staleCandidateIsRejected
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate)
    (h : candidate.generation ≠ owner.generation) :
    publishOverlay owner requestedSelector candidate = none := by
  simp [publishOverlay, h]

theorem wrongSelectorIsRejected
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate)
    (h : candidate.selector ≠ requestedSelector) :
    publishOverlay owner requestedSelector candidate = none := by
  simp [publishOverlay, h]

theorem wrongProjectionKindIsRejected
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate)
    (h : candidate.projectionKind ≠ "callable-skeleton") :
    publishOverlay owner requestedSelector candidate = none := by
  simp [publishOverlay, h]

theorem missingBaseSelectorCannotBeRepaired
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate)
    (h : requestedSelector ∉ owner.selectors) :
    publishOverlay owner requestedSelector candidate = none := by
  simp [publishOverlay, h]

structure ExactProjectionResult where
  generation : Nat
  requestedSelector : String
  resolvedSelector : String
  deriving DecidableEq, Repr

def exactProjectionAdmitted
    (owner : OwnerGeneration)
    (result : ExactProjectionResult) : Prop :=
  result.generation = owner.generation ∧
    result.requestedSelector ∈ owner.selectors ∧
    result.resolvedSelector = result.requestedSelector

theorem relocatedSelectorCannotBeExactHit
    (owner : OwnerGeneration)
    (result : ExactProjectionResult)
    (h : result.resolvedSelector ≠ result.requestedSelector) :
    ¬ exactProjectionAdmitted owner result := by
  intro admitted
  exact h admitted.2.2

structure ProviderNativeExactRoute where
  acceptsStdinJson : Bool
  emitsResponseJson : Bool
  deriving DecidableEq, Repr

def providerNativeExactRouteAdmitted (route : ProviderNativeExactRoute) : Prop :=
  route.acceptsStdinJson = true ∧ route.emitsResponseJson = true

theorem stdinOnlyProviderNativeRouteIsRejected :
    ¬ providerNativeExactRouteAdmitted
      { acceptsStdinJson := true, emitsResponseJson := false } := by
  simp [providerNativeExactRouteAdmitted]

theorem admittedCandidatePublishesExactlyRequestedSelector
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate)
    (h : candidateAdmitted owner requestedSelector candidate) :
    publishOverlay owner requestedSelector candidate = some candidate := by
  rcases h with ⟨generation, selector, member, kind⟩
  simp [publishOverlay, generation, selector, member, kind]

theorem publicationDoesNotMutateOwnerGeneration
    (owner : OwnerGeneration)
    (requestedSelector : String)
    (candidate : ProjectionCandidate) :
    (owner, publishOverlay owner requestedSelector candidate).1 = owner :=
  rfl

inductive GenerationWaitState where
  | building
  | ready
  | failed
  | timedOut
  deriving DecidableEq, Repr

def boundedTerminalObservation : Nat → List GenerationWaitState → GenerationWaitState
  | 0, _ => .timedOut
  | _ + 1, [] => .timedOut
  | budget + 1, .building :: rest => boundedTerminalObservation budget rest
  | _ + 1, state :: _ => state

theorem boundedTerminalObservationNeverLeaksBuilding
    (budget : Nat)
    (observations : List GenerationWaitState) :
    boundedTerminalObservation budget observations ≠ .building := by
  induction budget generalizing observations with
  | zero => simp [boundedTerminalObservation]
  | succ budget ih =>
      cases observations with
      | nil => simp [boundedTerminalObservation]
      | cons state rest =>
          cases state <;> simp [boundedTerminalObservation, ih]

structure MutationFlightKey where
  socketPath : String
  ownerEpoch : Nat
  workspaceIdentity : String
  mutationId : String
  deriving DecidableEq, Repr

theorem distinctMutationsCannotAliasSingleFlight
    (left right : MutationFlightKey)
    (h : left.mutationId ≠ right.mutationId) : left ≠ right := by
  intro equal
  apply h
  exact congrArg MutationFlightKey.mutationId equal

inductive GenerationBuildMode where
  | restoreOrBuild
  | rebuildAfterMutation
  deriving DecidableEq, Repr

def mayRestoreDurableGeneration : GenerationBuildMode → Bool
  | .restoreOrBuild => true
  | .rebuildAfterMutation => false

theorem admittedMutationCannotRestorePreviousGeneration :
    mayRestoreDurableGeneration .rebuildAfterMutation = false :=
  rfl

structure GenerationPublicationRequestId where
  workspaceIdentity : String
  generationRootDigest : String
  selectorSetDigest : String
  deriving DecidableEq, Repr

theorem distinctGenerationRootsCannotAliasPublication
    (left right : GenerationPublicationRequestId)
    (h : left.generationRootDigest ≠ right.generationRootDigest) : left ≠ right := by
  intro equal
  apply h
  exact congrArg GenerationPublicationRequestId.generationRootDigest equal

theorem distinctSelectorSetsCannotAliasPublication
    (left right : GenerationPublicationRequestId)
    (h : left.selectorSetDigest ≠ right.selectorSetDigest) : left ≠ right := by
  intro equal
  apply h
  exact congrArg GenerationPublicationRequestId.selectorSetDigest equal

end ASPProof.CallableSkeletonLazyRepair
