import ASPProof.Audit.RuntimeServerSearchGeneration

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.RuntimeServerSearchGeneration.auditJson
  IO.FS.writeFile
    "receipts/runtime-server-search-generation-v1.json"
    json.pretty
