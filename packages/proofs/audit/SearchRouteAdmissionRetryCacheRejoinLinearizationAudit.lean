-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinLinearizationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-linearization-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinLinearizationAudit
