import ASPProof.Audit.HookSessionMaterialization

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.HookSessionMaterialization.auditJson
  IO.FS.writeFile
    "receipts/hook-session-materialization-v1.json"
    json.pretty
