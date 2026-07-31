import ASPProof.Audit.SearchRouteAuthorityScopedLogicalTime

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteAuthorityScopedLogicalTime.receipt
