import ASPProof.ASPAgentLifeSessionClose

namespace ASPProof.ASPAgentLifeSessionAggregate

open ASPProof.ASPAgentSessionPortal
open ASPProof.ASPAgentLifeSessionRefinement
open ASPProof.ASPAgentLifeSessionClose

/-!
The aggregate composes binding authority and close/replacement authority without
allowing either sub-protocol to manufacture evidence owned by the other.
-/

def aggregateAuthority
    (decision : LifeSessionDecision)
    (close : CloseState) : CommandAuthority :=
  match close.phase with
  | .live => decision.authority
  | .closeIntentDurable => .none
  | .shutdownRequested => .none
  | .hostTerminated => .none
  | .pathReleased => .none
  | .blockedHostCapability => .none

def durableDispatchAdmitted
    (decision : LifeSessionDecision)
    (close : CloseState) : Flag :=
  match aggregateAuthority decision close with
  | .aspDurable => .yes
  | .none => .no
  | .hostOnly => .no

def replacementAdmission (close : CloseState) : Flag :=
  replacementAuthorized close

theorem archiveVisibilityCannotAlterBindingAuthority
    (decision : LifeSessionDecision) :
    aggregateAuthority decision
      ⟨.live, 1, 7, 0, .no, .yes, .no, .no, .no, .no⟩ =
    aggregateAuthority decision
      ⟨.live, 0, 7, 0, .yes, .no, .no, .no, .no, .no⟩ :=
  Eq.refl _

theorem closeIntentRevokesDurableDispatch :
    durableDispatchAdmitted
      ⟨.durableCall, .aspDurable, .none, .yes⟩ closeIntentState = .no :=
  Eq.refl _

theorem runtimeOnlyBindingCannotBecomeDurableDuringClose :
    aggregateAuthority
      ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩ closeIntentState = .none :=
  Eq.refl _

theorem replacementAndDurableDispatchAreDisjoint :
    replacementAdmission releasedState = .yes ∧
    durableDispatchAdmitted
      ⟨.durableCall, .aspDurable, .none, .yes⟩ releasedState = .no :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem liveDeliveredBindingAdmitsDurableDispatch :
    durableDispatchAdmitted
      ⟨.durableCall, .aspDurable, .none, .yes⟩ generationSeven = .yes :=
  Eq.refl _

theorem aggregateStillExposesOnePublicPortal :
    publicCommandCount = 1 :=
  Eq.refl _

end ASPProof.ASPAgentLifeSessionAggregate
