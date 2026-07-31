import ASPProof.Audit.Core
import ASPProof.SearchRouteDominanceTransitiveShortcut

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.bool_and_true_of_parts
    theoremFamily := "constructive-bool-and-construction"
    rfcClauseIds := ["DTS-CONSTRUCTIVE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.bool_or_true_of_left
    theoremFamily := "constructive-bool-or-left"
    rfcClauseIds := ["DTS-CONSTRUCTIVE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.bool_or_true_of_right
    theoremFamily := "constructive-bool-or-right"
    rfcClauseIds := ["DTS-CONSTRUCTIVE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.le_implies_natLe_true
    theoremFamily := "executable-natle-completeness"
    rfcClauseIds := ["DTS-COMPONENT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.noWorse_true_to_components
    theoremFamily := "noworse-to-componentwise-order"
    rfcClauseIds := ["DTS-COMPONENT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.components_to_noWorse_true
    theoremFamily := "componentwise-order-to-noworse"
    rfcClauseIds := ["DTS-COMPONENT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.strictlyImproves_true_to_dimension
    theoremFamily := "strict-improvement-dimension-witness"
    rfcClauseIds := ["DTS-STRICT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.dimension_to_strictlyImproves_true
    theoremFamily := "dimension-witness-to-strict-improvement"
    rfcClauseIds := ["DTS-STRICT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.costNoWorse_is_transitive
    theoremFamily := "componentwise-noworse-transitivity"
    rfcClauseIds := ["DTS-NOWORSE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.strictImprovement_followed_by_noWorse
    theoremFamily := "strict-improvement-survives-noworse-composition"
    rfcClauseIds := ["DTS-STRICT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.noWorse_is_transitive
    theoremFamily := "executable-noworse-transitivity"
    rfcClauseIds := ["DTS-NOWORSE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.strict_dominance_is_transitive
    theoremFamily := "strict-dominance-transitivity"
    rfcClauseIds := ["DTS-DOMINANCE"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.chain_endpoint_eq_or_strictly_dominates
    theoremFamily := "chain-endpoint-equality-or-direct-dominance"
    rfcClauseIds := ["DTS-CHAIN"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.nonempty_chain_endpoint_strictly_dominates_start
    theoremFamily := "nonempty-chain-endpoint-direct-dominance"
    rfcClauseIds := ["DTS-NONEMPTY", "DTS-CHAIN"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.buildTransitiveShortcut
    theoremFamily := "transitive-shortcut-construction"
    rfcClauseIds := ["DTS-SHORTCUT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.shortcut_preserves_removal_soundness
    theoremFamily := "shortcut-preserves-removal-soundness"
    rfcClauseIds := ["DTS-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.shortcut_receipt_is_length_independent
    theoremFamily := "shortcut-receipt-constant-shape"
    rfcClauseIds := ["DTS-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteDominanceTransitiveShortcut.shortcut_receipt_beats_nonempty_explicit_chain
    theoremFamily := "shortcut-receipt-beats-explicit-nonempty-chain"
    rfcClauseIds := ["DTS-RECEIPT"]
  }
]

elab "#writeDominanceTransitiveShortcutAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteDominanceTransitiveShortcut"
      "ASPProof/SearchRouteDominanceTransitiveShortcut.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-dominance-transitive-shortcut-audit-v1.json"
    json.pretty

#writeDominanceTransitiveShortcutAudit
