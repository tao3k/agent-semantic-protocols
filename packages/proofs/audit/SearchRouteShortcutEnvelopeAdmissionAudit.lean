-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteShortcutEnvelopeAdmission

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.admission_gate_true_implies_facts
    theoremFamily := "admission-gate-soundness"
    rfcClauseIds := ["SEA-GATE"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.admission_facts_imply_gate_true
    theoremFamily := "admission-gate-completeness"
    rfcClauseIds := ["SEA-GATE"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.VerifiedCachedShortcut.toCachedShortcut
    theoremFamily := "verified-to-replay-cache-derivation"
    rfcClauseIds := ["SEA-COHERENCE"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.verifyRawShortcut
    theoremFamily := "raw-envelope-witnessed-verification"
    rfcClauseIds := ["SEA-ADMISSION"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.raw_without_witness_falls_back
    theoremFamily := "raw-envelope-without-witness-fallback"
    rfcClauseIds := ["SEA-FALLBACK"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.witnessed_raw_becomes_verified
    theoremFamily := "witnessed-envelope-verification"
    rfcClauseIds := ["SEA-ADMISSION"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.admitted_shortcut_is_accepted_under_policy
    theoremFamily := "admitted-shortcut-policy-acceptance"
    rfcClauseIds := ["SEA-REPLAY"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.accepted_verified_replay_preserves_removal_soundness
    theoremFamily := "verified-replay-removal-soundness"
    rfcClauseIds := ["SEA-SOUNDNESS"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.changed_context_rejects_verified_shortcut
    theoremFamily := "verified-shortcut-context-change-rejection"
    rfcClauseIds := ["SEA-REPLAY"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.schema_mismatch_blocks_admission_witness
    theoremFamily := "schema-mismatch-blocks-admission"
    rfcClauseIds := ["SEA-SCHEMA"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.hash_algorithm_mismatch_blocks_admission_witness
    theoremFamily := "hash-algorithm-mismatch-blocks-admission"
    rfcClauseIds := ["SEA-HASH"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.canonical_digest_mismatch_blocks_admission_witness
    theoremFamily := "canonical-digest-mismatch-blocks-admission"
    rfcClauseIds := ["SEA-CANONICAL"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.commitment_mismatch_blocks_admission_witness
    theoremFamily := "commitment-mismatch-blocks-admission"
    rfcClauseIds := ["SEA-COMMITMENT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.false_dominance_claim_blocks_admission_witness
    theoremFamily := "false-dominance-claim-blocks-admission"
    rfcClauseIds := ["SEA-PROOF"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.verified_replay_projection_is_constant_shape
    theoremFamily := "verified-replay-projection-constant-shape"
    rfcClauseIds := ["SEA-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteShortcutEnvelopeAdmission.verified_replay_beats_nonempty_fallback_projection
    theoremFamily := "verified-replay-beats-nonempty-fallback"
    rfcClauseIds := ["SEA-RECEIPT"]
  }
]

elab "#writeShortcutEnvelopeAdmissionAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteShortcutEnvelopeAdmission"
      "ASPProof/SearchRouteShortcutEnvelopeAdmission.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-shortcut-envelope-admission-audit-v1.json"
    json.pretty

#writeShortcutEnvelopeAdmissionAudit
