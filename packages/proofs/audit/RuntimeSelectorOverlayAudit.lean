-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.RuntimeSelectorOverlay

open Lean Elab Command

elab "#writeRuntimeSelectorOverlayAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/runtime-selector-overlay-audit-v1.json"
    ASPProof.Audit.RuntimeSelectorOverlay.auditJson

#writeRuntimeSelectorOverlayAudit
