namespace ASPProof.ResidentGenerationSegmentAdmission

inductive SegmentAdmission where
  | current
  | recoveryRequired
  deriving DecidableEq, Repr

structure SegmentEnvelope where
  contractDigest : String
  payloadDigest : String
  deriving DecidableEq, Repr

def admit (currentContractDigest : String) (segment : SegmentEnvelope) : SegmentAdmission :=
  if segment.contractDigest = currentContractDigest then
    .current
  else
    .recoveryRequired

def payloadDecodeAuthorized
    (currentContractDigest : String)
    (segment : SegmentEnvelope) : Bool :=
  admit currentContractDigest segment = .current

theorem contract_drift_requires_recovery
    (currentContractDigest : String)
    (segment : SegmentEnvelope)
    (drift : segment.contractDigest ≠ currentContractDigest) :
    admit currentContractDigest segment = .recoveryRequired := by
  simp [admit, drift]

theorem contract_drift_cannot_decode_payload
    (currentContractDigest : String)
    (segment : SegmentEnvelope)
    (drift : segment.contractDigest ≠ currentContractDigest) :
    payloadDecodeAuthorized currentContractDigest segment = false := by
  simp [payloadDecodeAuthorized, admit, drift]

end ASPProof.ResidentGenerationSegmentAdmission
