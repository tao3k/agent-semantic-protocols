-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.GlobalRuntimeServer

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.GlobalRuntimeServer.auditJson
  IO.FS.writeFile
    "receipts/global-runtime-server-audit-v1.json"
    json.pretty
