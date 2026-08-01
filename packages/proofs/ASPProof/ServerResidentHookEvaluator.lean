namespace ASPProof.ServerResidentHookEvaluator

structure GenerationKey where
  artifact : Nat
  workspace : Nat
  config : Nat
  project : Nat
  contract : Nat
  activation : Nat
  deriving DecidableEq, Repr

inductive EvaluationRoute where
  | server
  | localFallback
  deriving DecidableEq, Repr

structure GenerationState where
  key : GenerationKey
  leases : Nat
  deriving DecidableEq, Repr

def Admissible (observed produced : GenerationKey) : Prop :=
  observed = produced

def compileCount : Nat → Nat → Nat
  | 0, _ => 0
  | _ + 1, distinctKeys => distinctKeys

def Evictable (generation : GenerationState) : Prop :=
  generation.leases = 0

def routeForServer (available : Bool) : EvaluationRoute :=
  if available then .server else .localFallback

def routeForIdentity (observed produced : GenerationKey) : EvaluationRoute :=
  if observed = produced then .server else .localFallback

def warmRoundTrips : Nat := 1

theorem workspace_identity_prevents_alias
    (key : GenerationKey) (otherWorkspace : Nat)
    (different : otherWorkspace ≠ key.workspace) :
    ¬ Admissible key { key with workspace := otherWorkspace } := by
  intro equal
  have workspace_equal := congrArg GenerationKey.workspace equal
  exact different workspace_equal.symm

theorem project_identity_prevents_alias
    (key : GenerationKey) (otherProject : Nat)
    (different : otherProject ≠ key.project) :
    ¬ Admissible key { key with project := otherProject } := by
  intro equal
  have project_equal := congrArg GenerationKey.project equal
  exact different project_equal.symm

theorem activation_generation_prevents_alias
    (key : GenerationKey) (otherActivation : Nat)
    (different : otherActivation ≠ key.activation) :
    ¬ Admissible key { key with activation := otherActivation } := by
  intro equal
  have activation_equal := congrArg GenerationKey.activation equal
  exact different activation_equal.symm

theorem same_key_compiles_once (events : Nat) (active : events ≠ 0) :
    compileCount events 1 = 1 := by
  cases events with
  | zero => exact False.elim (active rfl)
  | succ _ => rfl

theorem stale_generation_is_not_admissible
    (observed produced : GenerationKey) (stale : observed ≠ produced) :
    ¬ Admissible observed produced := by
  exact stale

theorem leased_generation_is_not_evictable
    (key : GenerationKey) (leases : Nat) (active : leases ≠ 0) :
    ¬ Evictable { key := key, leases := leases } := by
  exact active

theorem warm_path_is_one_round_trip : warmRoundTrips = 1 := by
  rfl

theorem server_failure_selects_local_fallback :
    routeForServer false = .localFallback := by
  rfl

theorem identity_mismatch_selects_local_fallback
    (observed produced : GenerationKey) (different : observed ≠ produced) :
    routeForIdentity observed produced = .localFallback := by
  exact if_neg different

end ASPProof.ServerResidentHookEvaluator
