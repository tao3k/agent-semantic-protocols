import ASPProof.Audit.SearchRouteForecastBoundParetoBatchScheduling

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteForecastBoundParetoBatchScheduling.receipt
