import ASPProof.ASPAgentLifeSessionClose

namespace ASPProof.ASPAgentLifeSessionSingleResident

open ASPAgentSessionPortal
open ASPAgentLifeSessionClose

def incumbentLiveCount (state : CloseState) : Nat :=
  match state.phase with
  | .live | .closeIntentDurable | .shutdownRequested | .blockedHostCapability => 1
  | .hostTerminated | .pathReleased => 0

def replacementLiveCount (state : CloseState) (newGeneration : Nat) : Nat :=
  match reserveReplacement state newGeneration with
  | some _ => 1
  | none => 0

def totalLiveCount (state : CloseState) (newGeneration : Nat) : Nat :=
  incumbentLiveCount state + replacementLiveCount state newGeneration

theorem missingPathReleaseRejectsReplacement
    (state : CloseState)
    (notReleased : state.pathReleaseReceipt = .no) :
    replacementAuthorized state = .no := by
  cases state with
  | mk phase revision physicalGeneration stableDispatchKey hostTreeVisible archiveReceipt closeIntent
      shutdownOutbox terminationReceipt pathReleaseReceipt =>
      cases phase <;> try rfl
      cases closeIntent <;> try rfl
      cases terminationReceipt <;> try rfl
      cases pathReleaseReceipt
      · rfl
      · cases notReleased

theorem archiveWithoutReleaseRejectsReplacement
    (state : CloseState)
    (_archived : state.archiveReceipt = .yes)
    (notReleased : state.pathReleaseReceipt = .no) :
    replacementAuthorized state = .no := by
  exact missingPathReleaseRejectsReplacement state notReleased

theorem missingPathReleaseCannotReserveSameName
    (state : CloseState)
    (newGeneration : Nat)
    (notReleased : state.pathReleaseReceipt = .no) :
    reserveReplacement state newGeneration = none := by
  cases state with
  | mk phase revision physicalGeneration stableDispatchKey hostTreeVisible archiveReceipt closeIntent
      shutdownOutbox terminationReceipt pathReleaseReceipt =>
      cases phase <;> try rfl
      cases closeIntent <;> try rfl
      cases terminationReceipt <;> try rfl
      cases pathReleaseReceipt
      · rfl
      · cases notReleased

theorem archiveWithoutReleaseCannotReserveSameName
    (state : CloseState)
    (newGeneration : Nat)
    (_archived : state.archiveReceipt = .yes)
    (notReleased : state.pathReleaseReceipt = .no) :
    reserveReplacement state newGeneration = none := by
  exact missingPathReleaseCannotReserveSameName state newGeneration notReleased

theorem singleResidentPerSlot
    (state : CloseState)
    (newGeneration : Nat) :
    totalLiveCount state newGeneration ≤ 1 := by
  cases state with
  | mk phase revision physicalGeneration stableDispatchKey hostTreeVisible archiveReceipt closeIntent
      shutdownOutbox terminationReceipt pathReleaseReceipt =>
      cases phase with
      | live => exact Nat.le_refl 1
      | closeIntentDurable => exact Nat.le_refl 1
      | shutdownRequested => exact Nat.le_refl 1
      | blockedHostCapability => exact Nat.le_refl 1
      | hostTerminated => exact Nat.zero_le 1
      | pathReleased =>
          cases closeIntent with
          | no => exact Nat.zero_le 1
          | yes =>
              cases terminationReceipt with
              | no => exact Nat.zero_le 1
              | yes =>
                  cases pathReleaseReceipt with
                  | no => exact Nat.zero_le 1
                  | yes =>
                      unfold totalLiveCount incumbentLiveCount replacementLiveCount
                        reserveReplacement replacementAuthorized
                      cases generationFresh : natLess physicalGeneration newGeneration
                      · dsimp only [CloseState.phase, CloseState.closeIntent,
                          CloseState.terminationReceipt, CloseState.pathReleaseReceipt]
                        exact Nat.zero_le 1
                      · dsimp only [CloseState.phase, CloseState.closeIntent,
                          CloseState.terminationReceipt, CloseState.pathReleaseReceipt]
                        exact Nat.le_refl 1

theorem releasedFreshReplacementKeepsExactlyOneResident :
    totalLiveCount releasedState 8 = 1 := by
  decide

theorem releasedSameGenerationCreatesNoResident :
    totalLiveCount releasedState 7 = 0 := by
  decide

end ASPProof.ASPAgentLifeSessionSingleResident
