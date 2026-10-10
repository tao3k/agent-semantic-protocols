-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteShortcutCommitmentReplay

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.natEq_true_implies_eq
    theoremFamily := "executable-commitment-word-equality-soundness"
    rfcClauseIds := ["SCR-EQUALITY"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.natEq_is_reflexive
    theoremFamily := "executable-commitment-word-equality-reflexivity"
    rfcClauseIds := ["SCR-EQUALITY"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.commitment_match_true_implies_equal
    theoremFamily := "nine-field-commitment-match-soundness"
    rfcClauseIds := ["SCR-COMMITMENT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.equal_commitments_match
    theoremFamily := "nine-field-commitment-match-completeness"
    rfcClauseIds := ["SCR-COMMITMENT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.commitment_mismatch_fails_closed
    theoremFamily := "commitment-mismatch-fails-closed"
    rfcClauseIds := ["SCR-FAIL-CLOSED"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.cacheTransitiveShortcut
    theoremFamily := "proof-carrying-shortcut-cache-construction"
    rfcClauseIds := ["SCR-CACHE"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.freshly_cached_shortcut_is_accepted
    theoremFamily := "fresh-shortcut-cache-acceptance"
    rfcClauseIds := ["SCR-ACCEPT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.accepted_replay_has_exact_commitment
    theoremFamily := "accepted-replay-exact-commitment"
    rfcClauseIds := ["SCR-ACCEPT", "SCR-COMMITMENT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.mismatched_replay_is_rejected
    theoremFamily := "mismatched-shortcut-replay-rejection"
    rfcClauseIds := ["SCR-FAIL-CLOSED"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.accepted_replay_preserves_removal_soundness
    theoremFamily := "accepted-replay-removal-soundness"
    rfcClauseIds := ["SCR-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.universe_digest_mismatch_is_rejected
    theoremFamily := "universe-digest-mismatch-rejection"
    rfcClauseIds := ["SCR-UNIVERSE"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.cost_semantics_mismatch_is_rejected
    theoremFamily := "cost-semantics-mismatch-rejection"
    rfcClauseIds := ["SCR-SEMANTICS"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.selection_policy_mismatch_is_rejected
    theoremFamily := "selection-policy-mismatch-rejection"
    rfcClauseIds := ["SCR-SELECTION"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.audit_receipt_mismatch_is_rejected
    theoremFamily := "audit-receipt-mismatch-rejection"
    rfcClauseIds := ["SCR-AUDIT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.cached_replay_receipt_is_constant_shape
    theoremFamily := "cached-shortcut-replay-constant-shape"
    rfcClauseIds := ["SCR-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutCommitmentReplay.cached_replay_beats_nonempty_explicit_replay
    theoremFamily := "cached-replay-beats-explicit-nonempty-replay"
    rfcClauseIds := ["SCR-RECEIPT"]
  }
]

elab "#writeShortcutCommitmentReplayAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteShortcutCommitmentReplay"
      "ASPProof/SearchRouteShortcutCommitmentReplay.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-shortcut-commitment-replay-audit-v1.json"
    json.pretty

#writeShortcutCommitmentReplayAudit
