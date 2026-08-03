import ASPProof.AgentSessionLifecycleExecutableRefinement

namespace ASPProof.Audit.AgentSessionLifecycleExecutableRefinement

open ASPProof.AgentSessionLifecycleProduct
open ASPProof.AgentSessionLifecycleExecutableRefinement

example
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (lateReceipt : state.binding.generation ≠ state.session.generation) :
    ¬ executableReplacementAdmitted state nextGeneration := by
  exact stale_binding_generation_rejects_replacement lateReceipt

example
    (state : LifecycleProduct)
    (nextGeneration : Nat)
    (admitted : executableReplacementAdmitted state nextGeneration) :
    replacementAdmitted state nextGeneration ∧
      state.binding.generation = state.session.generation := by
  exact admitted

example : SessionPhase.unobserved ≠ SessionPhase.declared := by
  decide

example : ServerHealth.unobserved ≠ ServerHealth.unavailable := by
  decide

end ASPProof.Audit.AgentSessionLifecycleExecutableRefinement
