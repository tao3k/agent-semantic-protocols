import ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation

def main (args : List String) : IO Unit := do
  let payload :=
    ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation.manifest.compress ++ "\n"
  match args with
  | [] => IO.print payload
  | ["--output", path] => IO.FS.writeFile path payload
  | _ => throw <| IO.userError "usage: SearchRouteAdaptiveBatchFailureIsolationAudit [--output PATH]"
