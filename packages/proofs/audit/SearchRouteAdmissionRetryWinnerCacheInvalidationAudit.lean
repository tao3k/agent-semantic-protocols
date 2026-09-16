-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryWinnerCacheInvalidationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-winner-cache-invalidation-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryWinnerCacheInvalidation.auditJson

writeSearchRouteAdmissionRetryWinnerCacheInvalidationAudit
