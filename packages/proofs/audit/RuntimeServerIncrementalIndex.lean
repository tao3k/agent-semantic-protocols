-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.RuntimeServerIncrementalIndex

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.RuntimeServerIncrementalIndex.auditJson
  IO.FS.writeFile
    "receipts/runtime-server-incremental-index-v1.json"
    json.pretty
