-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionGcPublication

namespace ASPProof.Audit.SearchRouteAdmissionGcPublication

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteAdmissionGcPublication.valid_barrier_implies_well_formed_cycle_window
    "cycle-window"
    ["ASP-RFC-10.05-AGP-CYCLE"],
  Target.mk
    `SearchRouteAdmissionGcPublication.publication_certificate_orders_both_barriers_before_visibility
    "barrier-publication-order"
    ["ASP-RFC-10.05-AGP-ORDER", "ASP-RFC-10.05-AGP-TOKEN"],
  Target.mk
    `SearchRouteAdmissionGcPublication.publication_certificate_implies_concurrent_commit_certificate
    "gc-certificate-bridge"
    ["ASP-RFC-10.05-AGP-BRIDGE"],
  Target.mk
    `SearchRouteAdmissionGcPublication.barrier_token_from_another_cycle_is_invalid
    "cross-cycle-token-counterexample"
    ["ASP-RFC-10.05-AGP-CYCLE", "ASP-RFC-10.05-AGP-NONIMPLICATION"],
  Target.mk
    `SearchRouteAdmissionGcPublication.barrier_installed_after_publication_is_invalid
    "late-barrier-counterexample"
    ["ASP-RFC-10.05-AGP-NONIMPLICATION", "ASP-RFC-10.05-AGP-ORDER"]
]

end ASPProof.Audit.SearchRouteAdmissionGcPublication
