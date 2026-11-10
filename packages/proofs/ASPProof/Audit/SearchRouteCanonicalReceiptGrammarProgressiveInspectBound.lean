-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

open ASPProof.Audit.Core
open ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

def targets : List Target := [
  Target.mk ``separator_count_zero
    "empty-array-separator-count" ["CRGI-SEPARATOR", "CRGI-EMPTY"],
  Target.mk ``separator_count_succ
    "nonempty-array-separator-count" ["CRGI-SEPARATOR", "CRGI-EXACT"],
  Target.mk ``separator_count_is_bounded_by_item_count
    "separator-capacity-bound" ["CRGI-SEPARATOR", "CRGI-CAPACITY"],
  Target.mk ``empty_json_array_has_two_bytes
    "empty-array-framing" ["CRGI-JSON", "CRGI-EMPTY"],
  Target.mk ``json_array_bytes_are_capacity_bounded
    "array-encoded-byte-bound" ["CRGI-JSON", "CRGI-CAPACITY"],
  Target.mk ``item_count_cap_without_item_byte_cap_does_not_bound_payload
    "count-cap-insufficient" ["CRGI-ITEM", "CRGI-COUNTEREXAMPLE"],
  Target.mk ``overflow_summary_is_bounded_by_encoded_count_cap
    "overflow-count-byte-bound" ["CRGI-OVERFLOW", "CRGI-COUNT"],
  Target.mk ``fixed_digest_without_count_byte_cap_does_not_bound_overflow_summary
    "overflow-root-insufficient" ["CRGI-OVERFLOW", "CRGI-COUNTEREXAMPLE"],
  Target.mk ``canonical_projection_bytes_are_capacity_bounded
    "canonical-projection-bound" ["CRGI-PROJECTION", "CRGI-CAPACITY"],
  Target.mk ``authorized_inspect_preserves_step_budget
    "inspect-step-invariant" ["CRGI-INSPECT", "CRGI-STEPS"],
  Target.mk ``authorized_inspect_preserves_byte_budget
    "inspect-byte-invariant" ["CRGI-INSPECT", "CRGI-BYTES"],
  Target.mk ``exhausted_step_budget_blocks_inspect
    "inspect-step-exhaustion" ["CRGI-INSPECT", "CRGI-FAIL-CLOSED"],
  Target.mk ``inspect_step_sum_is_bounded_by_length
    "inspect-list-sum-bound" ["CRGI-INSPECT", "CRGI-SEQUENCE"],
  Target.mk ``inspect_sequence_bytes_are_globally_bounded
    "inspect-global-byte-bound" ["CRGI-INSPECT", "CRGI-GLOBAL"],
  Target.mk ``per_step_cap_without_interaction_cap_does_not_bound_total
    "per-step-cap-insufficient" ["CRGI-INSPECT", "CRGI-COUNTEREXAMPLE"],
  Target.mk ``unchanged_inspect_identity_is_compatible
    "inspect-identity-reflexive" ["CRGI-IDENTITY", "CRGI-INSPECT"],
  Target.mk ``changed_inspect_depth_invalidates_identity
    "inspect-depth-invalidates" ["CRGI-IDENTITY", "CRGI-DEPTH"],
  Target.mk ``changed_inspect_snapshot_invalidates_identity
    "inspect-snapshot-invalidates" ["CRGI-IDENTITY", "CRGI-SNAPSHOT"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound"
    "ASPProof/SearchRouteCanonicalReceiptGrammarProgressiveInspectBound.lean"
    targets

end ASPProof.Audit.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

open ASPProof.Audit.SearchRouteCanonicalReceiptGrammarProgressiveInspectBound

elab "writeSearchRouteCanonicalReceiptGrammarProgressiveInspectBoundAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-canonical-receipt-grammar-progressive-inspect-bound-audit-v1.json"
    auditJson
