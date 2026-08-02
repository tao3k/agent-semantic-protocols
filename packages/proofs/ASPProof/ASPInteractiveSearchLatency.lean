namespace ASPProof.ASPInteractiveSearchLatency

def warmExactHardCeilingUs : Nat := 1000
def warmSearchHardCeilingUs : Nat := 10000
def firstReceiptHardCeilingUs : Nat := 100000
def waitQuantumHardCeilingUs : Nat := 250000

inductive GenerationState where
  | missing
  | building
  | readyCurrent
  | readyStale
  | projectionMissing
  | failed
  deriving DecidableEq, Repr

inductive ReplyKind where
  | evidence
  | progress
  | unavailable
  deriving DecidableEq, Repr

structure SearchReply where
  kind : ReplyKind
  retryAfterUs : Nat
  deriving DecidableEq, Repr

def replyFor : GenerationState -> SearchReply
  | .missing => { kind := .progress, retryAfterUs := waitQuantumHardCeilingUs }
  | .building => { kind := .progress, retryAfterUs := waitQuantumHardCeilingUs }
  | .readyCurrent => { kind := .evidence, retryAfterUs := 0 }
  | .readyStale => { kind := .progress, retryAfterUs := waitQuantumHardCeilingUs }
  | .projectionMissing =>
      { kind := .progress, retryAfterUs := waitQuantumHardCeilingUs }
  | .failed => { kind := .unavailable, retryAfterUs := 0 }

def firstReceiptAdmitted (elapsedUs : Nat) : Prop :=
  elapsedUs ≤ firstReceiptHardCeilingUs

def warmExactAdmitted (elapsedUs : Nat) : Prop :=
  elapsedUs ≤ warmExactHardCeilingUs

def warmSearchAdmitted (elapsedUs : Nat) : Prop :=
  elapsedUs ≤ warmSearchHardCeilingUs

def BackgroundDeadline := Nat
def InteractiveDeadline := Nat

theorem fortySecondForegroundWaitRejected :
    ¬firstReceiptAdmitted 40000000 := by
  unfold firstReceiptAdmitted firstReceiptHardCeilingUs
  exact Nat.not_le_of_gt (by decide)

theorem everyGenerationStateReturnsProtocolReply (state : GenerationState) :
    (replyFor state).kind = .evidence ∨
    (replyFor state).kind = .progress ∨
    (replyFor state).kind = .unavailable := by
  cases state <;> first | exact Or.inl rfl | exact Or.inr (Or.inl rfl) |
    exact Or.inr (Or.inr rfl)

theorem evidenceRequiresCurrentGeneration
    (state : GenerationState)
    (hEvidence : (replyFor state).kind = .evidence) :
    state = .readyCurrent := by
  cases state <;> cases hEvidence
  rfl

theorem missingGenerationReturnsProgress :
    (replyFor .missing).kind = .progress := by
  rfl

theorem buildingGenerationReturnsProgress :
    (replyFor .building).kind = .progress := by
  rfl

theorem staleGenerationReturnsProgress :
    (replyFor .readyStale).kind = .progress := by
  rfl

theorem terminalFailureReturnsUnavailable :
    (replyFor .failed).kind = .unavailable := by
  rfl

theorem retryAfterIsBounded (state : GenerationState) :
    (replyFor state).retryAfterUs ≤ waitQuantumHardCeilingUs := by
  cases state <;> decide

theorem backgroundDeadlineCannotReplaceFirstReceiptBudget
    (background : BackgroundDeadline)
    (hSlow : firstReceiptHardCeilingUs < background) :
    ¬firstReceiptAdmitted background := by
  exact Nat.not_le_of_gt hSlow

end ASPProof.ASPInteractiveSearchLatency
