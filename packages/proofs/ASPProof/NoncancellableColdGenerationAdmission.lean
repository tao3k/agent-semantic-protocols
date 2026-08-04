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

end ASPProof.NoncancellableColdGenerationAdmission
