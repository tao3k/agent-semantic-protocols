namespace ASPProof

inductive WorkspaceGenerationFailureStage where
  | generationBuilder
  | generationBuilderSupervision
  | workspaceBootstrap
  | durableRestore
  | sourceBuilder
  | sourceIndexCommit
  | canonicalGenerationPublication
  | admissionValidation
  deriving DecidableEq, Repr

inductive WorkspaceGenerationAdmissionState where
  | building
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

structure WorkspaceGenerationDeadline where
  operationMicros : Nat
  terminalPublicationMarginMicros : Nat
  absoluteMicros : Nat

def WorkspaceGenerationDeadline.Valid (deadline : WorkspaceGenerationDeadline) : Prop :=
  deadline.terminalPublicationMarginMicros = 20_000 ∧
    deadline.operationMicros + deadline.terminalPublicationMarginMicros =
      deadline.absoluteMicros

structure WorkspaceGenerationAdmissionReceipt where
  state : WorkspaceGenerationAdmissionState
  failureStage : Option WorkspaceGenerationFailureStage

def WorkspaceGenerationAdmissionReceipt.Valid
    (receipt : WorkspaceGenerationAdmissionReceipt) : Prop :=
  match receipt.state with
  | .building | .ready => receipt.failureStage = none
  | .failed | .cancelled => receipt.failureStage.isSome

inductive WorkspaceGenerationTerminalEvidence where
  | stagedFailure (stage : WorkspaceGenerationFailureStage)
  | genericTimeout
  deriving DecidableEq, Repr

def WorkspaceGenerationTerminalEvidence.Admitted :
    WorkspaceGenerationTerminalEvidence → Prop
  | .stagedFailure _ => True
  | .genericTimeout => False

theorem valid_deadline_reserves_terminal_publication_margin
    (deadline : WorkspaceGenerationDeadline)
    (valid : deadline.Valid) :
    deadline.operationMicros < deadline.absoluteMicros := by
  rcases valid with ⟨margin, total⟩
  rw [← total, margin]
  exact Nat.lt_add_of_pos_right (by decide)

theorem failed_receipt_exposes_failure_stage
    (receipt : WorkspaceGenerationAdmissionReceipt)
    (valid : receipt.Valid)
    (failed : receipt.state = .failed) :
    receipt.failureStage.isSome := by
  simpa [WorkspaceGenerationAdmissionReceipt.Valid, failed] using valid

theorem cancelled_receipt_exposes_failure_stage
    (receipt : WorkspaceGenerationAdmissionReceipt)
    (valid : receipt.Valid)
    (cancelled : receipt.state = .cancelled) :
    receipt.failureStage.isSome := by
  simpa [WorkspaceGenerationAdmissionReceipt.Valid, cancelled] using valid

theorem ready_receipt_hides_failure_stage
    (receipt : WorkspaceGenerationAdmissionReceipt)
    (valid : receipt.Valid)
    (ready : receipt.state = .ready) :
    receipt.failureStage = none := by
  simpa [WorkspaceGenerationAdmissionReceipt.Valid, ready] using valid

theorem generic_timeout_cannot_mask_staged_terminal_evidence :
    ¬ WorkspaceGenerationTerminalEvidence.genericTimeout.Admitted := by
  intro admitted
  exact admitted

theorem admitted_terminal_evidence_identifies_a_stage
    (evidence : WorkspaceGenerationTerminalEvidence)
    (admitted : evidence.Admitted) :
    ∃ stage, evidence = .stagedFailure stage := by
  cases evidence with
  | stagedFailure stage => exact ⟨stage, rfl⟩
  | genericTimeout => contradiction

end ASPProof
