import ASPProof.Audit.AgentSessionRuntimeServerBoundary

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.AgentSessionRuntimeServerBoundary.auditJson
  IO.FS.writeFile
    "receipts/agent-session-runtime-server-boundary-v1.json"
    json.pretty
