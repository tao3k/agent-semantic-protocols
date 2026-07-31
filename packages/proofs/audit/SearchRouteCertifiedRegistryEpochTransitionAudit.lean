import ASPProof.Audit.SearchRouteCertifiedRegistryEpochTransition

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteCertifiedRegistryEpochTransition.receipt
