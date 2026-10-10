-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionLedgerOrder

open Lean Elab Command

elab "writeSearchRouteAdmissionLedgerOrderAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-ledger-order-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionLedgerOrder.auditJson

writeSearchRouteAdmissionLedgerOrderAudit
