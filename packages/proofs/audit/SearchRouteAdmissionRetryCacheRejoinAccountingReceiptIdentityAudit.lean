-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

elab "writeSearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-accounting-receipt-identity-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentityAudit
