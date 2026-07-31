import ASPProof.Audit.Core
import ASPProof.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

open ASPProof.Audit.Core
open ASPProof.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

def targets : List Target := [
  Target.mk
    ``normalization_is_idempotent
    "certified-normalization-idempotent"
    ["CNU-NORMALIZE", "CNU-ACI"],
  Target.mk
    ``normalized_union_equals_sequential_insertion
    "normalized-union-sequential-insertion-equivalence"
    ["CNU-UNION", "CNU-SEQUENTIAL", "CNU-EXTENSIONAL"],
  Target.mk
    ``normalized_three_way_union_equals_sequential_insertion
    "three-way-normalized-union-sequential-equivalence"
    ["CNU-UNION", "CNU-SEQUENTIAL", "CNU-FANIN"],
  Target.mk
    ``normalized_parallel_fanin_equals_normalized_sequential_fold
    "normalized-parallel-sequential-equivalence"
    ["CNU-PARALLEL", "CNU-SEQUENTIAL"],
  Target.mk
    ``duplicate_evidence_merge_is_commutative
    "duplicate-evidence-union-commutative"
    ["CNU-PROVENANCE", "CNU-ACI"],
  Target.mk
    ``normalized_union_preserves_early_provenance
    "normalized-union-preserves-early-provenance"
    ["CNU-PROVENANCE", "CNU-UNION"],
  Target.mk
    ``premature_destructive_pruning_drops_early_provenance
    "premature-pruning-drops-provenance"
    ["CNU-PRUNING", "CNU-COUNTEREXAMPLE"],
  Target.mk
    ``destructive_pruning_is_not_extensionally_equivalent
    "destructive-pruning-extensional-counterexample"
    ["CNU-PRUNING", "CNU-EXTENSIONAL", "CNU-COUNTEREXAMPLE"],
  Target.mk
    ``same_normalization_identity_authorizes_reuse
    "same-normalization-identity-authorizes-reuse"
    ["CNU-IDENTITY"],
  Target.mk
    ``changed_candidate_universe_rejects_reuse
    "changed-candidate-universe-rejects-reuse"
    ["CNU-IDENTITY", "CNU-UNIVERSE"],
  Target.mk
    ``changed_dominance_certificate_rejects_reuse
    "changed-dominance-certificate-rejects-reuse"
    ["CNU-IDENTITY", "CNU-DOMINANCE"],
  Target.mk
    ``explicit_normalization_receipt_is_capacity_bounded
    "explicit-normalization-receipt-capacity-bound"
    ["CNU-RECEIPT", "CNU-CAPACITY"],
  Target.mk
    ``uncapped_normalization_disclosure_exceeds_every_fixed_bound
    "uncapped-normalization-has-no-fixed-bound"
    ["CNU-RECEIPT", "CNU-CAPACITY"],
  Target.mk
    ``summarized_normalization_receipt_is_count_independent
    "summary-normalization-receipt-constant-size"
    ["CNU-RECEIPT", "CNU-SUMMARY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteCertifiedNormalizedUnionSequentialEquivalence"
    "ASPProof/SearchRouteCertifiedNormalizedUnionSequentialEquivalence.lean"
    targets

elab "writeSearchRouteCertifiedNormalizedUnionSequentialEquivalenceAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-certified-normalized-union-sequential-equivalence-audit-v1.json"
    auditJson

end ASPProof.Audit.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

open ASPProof.Audit.SearchRouteCertifiedNormalizedUnionSequentialEquivalence

writeSearchRouteCertifiedNormalizedUnionSequentialEquivalenceAudit
