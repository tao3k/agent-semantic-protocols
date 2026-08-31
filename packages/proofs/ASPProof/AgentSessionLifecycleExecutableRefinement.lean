import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.AgentSessionLifecycleExecutableRefinement

open AgentSessionLifecycleProduct

def executableFollowupAdmitted (state : LifecycleProduct) : Prop :=
  state.binding.pathObservation = .present ∧
    state.binding.phase = .fresh ∧
    state.binding.generation = state.session.generation

theorem executable_followup_refines_product
    {state : LifecycleProduct}
    (admitted : executableFollowupAdmitted state) :
    followupTaskAdmitted state = true := by
  simp [followupTaskAdmitted, admitted.1, admitted.2.1, admitted.2.2]

theorem stale_binding_generation_rejects_followup
    {state : LifecycleProduct}
    (stale : state.binding.generation ≠ state.session.generation) :
    ¬ executableFollowupAdmitted state := by
  intro admitted
  exact stale admitted.2.2

theorem absent_path_rejects_executable_followup
    {state : LifecycleProduct}
    (absent : state.binding.pathObservation = .absent) :
    ¬ executableFollowupAdmitted state := by
  rintro ⟨present, _fresh, _generationAligned⟩
  rw [absent] at present
  cases present

end ASPProof.AgentSessionLifecycleExecutableRefinement
