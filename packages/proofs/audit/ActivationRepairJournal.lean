import ASPProof.Audit.ActivationRepairJournal

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.ActivationRepairJournal.auditJson
  IO.FS.writeFile
    "receipts/activation-repair-journal-v1.json"
    json.pretty
