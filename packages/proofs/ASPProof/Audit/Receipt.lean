-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import Lean.Elab.Command

namespace ASPProof.Audit

open Lean Elab Command

def writeReceipt
    (path : System.FilePath)
    (auditJson : Elab.Term.TermElabM Json) :
    CommandElabM Unit := do
  let audit ← liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

end ASPProof.Audit
