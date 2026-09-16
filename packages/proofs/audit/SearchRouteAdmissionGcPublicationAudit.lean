-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionGcPublication

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-gc-publication-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteAdmissionGcPublication"
      "packages/proofs/ASPProof/SearchRouteAdmissionGcPublication.lean"
      ASPProof.Audit.SearchRouteAdmissionGcPublication.targets)
