import ASPProof.Audit.GlobalRuntimeServer

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.GlobalRuntimeServer.auditJson
  IO.FS.writeFile
    "receipts/global-runtime-server-audit-v1.json"
    json.pretty
