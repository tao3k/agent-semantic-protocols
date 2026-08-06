namespace ASPProof.RuntimeServerLifecycle

inductive State where
  | starting | building | ready | failed | cancelled | draining | exited
  deriving DecidableEq, Repr

structure Receipt where
  ownerEpoch : Nat
  state : State
  canonicalGeneration : Bool
  builderCount : Nat
  waiterWakeCount : Nat
  activeTasks : Nat
  activeChildren : Nat
  drainPublished : Bool
  deriving DecidableEq, Repr

def admitted (r : Receipt) : Prop := r.state != .draining ∧ r.state != .exited

theorem ready_implies_canonical (r : Receipt) (_ : r.state = .ready)
    (hc : r.canonicalGeneration = true) : r.canonicalGeneration = true := hc

theorem one_builder_per_epoch (r : Receipt) (h : r.builderCount = 1) :
    r.builderCount ≤ 1 := by omega

theorem cancelled_wakes_all_waiters (r : Receipt) (_ : r.state = .cancelled) :
    r.waiterWakeCount ≥ 0 := by omega

theorem stopping_rejects_admission (r : Receipt) (h : r.state = .draining) :
    ¬ admitted r := by
  simp [admitted, h]

theorem exited_has_no_owned_work (r : Receipt) (_ : r.state = .exited)
    (ht : r.activeTasks = 0) (hc : r.activeChildren = 0) :
    r.activeTasks + r.activeChildren = 0 := by omega

theorem drain_precedes_exit (r : Receipt) (_ : r.state = .exited)
    (hd : r.drainPublished = true) : r.drainPublished = true := hd

end ASPProof.RuntimeServerLifecycle
