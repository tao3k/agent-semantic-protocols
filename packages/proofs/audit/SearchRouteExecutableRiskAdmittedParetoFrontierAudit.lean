import ASPProof.Audit.SearchRouteExecutableRiskAdmittedParetoFrontier

def main (args : List String) : IO Unit := do
  let payload :=
    ASPProof.Audit.SearchRouteExecutableRiskAdmittedParetoFrontier.manifest.compress ++ "\n"
  match args with
  | [] => IO.print payload
  | ["--output", path] => IO.FS.writeFile path payload
  | _ => throw <| IO.userError "usage: SearchRouteExecutableRiskAdmittedParetoFrontierAudit [--output PATH]"
