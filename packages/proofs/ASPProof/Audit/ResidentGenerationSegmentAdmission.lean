-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ResidentGenerationSegmentAdmission

namespace ASPProof.Audit.ResidentGenerationSegmentAdmission

open ASPProof.ResidentGenerationSegmentAdmission

theorem stale_segment_has_no_default_compatibility
    (currentContractDigest : String)
    (segment : SegmentEnvelope)
    (drift : segment.contractDigest ≠ currentContractDigest) :
    admit currentContractDigest segment = .recoveryRequired ∧
      payloadDecodeAuthorized currentContractDigest segment = false := by
  exact ⟨contract_drift_requires_recovery currentContractDigest segment drift,
    contract_drift_cannot_decode_payload currentContractDigest segment drift⟩

end ASPProof.Audit.ResidentGenerationSegmentAdmission
