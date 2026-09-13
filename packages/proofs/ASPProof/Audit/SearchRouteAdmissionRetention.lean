-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetention

namespace ASPProof.Audit.SearchRouteAdmissionRetention

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteAdmissionRetention.run_cannot_retire_before_retention_deadline
    "retention-horizon"
    ["ASP-RFC-10.05-ARR-HORIZON"],
  Target.mk
    `SearchRouteAdmissionRetention.retained_committed_run_has_recoverable_acknowledgement
    "retained-ack-recovery"
    ["ASP-RFC-10.05-ARR-RESOLUTION", "ASP-RFC-10.05-ARR-RETENTION"],
  Target.mk
    `SearchRouteAdmissionRetention.safe_gc_preserves_record_resolvability
    "safe-gc-record-preservation"
    ["ASP-RFC-10.05-ARR-SAFE-GC"],
  Target.mk
    `SearchRouteAdmissionRetention.safe_gc_preserves_retention_invariant
    "safe-gc-invariant-preservation"
    ["ASP-RFC-10.05-ARR-RETENTION", "ASP-RFC-10.05-ARR-SAFE-GC"],
  Target.mk
    `SearchRouteAdmissionRetention.committed_record_does_not_imply_payload_reachability
    "missing-payload-counterexample"
    ["ASP-RFC-10.05-ARR-NONIMPLICATION", "ASP-RFC-10.05-ARR-RESOLUTION"],
  Target.mk
    `SearchRouteAdmissionRetention.deleting_referenced_payload_breaks_acknowledgement_recovery
    "unsafe-gc-counterexample"
    ["ASP-RFC-10.05-ARR-NONIMPLICATION", "ASP-RFC-10.05-ARR-SAFE-GC"]
]

end ASPProof.Audit.SearchRouteAdmissionRetention
