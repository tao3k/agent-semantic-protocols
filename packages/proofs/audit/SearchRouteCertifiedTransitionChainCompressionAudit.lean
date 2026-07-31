import ASPProof.Audit.SearchRouteCertifiedTransitionChainCompression

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteCertifiedTransitionChainCompression.receipt
