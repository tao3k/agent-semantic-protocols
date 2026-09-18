-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionLedger

namespace ASPProof.Audit.SearchRouteAdmissionLedger

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteAdmissionLedger.atomic_commit_marks_run_and_both_receipts
    "atomic-commit-effect"
    ["ASP-RFC-10.05-EAL-ATOMIC-COMMIT"],
  Target.mk
    `SearchRouteAdmissionLedger.committed_run_rejects_exact_retry
    "exactly-once-retry-rejection"
    ["ASP-RFC-10.05-EAL-EXACTLY-ONCE"],
  Target.mk
    `SearchRouteAdmissionLedger.crash_recovery_observes_pre_or_post_state
    "crash-linearizability"
    ["ASP-RFC-10.05-EAL-ATOMIC-COMMIT", "ASP-RFC-10.05-EAL-CRASH-RECOVERY"],
  Target.mk
    `SearchRouteAdmissionLedger.partial_ledger_is_not_a_crash_consistent_state
    "partial-state-counterexample"
    ["ASP-RFC-10.05-EAL-CRASH-RECOVERY", "ASP-RFC-10.05-EAL-NONIMPLICATION"],
  Target.mk
    `SearchRouteAdmissionLedger.atomic_relation_alone_does_not_prove_durable_recovery
    "durability-witness-counterexample"
    ["ASP-RFC-10.05-EAL-CRASH-RECOVERY", "ASP-RFC-10.05-EAL-NONIMPLICATION"],
  Target.mk
    `SearchRouteAdmissionLedger.read_check_then_partial_write_is_not_atomic_admission
    "read-check-counterexample"
    ["ASP-RFC-10.05-EAL-NONIMPLICATION", "ASP-RFC-10.05-EAL-PRECONDITION"]
]

end ASPProof.Audit.SearchRouteAdmissionLedger
