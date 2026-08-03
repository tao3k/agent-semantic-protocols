import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.AgentSessionLifecycleExecutableRefinement

open AgentSessionLifecycleProduct

def replacementGenerationAligned (state : LifecycleProduct) : Prop :=
  state.binding.generation = state.session.generation

def executableReplacementAdmitted
    (state : LifecycleProduct)
    (nextGeneration : Nat) : Prop :=
  replacementAdmitted state nextGeneration ∧ replacementGenerationAligned state

theorem executable_replacement_refines_product
    {state : LifecycleProduct}
    {nextGeneration : Nat}
    (admitted : executableReplacementAdmitted state nextGeneration) :
    replacementAdmitted state nextGeneration :=
  admitted.1

theorem stale_binding_generation_rejects_replacement
    {state : LifecycleProduct}
    {nextGeneration : Nat}
    (stale : state.binding.generation ≠ state.session.generation) :
    ¬ executableReplacementAdmitted state nextGeneration := by
  intro admitted
  exact stale admitted.2

theorem executable_replacement_carries_generation_alignment
    {state : LifecycleProduct}
    {nextGeneration : Nat}
    (admitted : executableReplacementAdmitted state nextGeneration) :
    state.binding.generation = state.session.generation :=
  admitted.2

end ASPProof.AgentSessionLifecycleExecutableRefinement
