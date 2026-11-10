-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

elab "writeSearchRouteAdmissionRetryCacheRejoinConcreteWeightedCutAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-concrete-weighted-cut-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinConcreteWeightedCutAudit
