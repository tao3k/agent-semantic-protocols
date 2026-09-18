-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

elab "writeSearchRouteAdmissionRetryCacheRejoinJointDecisionFusionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-decision-fusion-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinJointDecisionFusionAudit
