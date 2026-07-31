import ASPProof.Audit.SearchRouteVerifierArtifactProvenance

def main : IO Unit :=
  IO.println
    ASPProof.Audit.SearchRouteVerifierArtifactProvenance.manifest.compress
