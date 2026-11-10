-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.ActivationLifecycleAudit

namespace ASPProof.Audit.ActivationLifecycleAudit

open ASPProof.Audit.Core
open ASPProof.ActivationLifecycleAudit

def targets : List Target :=
  [ Target.mk
      ``operational_ready_implies_live_bound_matching_runtime
      "derived-ready-invariant"
      [ "ASP-RFC-10.05-ALAE-READY" ]
  , Target.mk
      ``cached_ready_without_host_is_not_operational
      "false-ready-counterexample"
      [ "ASP-RFC-10.05-ALAE-READY" ]
  , Target.mk
      ``missing_activation_without_emergency_capability_deadlocks_repair
      "repair-deadlock-counterexample"
      [ "ASP-RFC-10.05-ALAE-REPAIR" ]
  , Target.mk
      ``hidden_reachable_child_is_not_safe_to_replace
      "unsafe-replacement-counterexample"
      [ "ASP-RFC-10.05-ALAE-REPLACEMENT" ]
  , Target.mk
      ``idle_resume_can_rebind_by_ack_without_subagent_start
      "acknowledgement-rebind"
      [ "ASP-RFC-10.05-ALAE-REBIND" ]
  , Target.mk
      ``missing_dispatch_receipt_blocks_operational_ready
      "dispatch-ambiguity-counterexample"
      [ "ASP-RFC-10.05-ALAE-DISPATCH" ]
  , Target.mk
      ``binary_drift_blocks_operational_ready
      "runtime-identity-counterexample"
      [ "ASP-RFC-10.05-ALAE-IDENTITY" ]
  , Target.mk
      ``operational_ready_does_not_imply_fresh_trusted_ready
      "stale-observation-counterexample"
      [ "ASP-RFC-10.05-ALAE-FRESHNESS" ]
  , Target.mk
      ``applicable_evolution_advances_generation_and_matches_runtime
      "monotone-evolution"
      [ "ASP-RFC-10.05-ALAE-EVOLUTION" ]
  , Target.mk
      ``generation_regression_cannot_be_applied
      "generation-regression-counterexample"
      [ "ASP-RFC-10.05-ALAE-EVOLUTION" ]
  , Target.mk
      ``evolution_without_rollback_cannot_be_applied
      "missing-rollback-counterexample"
      [ "ASP-RFC-10.05-ALAE-EVOLUTION" ]
  , Target.mk
      ``evolution_without_compatibility_witness_cannot_be_applied
      "missing-compatibility-counterexample"
      [ "ASP-RFC-10.05-ALAE-EVOLUTION" ]
  , Target.mk
      ``incomplete_candidate_cannot_replace_published_generation
      "atomic-incomplete-candidate-rejection"
      [ "ASP-RFC-10.05-ALAE-ATOMIC-GENERATION" ]
  , Target.mk
      ``digest_mismatched_candidate_cannot_replace_published_generation
      "atomic-mixed-generation-rejection"
      [ "ASP-RFC-10.05-ALAE-ATOMIC-GENERATION" ]
  , Target.mk
      ``consistent_candidate_is_the_only_new_visible_generation
      "atomic-publication-linearization"
      [ "ASP-RFC-10.05-ALAE-ATOMIC-GENERATION" ]
  , Target.mk
      ``two_file_activation_then_receipt_publish_is_not_consistent
      "two-file-publication-counterexample"
      [ "ASP-RFC-10.05-ALAE-ATOMIC-GENERATION" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.ActivationLifecycleAudit"
    "ASPProof/ActivationLifecycleAudit.lean"
    targets

end ASPProof.Audit.ActivationLifecycleAudit
