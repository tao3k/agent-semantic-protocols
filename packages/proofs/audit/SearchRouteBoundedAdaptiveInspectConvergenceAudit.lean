import ASPProof.Audit.SearchRouteBoundedAdaptiveInspectConvergence

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteBoundedAdaptiveInspectConvergence.receipt
