-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

elab "writeSearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransitionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-weight-schedule-transition-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransitionAudit
