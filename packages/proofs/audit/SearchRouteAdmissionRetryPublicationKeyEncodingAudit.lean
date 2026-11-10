-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionRetryPublicationKeyEncoding

open Lean Elab Command

elab "writeSearchRouteAdmissionRetryPublicationKeyEncodingAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-publication-key-encoding-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryPublicationKeyEncoding.auditJson

writeSearchRouteAdmissionRetryPublicationKeyEncodingAudit
