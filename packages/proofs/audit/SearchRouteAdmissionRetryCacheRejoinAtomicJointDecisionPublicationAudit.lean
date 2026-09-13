-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

elab "writeSearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublicationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-atomic-joint-decision-publication-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublicationAudit
