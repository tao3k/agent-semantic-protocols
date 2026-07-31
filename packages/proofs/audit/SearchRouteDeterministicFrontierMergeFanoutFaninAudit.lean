import ASPProof.Audit.Core
import ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteDeterministicFrontierMergeFanoutFanin

open ASPProof.Audit.Core
open ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin

def targets : List Target := [
  Target.mk
    ``mergeMembership_commutative
    "canonical-membership-merge-commutative"
    ["DFM-MEMBERSHIP", "DFM-ACI"],
  Target.mk
    ``mergeMembership_associative
    "canonical-membership-merge-associative"
    ["DFM-MEMBERSHIP", "DFM-ACI"],
  Target.mk
    ``mergeMembership_idempotent
    "canonical-membership-merge-idempotent"
    ["DFM-MEMBERSHIP", "DFM-ACI"],
  Target.mk
    ``parallel_fanin_equals_sequential_fold
    "parallel-fanin-sequential-equivalence"
    ["DFM-PARALLEL", "DFM-ACI"],
  Target.mk
    ``equal_route_provenance_union_commutative
    "equal-route-provenance-set-union"
    ["DFM-PROVENANCE", "DFM-ACI"],
  Target.mk
    ``reflexive_evaluation_identity_authorizes_merge
    "same-evaluation-identity-authorizes-merge"
    ["DFM-IDENTITY"],
  Target.mk
    ``changed_snapshot_rejects_merge
    "changed-snapshot-rejects-merge"
    ["DFM-IDENTITY", "DFM-STALE"],
  Target.mk
    ``changed_duplicate_policy_rejects_merge
    "changed-duplicate-policy-rejects-merge"
    ["DFM-IDENTITY", "DFM-POLICY"],
  Target.mk
    ``last_writer_wins_is_not_commutative
    "last-writer-wins-counterexample"
    ["DFM-PROVENANCE", "DFM-COUNTEREXAMPLE"],
  Target.mk
    ``two_partial_frontiers_can_remain_partial
    "partial-fanin-does-not-imply-completeness"
    ["DFM-COVERAGE"],
  Target.mk
    ``explicit_merge_receipt_is_bounded
    "explicit-merge-receipt-capacity-bound"
    ["DFM-RECEIPT", "DFM-CAPACITY"],
  Target.mk
    ``uncapped_explicit_merge_exceeds_every_fixed_bound
    "uncapped-explicit-merge-has-no-fixed-bound"
    ["DFM-RECEIPT", "DFM-CAPACITY"],
  Target.mk
    ``summarized_merge_receipt_is_entry_count_independent
    "summary-merge-receipt-constant-size"
    ["DFM-RECEIPT", "DFM-SUMMARY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteDeterministicFrontierMergeFanoutFanin"
    "ASPProof/SearchRouteDeterministicFrontierMergeFanoutFanin.lean"
    targets

elab "writeSearchRouteDeterministicFrontierMergeFanoutFaninAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-deterministic-frontier-merge-fanout-fanin-audit-v1.json"
    auditJson

end ASPProof.Audit.SearchRouteDeterministicFrontierMergeFanoutFanin

open ASPProof.Audit.SearchRouteDeterministicFrontierMergeFanoutFanin

writeSearchRouteDeterministicFrontierMergeFanoutFaninAudit
