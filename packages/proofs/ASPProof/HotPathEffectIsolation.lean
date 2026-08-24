namespace ASPProof.HotPathEffectIsolation

inductive Effect where
  | mpsc | oneshot | cancel | response
  | process | filesystem | dbWrite | generationMutation
  | providerActivation | controlPoll
  deriving DecidableEq, Repr

def forbidden : Effect → Prop
  | .process | .filesystem | .dbWrite | .generationMutation
  | .providerActivation | .controlPoll => True
  | _ => False

def ReadyEffects : List Effect := [.mpsc, .oneshot, .cancel, .response]

def contains (effect : Effect) : List Effect → Prop
  | [] => False
  | head :: tail => head = effect ∨ contains effect tail

def ReadyPlan (effects : List Effect) : Prop :=
  ∀ effect, contains effect effects → ¬ forbidden effect

theorem ready_plan_is_local : ReadyPlan ReadyEffects := by
  intro effect present
  cases effect <;> simp [ReadyEffects, contains, forbidden] at present ⊢

inductive AdmissionState where
  | ready | building | failed | cancelled
  deriving DecidableEq, Repr

def dispatchAllowed : AdmissionState → Prop
  | .ready => True
  | .building | .failed | .cancelled => False

theorem non_ready_does_not_dispatch (state : AdmissionState)
    (notReady : state ≠ .ready) : ¬ dispatchAllowed state := by
  cases state <;> simp [dispatchAllowed] at notReady ⊢

structure TaskCounts where
  accepted : Nat
  cancelled : Nat
  drained : Nat

def drained (counts : TaskCounts) : Prop :=
  counts.accepted = counts.cancelled ∧ counts.cancelled = counts.drained

theorem drained_counts_have_no_residual_tasks (counts : TaskCounts)
    (balanced : drained counts) : counts.accepted - counts.drained = 0 := by
  rcases balanced with ⟨acceptedCancelled, cancelledDrained⟩
  omega

end ASPProof.HotPathEffectIsolation
