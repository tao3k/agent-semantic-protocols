-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionLedger

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-ledger-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteAdmissionLedger"
      "packages/proofs/ASPProof/SearchRouteAdmissionLedger.lean"
      ASPProof.Audit.SearchRouteAdmissionLedger.targets)
