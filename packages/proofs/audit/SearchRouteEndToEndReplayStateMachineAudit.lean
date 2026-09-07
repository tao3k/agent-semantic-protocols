-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteEndToEndReplayStateMachine
import ASPProof.Audit.Core

open ASPProof.Audit.Core
open Lean Elab Command

private def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.canonicalPayloadOfRaw
    theoremFamily := "raw-envelope-canonical-payload-projection"
    rfcClauseIds := ["ERS-CANONICAL"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.AcceptedReplay.payloadEqual
    theoremFamily := "accepted-replay-payload-identity"
    rfcClauseIds := ["ERS-NEGOTIATION", "ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.transition_increments_stage_rank
    theoremFamily := "transition-increments-stage-rank"
    rfcClauseIds := ["ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.add_one_then_length
    theoremFamily := "constructive-path-rank-arithmetic"
    rfcClauseIds := ["ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.path_stage_rank_accounting
    theoremFamily := "path-stage-rank-accounting"
    rfcClauseIds := ["ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.raw_to_accepted_path_has_exact_length
    theoremFamily := "raw-to-accepted-exact-transition-count"
    rfcClauseIds := ["ERS-NO-BYPASS", "ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.raw_to_fallback_path_has_exact_length
    theoremFamily := "raw-to-fallback-exact-transition-count"
    rfcClauseIds := ["ERS-FALLBACK", "ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.canonicalAcceptedPath
    theoremFamily := "canonical-three-transition-accepted-path"
    rfcClauseIds := ["ERS-ADMISSION", "ERS-NEGOTIATION", "ERS-PATH"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.no_direct_raw_to_accepted_transition
    theoremFamily := "no-direct-raw-to-accepted-transition"
    rfcClauseIds := ["ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.no_direct_admitted_to_accepted_transition
    theoremFamily := "no-direct-admitted-to-accepted-transition"
    rfcClauseIds := ["ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.acceptedReplayAdmissionWitness
    theoremFamily := "accepted-replay-admission-witness"
    rfcClauseIds := ["ERS-ADMISSION", "ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.accepted_replay_contains_negotiated_identity
    theoremFamily := "accepted-replay-negotiated-identity"
    rfcClauseIds := ["ERS-NEGOTIATION", "ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.acceptedReplayGateClosure
    theoremFamily := "accepted-replay-combined-gate-closure"
    rfcClauseIds := ["ERS-ADMISSION", "ERS-NEGOTIATION", "ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.fallback_is_not_accepted
    theoremFamily := "fallback-is-not-accepted"
    rfcClauseIds := ["ERS-FALLBACK"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.componentwise_cost_reduction_is_strict_improvement
    theoremFamily := "componentwise-core-cost-improvement"
    rfcClauseIds := ["ERS-COST"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.illustrative_digest_shortcut_strictly_improves_core
    theoremFamily := "illustrative-digest-shortcut-core-improvement"
    rfcClauseIds := ["ERS-COST"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.accepted_identity_is_independent_of_cache_credits
    theoremFamily := "accepted-identity-independent-of-cache-credits"
    rfcClauseIds := ["ERS-CACHE", "ERS-NO-BYPASS"]
  },
  {
    name := ``ASPProof.SearchRouteEndToEndReplayStateMachine.cache_credits_do_not_change_core_cost
    theoremFamily := "cache-credits-do-not-change-core-cost"
    rfcClauseIds := ["ERS-CACHE", "ERS-COST"]
  }
]

elab "emitSearchRouteEndToEndReplayStateMachineAudit" : command => do
  let receipt ← liftTermElabM <| proofAuditJson
    "ASPProof.SearchRouteEndToEndReplayStateMachine"
    "ASPProof/SearchRouteEndToEndReplayStateMachine.lean"
    targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-end-to-end-replay-state-machine-audit-v1.json"
    receipt.pretty
  logInfo receipt.compress

emitSearchRouteEndToEndReplayStateMachineAudit
