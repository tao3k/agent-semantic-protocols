namespace ASPProof.OrgizeTypstLintEvidence

inductive EvidenceLane where
  | proof
  | runtime
  deriving DecidableEq, Repr

inductive HeaderArgClass where
  | lint
  | format
  | runtime
  | other
  deriving DecidableEq, Repr

inductive BindingKind where
  | typstPath
  | exactPath
  deriving DecidableEq, Repr

inductive TerminationOutcome where
  | exited
  | timedOut
  | outputBudgetExceeded
  | spawnFailed
  | ioFailed
  deriving DecidableEq, Repr

inductive ReceiptStatus where
  | accepted
  | rejected
  deriving DecidableEq, Repr

structure SourceContext where
  sourcePath : String
  workingDirectory : String
  packageRoot : String
  deriving DecidableEq, Repr

structure SourceIdentity where
  digest : String
  language : String
  context : SourceContext
  deriving DecidableEq, Repr

structure RuntimeBinding where
  kind : BindingKind
  program : String
  args : List String
  stdin : Bool
  deriving DecidableEq, Repr

structure RuntimePolicy where
  timeoutMs : Nat
  outputByteBudget : Nat
  deriving DecidableEq, Repr

structure RuntimeObservation where
  stdoutBytes : Nat
  stderrBytes : Nat
  elapsedMs : Nat
  terminationOutcome : TerminationOutcome
  exitCode : Option Int
  childReaped : Bool
  deriving DecidableEq, Repr

structure StructuralEvidence where
  lane : EvidenceLane
  source : SourceIdentity
  parserId : String
  parseSucceeded : Bool
  orgLintSucceeded : Bool
  lintHeaderClass : HeaderArgClass
  formatHeaderClass : HeaderArgClass
  runtimeHeaderClass : HeaderArgClass
  deriving DecidableEq, Repr

structure StdinValidationReceipt where
  schema : String
  version : String
  sourceContext : SourceContext
  binding : RuntimeBinding
  policy : RuntimePolicy
  observation : RuntimeObservation
  status : ReceiptStatus
  diagnosticCode : Option String
  deriving DecidableEq, Repr

structure EvidenceBundle where
  structural : Option StructuralEvidence
  runtime : Option StdinValidationReceipt
  deriving DecidableEq, Repr

def ExactRuntimeBinding
    (expected observed : RuntimeBinding) : Prop :=
  observed = expected

def StructuralParseLintSuccess
    (expectedSource : SourceIdentity)
    (evidence : StructuralEvidence) : Prop :=
  evidence.lane = .proof ∧
    evidence.source = expectedSource ∧
    evidence.parseSucceeded = true ∧
    evidence.orgLintSucceeded = true

def HeaderContractValid (evidence : StructuralEvidence) : Prop :=
  evidence.lintHeaderClass = .lint ∧
    evidence.formatHeaderClass = .format ∧
    evidence.runtimeHeaderClass = .runtime

def StructuralValid
    (expectedSource : SourceIdentity)
    (evidence : StructuralEvidence) : Prop :=
  StructuralParseLintSuccess expectedSource evidence ∧
    HeaderContractValid evidence

def EnvelopeValid (receipt : StdinValidationReceipt) : Prop :=
  receipt.schema = "org-babel-runtime-validation-receipt.v1" ∧
    receipt.version = "1"

def SourceContextComplete (context : SourceContext) : Prop :=
  context.sourcePath ≠ "" ∧
    context.workingDirectory ≠ "" ∧
    context.packageRoot ≠ ""

def SourceContextBound
    (expectedSource : SourceIdentity)
    (receipt : StdinValidationReceipt) : Prop :=
  receipt.sourceContext = expectedSource.context ∧
    SourceContextComplete receipt.sourceContext

def BindingValid
    (expectedBinding : RuntimeBinding)
    (receipt : StdinValidationReceipt) : Prop :=
  ExactRuntimeBinding expectedBinding receipt.binding ∧
    receipt.binding.stdin = true

def PolicyBounded (receipt : StdinValidationReceipt) : Prop :=
  0 < receipt.policy.timeoutMs ∧
    receipt.observation.elapsedMs ≤ receipt.policy.timeoutMs

def OutputBounded (receipt : StdinValidationReceipt) : Prop :=
  0 < receipt.policy.outputByteBudget ∧
    receipt.observation.stdoutBytes + receipt.observation.stderrBytes ≤
      receipt.policy.outputByteBudget

def ObservationAccepted (receipt : StdinValidationReceipt) : Prop :=
  receipt.observation.terminationOutcome = .exited ∧
    receipt.observation.exitCode = some 0 ∧
    receipt.observation.childReaped = true

def RuntimeValid
    (expectedSource : SourceIdentity)
    (expectedBinding : RuntimeBinding)
    (receipt : StdinValidationReceipt) : Prop :=
  EnvelopeValid receipt ∧
    SourceContextBound expectedSource receipt ∧
    BindingValid expectedBinding receipt ∧
    PolicyBounded receipt ∧
    OutputBounded receipt ∧
    ObservationAccepted receipt ∧
    receipt.status = .accepted

def Accepted
    (expectedSource : SourceIdentity)
    (expectedBinding : RuntimeBinding)
    (bundle : EvidenceBundle) : Prop :=
  ∃ structural runtime,
    bundle.structural = some structural ∧
      bundle.runtime = some runtime ∧
      StructuralValid expectedSource structural ∧
      RuntimeValid expectedSource expectedBinding runtime

theorem structural_valid_implies_proof_lane
    (source : SourceIdentity) (evidence : StructuralEvidence)
    (h : StructuralValid source evidence) : evidence.lane = .proof := by
  exact h.1.1

theorem exact_binding_preserves_program
    (expected observed : RuntimeBinding)
    (h : ExactRuntimeBinding expected observed) :
    observed.program = expected.program := by
  rw [h]

theorem runtime_valid_requires_schema_version
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) : EnvelopeValid receipt := by
  exact h.1

theorem runtime_valid_requires_exited
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) :
    receipt.observation.terminationOutcome = .exited := by
  exact h.2.2.2.2.2.1.1

theorem runtime_valid_requires_exit_zero
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) :
    receipt.observation.exitCode = some 0 := by
  exact h.2.2.2.2.2.1.2.1

theorem runtime_valid_requires_reaped
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) :
    receipt.observation.childReaped = true := by
  exact h.2.2.2.2.2.1.2.2

theorem runtime_valid_requires_exact_context
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) :
    receipt.sourceContext = source.context := by
  exact h.2.1.1

theorem runtime_valid_requires_package_root
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) :
    receipt.sourceContext.packageRoot ≠ "" := by
  exact h.2.1.2.2.2

theorem runtime_valid_requires_combined_budget
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) : OutputBounded receipt := by
  exact h.2.2.2.2.1

theorem runtime_valid_requires_accepted_status
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (h : RuntimeValid source binding receipt) : receipt.status = .accepted := by
  exact h.2.2.2.2.2.2

theorem acceptance_preserved_by_valid_receipts
    (source : SourceIdentity) (binding : RuntimeBinding)
    (structural : StructuralEvidence) (runtime : StdinValidationReceipt)
    (hs : StructuralValid source structural)
    (hr : RuntimeValid source binding runtime) :
    Accepted source binding { structural := some structural, runtime := some runtime } := by
  exact ⟨structural, runtime, rfl, rfl, hs, hr⟩

theorem acceptance_requires_structural_evidence
    (source : SourceIdentity) (binding : RuntimeBinding) (bundle : EvidenceBundle)
    (h : Accepted source binding bundle) :
    ∃ structural, bundle.structural = some structural ∧
      StructuralValid source structural := by
  rcases h with ⟨structural, _, hs, _, hv, _⟩
  exact ⟨structural, hs, hv⟩

theorem acceptance_requires_runtime_evidence
    (source : SourceIdentity) (binding : RuntimeBinding) (bundle : EvidenceBundle)
    (h : Accepted source binding bundle) :
    ∃ runtime, bundle.runtime = some runtime ∧ RuntimeValid source binding runtime := by
  rcases h with ⟨_, runtime, _, hr, _, hv⟩
  exact ⟨runtime, hr, hv⟩

theorem missing_runtime_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (structural : StructuralEvidence) :
    ¬ Accepted source binding { structural := some structural, runtime := none } := by
  intro h
  rcases h with ⟨_, runtime, _, hr, _, _⟩
  cases hr

theorem missing_structural_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (runtime : StdinValidationReceipt) :
    ¬ Accepted source binding { structural := none, runtime := some runtime } := by
  intro h
  rcases h with ⟨structural, _, hs, _, _, _⟩
  cases hs

theorem wrong_runtime_binding_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hWrong : receipt.binding ≠ binding) : ¬ RuntimeValid source binding receipt := by
  intro h
  exact hWrong h.2.2.1.1

theorem missing_package_root_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hMissing : receipt.sourceContext.packageRoot = "") :
    ¬ RuntimeValid source binding receipt := by
  intro h
  exact h.2.1.2.2.2 hMissing

theorem mismatched_package_root_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hMismatch : receipt.sourceContext.packageRoot ≠ source.context.packageRoot) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  exact hMismatch (congrArg SourceContext.packageRoot h.2.1.1)

theorem zero_timeout_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt) (hZero : receipt.policy.timeoutMs = 0) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hp : 0 < receipt.policy.timeoutMs := h.2.2.2.1.1
  rw [hZero] at hp
  exact Nat.not_lt_zero 0 hp

theorem zero_output_budget_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hZero : receipt.policy.outputByteBudget = 0) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hp : 0 < receipt.policy.outputByteBudget := h.2.2.2.2.1.1
  rw [hZero] at hp
  exact Nat.not_lt_zero 0 hp

theorem output_budget_exceeded_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hExceeded : receipt.policy.outputByteBudget <
      receipt.observation.stdoutBytes + receipt.observation.stderrBytes) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  exact (Nat.not_lt_of_ge h.2.2.2.2.1.2) hExceeded

theorem elapsed_timeout_fails_closed
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hExceeded : receipt.policy.timeoutMs < receipt.observation.elapsedMs) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  exact (Nat.not_lt_of_ge h.2.2.2.1.2) hExceeded

theorem timed_out_outcome_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hOutcome : receipt.observation.terminationOutcome = .timedOut) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exited source binding receipt h
  rw [hOutcome] at hx
  exact TerminationOutcome.noConfusion hx

theorem output_budget_exceeded_outcome_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hOutcome : receipt.observation.terminationOutcome = .outputBudgetExceeded) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exited source binding receipt h
  rw [hOutcome] at hx
  exact TerminationOutcome.noConfusion hx

theorem spawn_failed_outcome_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hOutcome : receipt.observation.terminationOutcome = .spawnFailed) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exited source binding receipt h
  rw [hOutcome] at hx
  exact TerminationOutcome.noConfusion hx

theorem io_failed_outcome_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hOutcome : receipt.observation.terminationOutcome = .ioFailed) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exited source binding receipt h
  rw [hOutcome] at hx
  exact TerminationOutcome.noConfusion hx

theorem exited_without_exit_code_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hMissing : receipt.observation.exitCode = none) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exit_zero source binding receipt h
  rw [hMissing] at hx
  cases hx

theorem nonzero_exit_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt) (code : Int)
    (hCode : receipt.observation.exitCode = some code) (hNonzero : code ≠ 0) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_exit_zero source binding receipt h
  rw [hCode] at hx
  exact hNonzero (Option.some.inj hx)

theorem unreaped_child_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (hUnreaped : receipt.observation.childReaped = false) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_reaped source binding receipt h
  rw [hUnreaped] at hx
  exact Bool.noConfusion hx

theorem rejected_status_rejected
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt) (hStatus : receipt.status = .rejected) :
    ¬ RuntimeValid source binding receipt := by
  intro h
  have hx := runtime_valid_requires_accepted_status source binding receipt h
  rw [hStatus] at hx
  exact ReceiptStatus.noConfusion hx

theorem accepted_status_cannot_launder_failure_outcome
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (_hStatus : receipt.status = .accepted)
    (hFailure : receipt.observation.terminationOutcome = .spawnFailed) :
    ¬ RuntimeValid source binding receipt := by
  exact spawn_failed_outcome_rejected source binding receipt hFailure

theorem zero_exit_cannot_launder_rejected_status
    (source : SourceIdentity) (binding : RuntimeBinding)
    (receipt : StdinValidationReceipt)
    (_hZero : receipt.observation.exitCode = some 0)
    (hRejected : receipt.status = .rejected) :
    ¬ RuntimeValid source binding receipt := by
  exact rejected_status_rejected source binding receipt hRejected

theorem untyped_lint_header_fails_closed
    (source : SourceIdentity) (evidence : StructuralEvidence)
    (hOther : evidence.lintHeaderClass = .other) :
    ¬ StructuralValid source evidence := by
  intro h
  have hx : evidence.lintHeaderClass = .lint := h.2.1
  rw [hOther] at hx
  exact HeaderArgClass.noConfusion hx

theorem untyped_format_header_fails_closed
    (source : SourceIdentity) (evidence : StructuralEvidence)
    (hOther : evidence.formatHeaderClass = .other) :
    ¬ StructuralValid source evidence := by
  intro h
  have hx : evidence.formatHeaderClass = .format := h.2.2.1
  rw [hOther] at hx
  exact HeaderArgClass.noConfusion hx

theorem untyped_runtime_header_fails_closed
    (source : SourceIdentity) (evidence : StructuralEvidence)
    (hOther : evidence.runtimeHeaderClass = .other) :
    ¬ StructuralValid source evidence := by
  intro h
  have hx : evidence.runtimeHeaderClass = .runtime := h.2.2.2
  rw [hOther] at hx
  exact HeaderArgClass.noConfusion hx

def counterSource : SourceIdentity := {
  digest := "sha256:org-source"
  language := "typst"
  context := {
    sourcePath := "docs/counterexample.org"
    workingDirectory := "docs"
    packageRoot := "."
  }
}

def counterBinding : RuntimeBinding := {
  kind := .typstPath
  program := "typst"
  args := ["eval", "--in", "-", "\"ok\""]
  stdin := true
}

def counterStructural : StructuralEvidence := {
  lane := .proof
  source := counterSource
  parserId := "orgize"
  parseSucceeded := true
  orgLintSucceeded := true
  lintHeaderClass := .other
  formatHeaderClass := .other
  runtimeHeaderClass := .other
}

def counterRuntime : StdinValidationReceipt := {
  schema := "org-babel-runtime-validation-receipt.v1"
  version := "1"
  sourceContext := counterSource.context
  binding := counterBinding
  policy := { timeoutMs := 1000, outputByteBudget := 1024 }
  observation := {
    stdoutBytes := 0
    stderrBytes := 64
    elapsedMs := 1
    terminationOutcome := .spawnFailed
    exitCode := some 0
    childReaped := true
  }
  status := .accepted
  diagnosticCode := some "ORG-TYPST-SPAWN-FAILED"
}

theorem structural_success_counterexample :
    StructuralParseLintSuccess counterSource counterStructural ∧
      ¬ RuntimeValid counterSource counterBinding counterRuntime := by
  exact ⟨⟨rfl, rfl, rfl, rfl⟩,
    accepted_status_cannot_launder_failure_outcome
      counterSource counterBinding counterRuntime rfl rfl⟩

theorem structural_success_does_not_imply_runtime_validity :
    ¬ (∀ source binding structural runtime,
      StructuralParseLintSuccess source structural →
        RuntimeValid source binding runtime) := by
  intro h
  exact structural_success_counterexample.2
    (h counterSource counterBinding counterStructural counterRuntime
      structural_success_counterexample.1)

theorem structural_evidence_cannot_be_laundered_as_acceptance :
    ¬ Accepted counterSource counterBinding {
      structural := some counterStructural
      runtime := some counterRuntime
    } := by
  intro h
  rcases acceptance_requires_runtime_evidence
    counterSource counterBinding _ h with ⟨runtime, hs, hv⟩
  cases hs
  exact structural_success_counterexample.2 hv

end ASPProof.OrgizeTypstLintEvidence
