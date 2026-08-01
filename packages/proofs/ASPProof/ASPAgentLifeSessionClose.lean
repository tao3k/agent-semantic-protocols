import ASPProof.ASPAgentLifeSessionRefinement

namespace ASPProof.ASPAgentLifeSessionClose

open ASPProof.ASPAgentSessionPortal

/-!
Closing a Codex child is a durable protocol, not a presentation operation.
Archive visibility, the persisted close edge, shutdown dispatch, observed host
termination, and canonical collaboration-path release are independent facts.

The reducer is deliberately shaped as a Rust aggregate: every non-idempotent
transition is admitted by revision and physical-generation fences, and the
stable dispatch key survives retries between intent persistence and host
termination.
-/

inductive ClosePhase where
  | live
  | closeIntentDurable
  | shutdownRequested
  | hostTerminated
  | pathReleased
  | blockedHostCapability
  deriving DecidableEq, Repr

inductive CloseEvent where
  | archiveAccepted
  | closeIntentCommitted
  | shutdownDispatched
  | terminationObserved
  | pathReleaseCommitted
  | shutdownTimedOut
  deriving DecidableEq, Repr

structure CloseEnvelope where
  expectedRevision : Nat
  physicalGeneration : Nat
  stableDispatchKey : Nat
  deriving DecidableEq, Repr

structure CloseState where
  phase : ClosePhase
  revision : Nat
  physicalGeneration : Nat
  stableDispatchKey : Nat
  hostTreeVisible : Flag
  archiveReceipt : Flag
  closeIntent : Flag
  shutdownOutbox : Flag
  terminationReceipt : Flag
  pathReleaseReceipt : Flag

def initialCloseState (generation : Nat) : CloseState :=
  ⟨.live, 0, generation, 0, .yes, .no, .no, .no, .no, .no⟩

def natEqual : Nat → Nat → Bool
  | 0, 0 => true
  | 0, Nat.succ _ => false
  | Nat.succ _, 0 => false
  | Nat.succ left, Nat.succ right => natEqual left right

def envelopeCurrent (state : CloseState) (envelope : CloseEnvelope) : Bool :=
  match natEqual state.revision envelope.expectedRevision with
  | false => false
  | true => natEqual state.physicalGeneration envelope.physicalGeneration

def stableKeyMatches (state : CloseState) (envelope : CloseEnvelope) : Bool :=
  natEqual state.stableDispatchKey envelope.stableDispatchKey

def natLess : Nat → Nat → Bool
  | 0, 0 => false
  | 0, Nat.succ _ => true
  | Nat.succ _, 0 => false
  | Nat.succ left, Nat.succ right => natLess left right

def replacementAuthorized (state : CloseState) : Flag :=
  match state.phase with
  | .live => .no
  | .closeIntentDurable => .no
  | .shutdownRequested => .no
  | .hostTerminated => .no
  | .blockedHostCapability => .no
  | .pathReleased =>
      match state.closeIntent with
      | .no => .no
      | .yes =>
          match state.terminationReceipt with
          | .no => .no
          | .yes => state.pathReleaseReceipt

def advance (state : CloseState) : CloseState :=
  { state with revision := state.revision + 1 }

def applyCurrentClose
    (state : CloseState)
    (envelope : CloseEnvelope)
    (event : CloseEvent) : Option CloseState :=
  match event with
  | .archiveAccepted =>
      match state.archiveReceipt with
      | .yes => some state
      | .no => some (advance { state with hostTreeVisible := .no, archiveReceipt := .yes })
  | .closeIntentCommitted =>
      match state.phase with
      | .live =>
          some (advance {
            state with
            phase := .closeIntentDurable
            stableDispatchKey := envelope.stableDispatchKey
            closeIntent := .yes
          })
      | .closeIntentDurable =>
          match stableKeyMatches state envelope with
          | true => some state
          | false => none
      | .shutdownRequested =>
          match stableKeyMatches state envelope with
          | true => some state
          | false => none
      | .hostTerminated =>
          match stableKeyMatches state envelope with
          | true => some state
          | false => none
      | .pathReleased =>
          match stableKeyMatches state envelope with
          | true => some state
          | false => none
      | .blockedHostCapability =>
          match stableKeyMatches state envelope with
          | true => some state
          | false => none
  | .shutdownDispatched =>
      match stableKeyMatches state envelope with
      | true =>
        match state.phase with
        | .closeIntentDurable =>
            some (advance { state with phase := .shutdownRequested, shutdownOutbox := .yes })
        | .shutdownRequested => some state
        | .blockedHostCapability => some state
        | .hostTerminated => some state
        | .pathReleased => some state
        | .live => none
      | false => none
  | .terminationObserved =>
      match stableKeyMatches state envelope with
      | true =>
        match state.phase with
        | .shutdownRequested =>
            some (advance { state with phase := .hostTerminated, terminationReceipt := .yes })
        | .blockedHostCapability =>
            some (advance { state with phase := .hostTerminated, terminationReceipt := .yes })
        | .hostTerminated => some state
        | .pathReleased => some state
        | .live => none
        | .closeIntentDurable => none
      | false => none
  | .pathReleaseCommitted =>
      match stableKeyMatches state envelope with
      | true =>
        match state.phase with
        | .hostTerminated =>
            some (advance { state with phase := .pathReleased, pathReleaseReceipt := .yes })
        | .pathReleased => some state
        | .live => none
        | .closeIntentDurable => none
        | .shutdownRequested => none
        | .blockedHostCapability => none
      | false => none
  | .shutdownTimedOut =>
      match stableKeyMatches state envelope with
      | true =>
        match state.phase with
        | .shutdownRequested =>
            some (advance { state with phase := .blockedHostCapability })
        | .blockedHostCapability => some state
        | .live => none
        | .closeIntentDurable => none
        | .hostTerminated => none
        | .pathReleased => none
      | false => none

def applyClose
    (state : CloseState)
    (envelope : CloseEnvelope)
    (event : CloseEvent) : Option CloseState :=
  match envelopeCurrent state envelope with
  | true => applyCurrentClose state envelope event
  | false => none

def reserveReplacement (state : CloseState) (newGeneration : Nat) : Option Nat :=
  match replacementAuthorized state with
  | .no => none
  | .yes =>
      match natLess state.physicalGeneration newGeneration with
      | true => some newGeneration
      | false => none

def generationSeven : CloseState := initialCloseState 7
def envelope0 : CloseEnvelope := ⟨0, 7, 41⟩
def closeIntentState : CloseState :=
  ⟨.closeIntentDurable, 1, 7, 41, .yes, .no, .yes, .no, .no, .no⟩
def envelope1 : CloseEnvelope := ⟨1, 7, 41⟩
def shutdownRequestedState : CloseState :=
  ⟨.shutdownRequested, 2, 7, 41, .yes, .no, .yes, .yes, .no, .no⟩
def envelope2 : CloseEnvelope := ⟨2, 7, 41⟩
def terminatedState : CloseState :=
  ⟨.hostTerminated, 3, 7, 41, .yes, .no, .yes, .yes, .yes, .no⟩
def envelope3 : CloseEnvelope := ⟨3, 7, 41⟩
def releasedState : CloseState :=
  ⟨.pathReleased, 4, 7, 41, .yes, .no, .yes, .yes, .yes, .yes⟩
def blockedState : CloseState :=
  ⟨.blockedHostCapability, 3, 7, 41, .yes, .no, .yes, .yes, .no, .no⟩

theorem archiveIsPresentationOnly :
    applyClose generationSeven envelope0 .archiveAccepted =
      some ⟨.live, 1, 7, 0, .no, .yes, .no, .no, .no, .no⟩ :=
  Eq.refl _

theorem archiveDoesNotAuthorizeReplacement :
    replacementAuthorized
      ⟨.live, 1, 7, 0, .no, .yes, .no, .no, .no, .no⟩ = .no :=
  Eq.refl _

theorem closeIntentPersistsStableDispatchKey :
    applyClose generationSeven envelope0 .closeIntentCommitted = some closeIntentState :=
  Eq.refl _

theorem persistedCloseEdgeDoesNotProveTermination :
    closeIntentState.closeIntent = .yes ∧
    closeIntentState.terminationReceipt = .no ∧
    replacementAuthorized closeIntentState = .no :=
  ⟨Eq.refl _, Eq.refl _, Eq.refl _⟩

theorem staleRevisionIsRejected :
    applyClose closeIntentState envelope0 .shutdownDispatched = none :=
  Eq.refl _

theorem staleGenerationIsRejected :
    applyClose generationSeven ⟨0, 6, 41⟩ .closeIntentCommitted = none :=
  Eq.refl _

theorem mismatchedDispatchKeyIsRejected :
    applyClose closeIntentState ⟨1, 7, 42⟩ .shutdownDispatched = none :=
  Eq.refl _

theorem shutdownDispatchRequiresDurableIntent :
    applyClose generationSeven envelope0 .shutdownDispatched = none :=
  Eq.refl _

theorem closeIntentAdvancesToShutdown :
    applyClose closeIntentState envelope1 .shutdownDispatched = some shutdownRequestedState :=
  Eq.refl _

theorem timeoutBlocksWithoutAuthorizingReplacement :
    applyClose shutdownRequestedState envelope2 .shutdownTimedOut = some blockedState ∧
    replacementAuthorized blockedState = .no :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem lateTerminationRecoversBlockedClose :
    applyClose blockedState ⟨3, 7, 41⟩ .terminationObserved =
      some ⟨.hostTerminated, 4, 7, 41, .yes, .no, .yes, .yes, .yes, .no⟩ :=
  Eq.refl _

theorem releaseBeforeTerminationIsRejected :
    applyClose shutdownRequestedState envelope2 .pathReleaseCommitted = none :=
  Eq.refl _

theorem terminationReceiptAdvancesClose :
    applyClose shutdownRequestedState envelope2 .terminationObserved = some terminatedState :=
  Eq.refl _

theorem releaseClosesCanonicalPath :
    applyClose terminatedState envelope3 .pathReleaseCommitted = some releasedState ∧
    replacementAuthorized releasedState = .yes :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem replacementRequiresFreshPhysicalGeneration :
    reserveReplacement releasedState 7 = none ∧
    reserveReplacement releasedState 8 = some 8 :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem archiveMayFollowCloseIntent :
    applyClose closeIntentState envelope1 .archiveAccepted =
      some ⟨.closeIntentDurable, 2, 7, 41, .no, .yes, .yes, .no, .no, .no⟩ :=
  Eq.refl _

theorem duplicateCloseIntentIsIdempotent :
    applyClose closeIntentState envelope1 .closeIntentCommitted = some closeIntentState :=
  Eq.refl _

end ASPProof.ASPAgentLifeSessionClose
