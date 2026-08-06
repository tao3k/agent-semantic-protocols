namespace ASPProof.RuntimeServerTaskLifecycle

inductive Phase where
  | accepting
  | draining
  | terminated
  deriving DecidableEq, Repr

structure Ledger where
  phase : Phase
  started : Nat
  completed : Nat
  cancelled : Nat
  failed : Nat
  active : Nat
  conservation : started = completed + cancelled + failed + active

def admit (ledger : Ledger) : Option Ledger :=
  if ledger.phase = Phase.accepting then
    some {
      ledger with
      started := ledger.started + 1
      active := ledger.active + 1
      conservation := by
        have conserved := ledger.conservation
        omega
    }
  else
    none

def complete (ledger : Ledger) (positive : 0 < ledger.active) : Ledger := {
  ledger with
  completed := ledger.completed + 1
  active := ledger.active - 1
  conservation := by
    have conserved := ledger.conservation
    omega
}

def beginDrain (ledger : Ledger) : Ledger := { ledger with phase := Phase.draining }

def terminate (ledger : Ledger) (_drained : ledger.active = 0) : Ledger := {
  ledger with
  phase := Phase.terminated
}

theorem draining_rejects_admission (ledger : Ledger)
    (phase : ledger.phase = Phase.draining) :
    admit ledger = none := by
  simp [admit, phase]

theorem terminated_rejects_admission (ledger : Ledger)
    (phase : ledger.phase = Phase.terminated) :
    admit ledger = none := by
  simp [admit, phase]

theorem terminated_has_no_leaked_tasks (ledger : Ledger)
    (drained : ledger.active = 0) :
    (terminate ledger drained).active = 0 := by
  simp [terminate, drained]

theorem terminal_receipt_is_conserved (ledger : Ledger)
    (drained : ledger.active = 0) :
    ledger.started = ledger.completed + ledger.cancelled + ledger.failed := by
  have conserved := ledger.conservation
  omega

end ASPProof.RuntimeServerTaskLifecycle
