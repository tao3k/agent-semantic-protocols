import ASPProof.SearchRouteSafeReplayModeNegotiation
import ASPProof.Audit.Core

open ASPProof.Audit.Core
open Lean Elab Command

private def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.absent_collision_evidence_never_selects_digest
    theoremFamily := "absent-collision-evidence-never-selects-digest"
    rfcClauseIds := ["SRM-MODE", "SRM-ASSUMPTION"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.collision_and_digest_evidence_select_digest
    theoremFamily := "collision-and-digest-evidence-select-digest"
    rfcClauseIds := ["SRM-MODE", "SRM-DIGEST"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.absent_collision_with_canonical_evidence_selects_full_field
    theoremFamily := "absent-collision-selects-full-field"
    rfcClauseIds := ["SRM-MODE", "SRM-FULL-FIELD"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.no_identity_evidence_selects_fallback
    theoremFamily := "missing-identity-evidence-selects-fallback"
    rfcClauseIds := ["SRM-MODE", "SRM-FALLBACK"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.negotiated_digest_has_collision_evidence
    theoremFamily := "negotiated-digest-has-collision-evidence"
    rfcClauseIds := ["SRM-ASSUMPTION", "SRM-DIGEST"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.accepted_outcome_implies_payload_equality
    theoremFamily := "accepted-mode-payload-identity"
    rfcClauseIds := ["SRM-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.digest_outcome_receipt_has_assumption_identity
    theoremFamily := "digest-receipt-assumption-identity"
    rfcClauseIds := ["SRM-ASSUMPTION", "SRM-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.full_field_outcome_has_no_assumption_identity
    theoremFamily := "full-field-receipt-no-assumption"
    rfcClauseIds := ["SRM-FULL-FIELD", "SRM-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.full_and_digest_evidence_agree_on_payload_identity
    theoremFamily := "full-and-digest-evidence-agreement"
    rfcClauseIds := ["SRM-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.negotiated_full_and_digest_modes_agree_on_payload_identity
    theoremFamily := "full-and-digest-mode-agreement"
    rfcClauseIds := ["SRM-MODE", "SRM-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteSafeReplayModeNegotiation.digest_mode_receipt_token_cost_is_smaller
    theoremFamily := "digest-mode-receipt-smaller"
    rfcClauseIds := ["SRM-RECEIPT"]
  }
]

elab "emitSearchRouteSafeReplayModeNegotiationAudit" : command => do
  let receipt ← liftTermElabM <| proofAuditJson
    "ASPProof.SearchRouteSafeReplayModeNegotiation"
    "ASPProof/SearchRouteSafeReplayModeNegotiation.lean"
    targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-safe-replay-mode-negotiation-audit-v1.json"
    receipt.pretty
  logInfo receipt.compress

emitSearchRouteSafeReplayModeNegotiationAudit
