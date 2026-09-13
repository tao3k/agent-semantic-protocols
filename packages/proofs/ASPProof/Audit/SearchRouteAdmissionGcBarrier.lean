-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionGcBarrier

namespace ASPProof.Audit.SearchRouteAdmissionGcBarrier

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteAdmissionGcBarrier.barrier_safe_sweep_preserves_concurrent_commit_receipts
    "barrier-safe-sweep"
    ["ASP-RFC-10.05-AGC-SAFE-SWEEP"],
  Target.mk
    `SearchRouteAdmissionGcBarrier.snapshot_only_gc_can_delete_concurrently_referenced_receipts
    "snapshot-only-counterexample"
    ["ASP-RFC-10.05-AGC-EPOCH", "ASP-RFC-10.05-AGC-NONIMPLICATION"],
  Target.mk
    `SearchRouteAdmissionGcBarrier.publication_during_sweep_requires_both_barrier_entries
    "publication-barrier"
    ["ASP-RFC-10.05-AGC-BARRIER"],
  Target.mk
    `SearchRouteAdmissionGcBarrier.remembered_set_monotonicity_preserves_installed_barrier
    "remembered-set-monotonicity"
    ["ASP-RFC-10.05-AGC-MONOTONE"]
]

end ASPProof.Audit.SearchRouteAdmissionGcBarrier
