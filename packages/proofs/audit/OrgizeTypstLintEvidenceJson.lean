import Lean
import ASPProof.OrgizeTypstLintEvidence

open Lean Elab Command

namespace ASPProof.OrgizeTypstLintEvidenceJsonAudit

structure DeclarationAudit where
  declaration : String
  axioms : Array String
  deriving ToJson

structure AuditReceipt where
  schema : String
  moduleName : String
  source : String
  declarations : Array DeclarationAudit
  axiomFreeDeclarations : Array String
  axiomDependentDeclarations : Array String
  axiomInventory : Array String
  sorryAx : Bool
  theoremFamilies : Array String
  clauseCoverage : Array String
  counterexamples : Array String
  deriving ToJson

def declarationNames : Array Name := #[
  ``ASPProof.OrgizeTypstLintEvidence.structural_valid_implies_proof_lane,
  ``ASPProof.OrgizeTypstLintEvidence.exact_binding_preserves_program,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_schema_version,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_exited,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_exit_zero,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_reaped,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_exact_context,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_package_root,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_combined_budget,
  ``ASPProof.OrgizeTypstLintEvidence.runtime_valid_requires_accepted_status,
  ``ASPProof.OrgizeTypstLintEvidence.acceptance_preserved_by_valid_receipts,
  ``ASPProof.OrgizeTypstLintEvidence.acceptance_requires_structural_evidence,
  ``ASPProof.OrgizeTypstLintEvidence.acceptance_requires_runtime_evidence,
  ``ASPProof.OrgizeTypstLintEvidence.missing_runtime_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.missing_structural_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.wrong_runtime_binding_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.missing_package_root_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.mismatched_package_root_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.zero_timeout_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.zero_output_budget_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.output_budget_exceeded_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.elapsed_timeout_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.timed_out_outcome_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.output_budget_exceeded_outcome_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.spawn_failed_outcome_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.io_failed_outcome_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.exited_without_exit_code_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.nonzero_exit_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.unreaped_child_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.rejected_status_rejected,
  ``ASPProof.OrgizeTypstLintEvidence.accepted_status_cannot_launder_failure_outcome,
  ``ASPProof.OrgizeTypstLintEvidence.zero_exit_cannot_launder_rejected_status,
  ``ASPProof.OrgizeTypstLintEvidence.untyped_lint_header_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.untyped_format_header_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.untyped_runtime_header_fails_closed,
  ``ASPProof.OrgizeTypstLintEvidence.structural_success_counterexample,
  ``ASPProof.OrgizeTypstLintEvidence.structural_success_does_not_imply_runtime_validity,
  ``ASPProof.OrgizeTypstLintEvidence.structural_evidence_cannot_be_laundered_as_acceptance
]

def appendUnique (values additions : Array String) : Array String :=
  additions.foldl
    (fun result value => if result.contains value then result else result.push value)
    values

run_cmd do
  let declarations ← declarationNames.mapM fun declaration => do
    let axioms ← Lean.collectAxioms declaration
    pure { declaration := declaration.toString, axioms := axioms.map Name.toString }
  let axiomFreeDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then result.push declaration.declaration else result)
    #[]
  let axiomDependentDeclarations := declarations.foldl
    (fun result declaration =>
      if declaration.axioms.isEmpty then result else result.push declaration.declaration)
    #[]
  let axiomInventory := declarations.foldl
    (fun result declaration => appendUnique result declaration.axioms) #[]
  let receipt : AuditReceipt := {
    schema := "asp.lean-proof-audit.v1"
    moduleName := "ASPProof.OrgizeTypstLintEvidence"
    source := "packages/proofs/ASPProof/OrgizeTypstLintEvidence.lean"
    declarations
    axiomFreeDeclarations
    axiomDependentDeclarations
    axiomInventory
    sorryAx := axiomInventory.contains "sorryAx"
    theoremFamilies := #[
      "structural-runtime-evidence-separation",
      "v1-envelope-and-exact-binding",
      "typed-source-context",
      "bounded-time-and-combined-output",
      "typed-termination-outcomes",
      "receipt-status-non-laundering",
      "dual-evidence-admission-and-countermodel"
    ]
    clauseCoverage := #[
      "ORG-TYPST-LINT-STRUCTURAL-001",
      "ORG-TYPST-LINT-RUNTIME-002",
      "ORG-TYPST-LINT-BINDING-003",
      "ORG-TYPST-LINT-STDIN-004",
      "ORG-TYPST-LINT-EXIT-005",
      "ORG-TYPST-LINT-TIMEOUT-006",
      "ORG-TYPST-LINT-SIDE-EFFECT-007",
      "ORG-TYPST-LINT-ACCEPTANCE-008",
      "ORG-TYPST-LINT-IMPLICIT-009",
      "ORG-TYPST-LINT-BOUNDED-RUNTIME-010",
      "ORG-TYPST-LINT-SOURCE-CONTEXT-011",
      "ORG-TYPST-LINT-OUTPUT-BOUND-012",
      "ORG-TYPST-LINT-RUNTIME-RECEIPT-013"
    ]
    counterexamples := #[
      "structural-success-does-not-imply-runtime-validity",
      "failure-outcome-with-accepted-status-is-rejected",
      "zero-exit-with-rejected-status-is-rejected",
      "missing-or-mismatched-package-root-is-rejected",
      "per-stream-success-does-not-imply-combined-budget",
      "untyped-other-header-does-not-establish-runtime-evidence"
    ]
  }
  liftIO <| IO.println (toJson receipt).compress

end ASPProof.OrgizeTypstLintEvidenceJsonAudit
