-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinFenceAuthorityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-fence-authority-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFenceAuthority.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinFenceAuthorityAudit
