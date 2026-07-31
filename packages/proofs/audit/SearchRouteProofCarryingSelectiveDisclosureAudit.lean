import ASPProof.Audit.SearchRouteProofCarryingSelectiveDisclosure

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteProofCarryingSelectiveDisclosure.receipt
