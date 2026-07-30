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
