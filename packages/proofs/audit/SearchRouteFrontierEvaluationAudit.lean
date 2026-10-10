-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteFrontierEvaluation

open Lean Elab Command

run_cmd
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-frontier-evaluation-audit-v1.json"
    (ASPProof.Audit.Core.proofAuditJson
      "ASPProof.SearchRouteFrontierEvaluation"
      "packages/proofs/ASPProof/SearchRouteFrontierEvaluation.lean"
      ASPProof.Audit.SearchRouteFrontierEvaluation.targets)
