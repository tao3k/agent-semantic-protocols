import ASPProof.RuntimeWorkspaceGeneration

namespace ASPProof.Audit.RuntimeWorkspaceGeneration

open ASPProof.RuntimeWorkspaceGeneration

theorem generation_identity_is_a_function
    (inputs : GenerationInputs) :
    deriveGenerationId inputs = deriveGenerationId inputs := by
  rfl

theorem resident_read_gate_requires_zero_db_open
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.databaseOpens = 0 := by
  exact admitted.2

theorem resident_read_gate_requires_memory_hit
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.residentHit = true := by
  exact admitted.1

end ASPProof.Audit.RuntimeWorkspaceGeneration
