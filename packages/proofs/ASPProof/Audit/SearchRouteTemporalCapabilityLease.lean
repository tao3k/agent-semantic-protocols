-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteTemporalCapabilityLease

namespace ASPProof.Audit.SearchRouteTemporalCapabilityLease

open Lean
open ASPProof.Audit.Core

def targets : List Target :=
  [
    { name := ``_root_.SearchRouteTemporalCapabilityLease.usable_lease_is_bound
      theoremFamily := "lease-binding"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-BOUND"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.usable_lease_is_fresh
      theoremFamily := "lease-freshness"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-FRESH"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.expired_lease_is_not_usable
      theoremFamily := "expiry-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-EXPIRY"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.future_lease_is_not_usable
      theoremFamily := "future-lease-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-FRESH"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.source_index_generation_drift_rejects_lease
      theoremFamily := "source-index-drift"
      rfcClauseIds := [
        "ASP-RFC-10.05-TCL-DRIFT",
        "ASP-RFC-10.05-TCL-GENERATION"
      ] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.runtime_generation_drift_rejects_lease
      theoremFamily := "runtime-drift"
      rfcClauseIds := [
        "ASP-RFC-10.05-TCL-DRIFT",
        "ASP-RFC-10.05-TCL-GENERATION"
      ] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.projection_version_drift_rejects_lease
      theoremFamily := "projection-drift"
      rfcClauseIds := [
        "ASP-RFC-10.05-TCL-DRIFT",
        "ASP-RFC-10.05-TCL-GENERATION"
      ] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.runtime_ready_does_not_imply_lease_usable
      theoremFamily := "ready-not-sufficient-counterexample"
      rfcClauseIds := [
        "ASP-RFC-10.05-TCL-EXPIRY",
        "ASP-RFC-10.05-TCL-STATIC-GAP"
      ] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.recovery_run_length_is_bounded
      theoremFamily := "bounded-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-RECOVERY"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.zero_budget_has_no_recovery_progress
      theoremFamily := "zero-budget-termination"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-RECOVERY"] },
    { name := ``_root_.SearchRouteTemporalCapabilityLease.recoverable_self_loop_has_no_progress
      theoremFamily := "self-loop-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-TCL-RECOVERY"] }
  ]

def auditJson : Elab.Term.TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteTemporalCapabilityLease"
    "packages/proofs/ASPProof/SearchRouteTemporalCapabilityLease.lean"
    targets

end ASPProof.Audit.SearchRouteTemporalCapabilityLease
