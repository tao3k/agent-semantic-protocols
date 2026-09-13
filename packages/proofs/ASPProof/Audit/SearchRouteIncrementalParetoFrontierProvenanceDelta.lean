-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteIncrementalParetoFrontierProvenanceDelta

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteIncrementalParetoFrontierProvenanceDelta

open ASPProof.Audit.Core
open ASPProof.SearchRouteIncrementalParetoFrontierProvenanceDelta

def targets : List Target := [
  Target.mk ``strict_dominance_excludes_reverse_no_worse
    "strict-dominance-asymmetry" ["IPFD-DOMINANCE", "IPFD-ORDER"],
  Target.mk ``cost_equivalence_excludes_strict_dominance
    "cost-equivalence-allows-provenance" ["IPFD-EQUIVALENCE", "IPFD-PROVENANCE"],
  Target.mk ``undominated_fresh_insertion_preserves_strict_antichain
    "insert-preserves-strict-antichain" ["IPFD-INSERT", "IPFD-ANTICHAIN"],
  Target.mk ``insertion_preserves_pareto_coverage
    "insert-preserves-coverage" ["IPFD-INSERT", "IPFD-COVERAGE"],
  Target.mk ``equal_cost_distinct_provenance_routes_survive_insertion
    "equal-cost-provenance-survives" ["IPFD-EQUIVALENCE", "IPFD-INSERT"],
  Target.mk ``equal_cost_routes_can_have_distinct_canonical_identity
    "cost-equality-not-route-identity" ["IPFD-IDENTITY", "IPFD-PROVENANCE"],
  Target.mk ``canonical_identity_alone_can_hide_cost_drift
    "canonical-id-needs-cost-consistency" ["IPFD-IDENTITY", "IPFD-COUNTEREXAMPLE"],
  Target.mk ``explicit_delta_is_bounded_by_frontier_capacity
    "explicit-delta-frontier-bound" ["IPFD-DELTA", "IPFD-CAPACITY"],
  Target.mk ``uncapped_explicit_removals_have_no_fixed_delta_bound
    "uncapped-removal-delta" ["IPFD-DELTA", "IPFD-COUNTEREXAMPLE"],
  Target.mk ``summarized_delta_is_independent_of_removal_count
    "summary-delta-constant" ["IPFD-DELTA", "IPFD-SUMMARY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteIncrementalParetoFrontierProvenanceDelta"
    "ASPProof/SearchRouteIncrementalParetoFrontierProvenanceDelta.lean"
    targets

end ASPProof.Audit.SearchRouteIncrementalParetoFrontierProvenanceDelta

open ASPProof.Audit.SearchRouteIncrementalParetoFrontierProvenanceDelta

elab "writeSearchRouteIncrementalParetoFrontierProvenanceDeltaAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-incremental-pareto-frontier-provenance-delta-audit-v1.json"
    auditJson
