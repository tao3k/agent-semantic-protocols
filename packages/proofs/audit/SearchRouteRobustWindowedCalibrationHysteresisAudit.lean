import ASPProof.Audit.SearchRouteRobustWindowedCalibrationHysteresis

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteRobustWindowedCalibrationHysteresis.receipt
