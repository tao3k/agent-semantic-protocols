import ASPProof.Audit.Core
import ASPProof.PolyglotSearchConformance

namespace ASPProof.Audit

def writePolyglotSearchConformanceReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.PolyglotSearchConformance

open ASPProof.Audit.Core
open ASPProof.PolyglotSearchConformance

def targets : List Target := [
  Target.mk ``valid_document_is_admitted "document-positive"
    ["ASP-RFC-PSC-DOCUMENT", "ASP-RFC-PSC-PARSER"],
  Target.mk ``gql_mutation_is_rejected "gql-mutation-rejection"
    ["ASP-RFC-PSC-GQL", "ASP-RFC-PSC-READ-ONLY"],
  Target.mk ``safe_summary_beside_mutating_source_is_rejected "source-summary-binding"
    ["ASP-RFC-PSC-PARSER", "ASP-RFC-PSC-BINDING"],
  Target.mk ``unbounded_logic_evaluation_is_rejected "logic-evaluation-bound"
    ["ASP-RFC-PSC-LOGIC", "ASP-RFC-PSC-BOUND"],
  Target.mk ``valid_relation_batch_is_admitted "relation-positive"
    ["ASP-RFC-PSC-RELATION", "ASP-RFC-PSC-ABI"],
  Target.mk ``stale_relation_snapshot_is_rejected "relation-snapshot-drift"
    ["ASP-RFC-PSC-RELATION", "ASP-RFC-PSC-SNAPSHOT"],
  Target.mk ``relation_row_schema_mismatch_is_rejected "relation-row-schema"
    ["ASP-RFC-PSC-RELATION", "ASP-RFC-PSC-ABI"],
  Target.mk ``turbo_feature_authority_elevation_is_rejected "turbo-authority"
    ["ASP-RFC-PSC-TURBO", "ASP-RFC-PSC-AUTHORITY"],
  Target.mk ``valid_progressive_frontier_is_admitted "frontier-positive"
    ["ASP-RFC-PSC-FRONTIER", "ASP-RFC-PSC-BOUND"],
  Target.mk ``unexposed_selection_is_rejected "unexposed-selection"
    ["ASP-RFC-PSC-FRONTIER", "ASP-RFC-PSC-SELECTION"],
  Target.mk ``stale_frontier_continuation_is_rejected "stale-continuation"
    ["ASP-RFC-PSC-FRONTIER", "ASP-RFC-PSC-CONTINUATION"],
  Target.mk ``duplicate_visible_candidates_are_rejected "frontier-deduplication"
    ["ASP-RFC-PSC-FRONTIER", "ASP-RFC-PSC-DETERMINISM"],
  Target.mk ``valid_certificate_is_admitted "certificate-positive"
    ["ASP-RFC-PSC-CERTIFICATE", "ASP-RFC-PSC-CUTOVER"],
  Target.mk ``certificate_must_bind_exact_projected_packet "certificate-packet-binding"
    ["ASP-RFC-PSC-CERTIFICATE", "ASP-RFC-PSC-TOCTOU"],
  Target.mk ``runtime_drift_certificate_is_rejected "certificate-runtime-drift"
    ["ASP-RFC-PSC-CERTIFICATE", "ASP-RFC-PSC-RUNTIME"],
  Target.mk ``valid_pipeline_is_executable "pipeline-positive"
    ["ASP-RFC-PSC-PIPELINE", "ASP-RFC-PSC-REFINEMENT"],
  Target.mk ``refinement_exposes_document_conformance "refinement-document"
    ["ASP-RFC-PSC-PIPELINE", "ASP-RFC-PSC-REFINEMENT"],
  Target.mk ``refinement_exposes_relation_conformance "refinement-relation"
    ["ASP-RFC-PSC-PIPELINE", "ASP-RFC-PSC-REFINEMENT"],
  Target.mk ``refinement_exposes_frontier_conformance "refinement-frontier"
    ["ASP-RFC-PSC-PIPELINE", "ASP-RFC-PSC-REFINEMENT"],
  Target.mk ``refinement_exposes_certificate_conformance "refinement-certificate"
    ["ASP-RFC-PSC-PIPELINE", "ASP-RFC-PSC-REFINEMENT"],
  Target.mk ``rejected_document_preserves_state "rejected-document-pure"
    ["ASP-RFC-PSC-FAIL-CLOSED", "ASP-RFC-PSC-STATE"],
  Target.mk ``rejected_relation_batch_preserves_state "rejected-relation-pure"
    ["ASP-RFC-PSC-FAIL-CLOSED", "ASP-RFC-PSC-STATE"],
  Target.mk ``rejected_frontier_preserves_state "rejected-frontier-pure"
    ["ASP-RFC-PSC-FAIL-CLOSED", "ASP-RFC-PSC-STATE"],
  Target.mk ``rejected_certificate_preserves_state "rejected-certificate-pure"
    ["ASP-RFC-PSC-FAIL-CLOSED", "ASP-RFC-PSC-STATE"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.PolyglotSearchConformance"
    "ASPProof/PolyglotSearchConformance.lean"
    targets

end ASPProof.Audit.PolyglotSearchConformance

open ASPProof.Audit.PolyglotSearchConformance

elab "writePolyglotSearchConformanceAudit" : command =>
  ASPProof.Audit.writePolyglotSearchConformanceReceipt
    "receipts/polyglot-search-conformance-audit-v1.json"
    auditJson
