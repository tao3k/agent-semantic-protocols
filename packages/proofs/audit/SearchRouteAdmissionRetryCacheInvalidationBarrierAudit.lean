-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryCacheInvalidationBarrier

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryCacheInvalidationBarrierAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-invalidation-barrier-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheInvalidationBarrier.auditJson

writeSearchRouteAdmissionRetryCacheInvalidationBarrierAudit
