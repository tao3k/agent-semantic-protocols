namespace ASPProof.ActiveGenerationReadLease

inductive AdmissionState where
  | building
  | ready
  | failed
  | cancelled
  deriving DecidableEq

structure AdmissionReceipt where
  state : AdmissionState
  hasCommit : Bool

def admitsResidentRead (receipt : AdmissionReceipt) : Prop :=
  receipt.state = .ready ∧ receipt.hasCommit = true

theorem buildingCannotRead (receipt : AdmissionReceipt)
    (building : receipt.state = .building) :
    ¬ admitsResidentRead receipt := by
  intro readable
  exact AdmissionState.noConfusion (building.symm.trans readable.1)

theorem failedCannotRead (receipt : AdmissionReceipt)
    (failed : receipt.state = .failed) :
    ¬ admitsResidentRead receipt := by
  intro readable
  exact AdmissionState.noConfusion (failed.symm.trans readable.1)

theorem readyWithoutCommitCannotRead (receipt : AdmissionReceipt)
    (missingCommit : receipt.hasCommit = false) :
    ¬ admitsResidentRead receipt := by
  intro readable
  have : false = true := missingCommit.symm.trans readable.2
  contradiction

theorem readyCommittedCanRead (receipt : AdmissionReceipt)
    (ready : receipt.state = .ready)
    (committed : receipt.hasCommit = true) :
    admitsResidentRead receipt := by
  exact ⟨ready, committed⟩

end ASPProof.ActiveGenerationReadLease
