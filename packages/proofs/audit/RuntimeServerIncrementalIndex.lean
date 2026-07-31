import ASPProof.Audit.RuntimeServerIncrementalIndex

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.RuntimeServerIncrementalIndex.auditJson
  IO.FS.writeFile
    "receipts/runtime-server-incremental-index-v1.json"
    json.pretty
