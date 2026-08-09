namespace ASPProof.HookMutationEnvelopeRefinement

inductive SemanticToolAction where
  | applyPatch (changedOwners : List String)
  | unrelated
  deriving DecidableEq

inductive HostEnvelope where
  | direct (action : SemanticToolAction)
  | functionsExec (nested : SemanticToolAction)
  deriving DecidableEq

def normalize : HostEnvelope → SemanticToolAction
  | .direct action | .functionsExec action => action

def changedOwners : SemanticToolAction → List String
  | .applyPatch owners => owners
  | .unrelated => []

theorem functions_exec_refines_direct_apply_patch (owners : List String) :
    changedOwners (normalize (.functionsExec (.applyPatch owners))) =
      changedOwners (normalize (.direct (.applyPatch owners))) := by
  rfl

theorem unrelated_envelope_has_no_generation_effect :
    changedOwners (normalize (.functionsExec .unrelated)) = [] := by
  rfl

end ASPProof.HookMutationEnvelopeRefinement
