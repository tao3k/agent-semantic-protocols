import ASPProof.Audit.SearchRouteAppendOnlyForecastCalibration

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteAppendOnlyForecastCalibration.receipt
