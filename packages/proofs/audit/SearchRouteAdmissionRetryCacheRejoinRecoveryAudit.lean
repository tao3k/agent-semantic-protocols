-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryCacheRejoinRecoveryAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-recovery-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinRecovery.auditJson

writeSearchRouteAdmissionRetryCacheRejoinRecoveryAudit
