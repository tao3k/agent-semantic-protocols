import ASPProof.Audit.SearchRouteTemporalCertificateValidity

def main : IO Unit :=
  IO.println <|
    Lean.Json.compress
      ASPProof.Audit.SearchRouteTemporalCertificateValidity.receipt
