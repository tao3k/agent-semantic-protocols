namespace ASPProof.MultiAgentHostIdentityBinding

inductive AgentSandboxMode where
  | readOnly
  | workspaceWrite
  | dangerFullAccess
  deriving DecidableEq, Repr

inductive BindingLifecycle where
  | live
  | completed
  | stopped
  | unregistered
  | archived
  deriving DecidableEq, Repr

inductive ConfiguredResidentId where
  | aspExplorer
  | aspTesting
  deriving DecidableEq, Repr

structure HostTaskIdentity where
  rootSessionId : Nat
  childSessionId : Nat
  taskNameDigest : Nat
  deriving DecidableEq, Repr

structure AspRegistrationIdentity where
  instanceId : Nat
  registrationId : Nat
  deriving DecidableEq, Repr

structure ExactAgentBinding where
  host : HostTaskIdentity
  asp : AspRegistrationIdentity
  residentId : ConfiguredResidentId
  routeDigest : Nat
  profileDigest : Nat
  modelDigest : Nat
  sandboxMode : AgentSandboxMode
  generation : Nat
  deriving DecidableEq, Repr

structure ResidentGenerationFact where
  binding : ExactAgentBinding
  lifecycle : BindingLifecycle
  routable : Bool
  deriving DecidableEq, Repr

def registeredBinding
    (expected : ExactAgentBinding)
    (resident : ResidentGenerationFact) : Prop :=
  resident.binding = expected ∧
    resident.lifecycle = .live ∧
    resident.routable = true

instance (expected : ExactAgentBinding) (resident : ResidentGenerationFact) :
    Decidable (registeredBinding expected resident) := by
  unfold registeredBinding
  infer_instance

def registrationMatches
    (expected : ExactAgentBinding)
    (resident : ResidentGenerationFact) : Bool :=
  decide (registeredBinding expected resident)

theorem registered_implies_exact_live_binding
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (registered : registeredBinding expected resident) :
    resident.binding = expected ∧
      resident.lifecycle = .live ∧
      resident.routable = true :=
  registered

theorem binding_mismatch_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding ≠ expected) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch registered.1

theorem same_name_spoof_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (_sameName : resident.binding.host.taskNameDigest = expected.host.taskNameDigest)
    (differentHost :
      resident.binding.host.rootSessionId ≠ expected.host.rootSessionId ∨
      resident.binding.host.childSessionId ≠ expected.host.childSessionId) :
    ¬ registeredBinding expected resident := by
  intro registered
  rcases differentHost with differentRoot | differentChild
  · exact differentRoot (congrArg (fun binding => binding.host.rootSessionId) registered.1)
  · exact differentChild (congrArg (fun binding => binding.host.childSessionId) registered.1)

theorem wrong_child_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.host.childSessionId ≠ expected.host.childSessionId) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg (fun binding => binding.host.childSessionId) registered.1)

theorem wrong_root_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.host.rootSessionId ≠ expected.host.rootSessionId) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg (fun binding => binding.host.rootSessionId) registered.1)

theorem wrong_instance_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.asp.instanceId ≠ expected.asp.instanceId) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg (fun binding => binding.asp.instanceId) registered.1)

theorem wrong_registration_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.asp.registrationId ≠ expected.asp.registrationId) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg (fun binding => binding.asp.registrationId) registered.1)

theorem wrong_resident_id_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.residentId ≠ expected.residentId) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.residentId registered.1)

theorem wrong_route_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.routeDigest ≠ expected.routeDigest) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.routeDigest registered.1)

theorem wrong_generation_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.generation ≠ expected.generation) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.generation registered.1)

theorem wrong_profile_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.profileDigest ≠ expected.profileDigest) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.profileDigest registered.1)

theorem wrong_model_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.modelDigest ≠ expected.modelDigest) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.modelDigest registered.1)

theorem wrong_sandbox_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (mismatch : resident.binding.sandboxMode ≠ expected.sandboxMode) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact mismatch (congrArg ExactAgentBinding.sandboxMode registered.1)

theorem non_live_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (terminal : resident.lifecycle ≠ .live) :
    ¬ registeredBinding expected resident := by
  intro registered
  exact terminal registered.2.1

theorem completed_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (completed : resident.lifecycle = .completed) :
    ¬ registeredBinding expected resident := by
  apply non_live_cannot_register
  simp [completed]

theorem stopped_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (stopped : resident.lifecycle = .stopped) :
    ¬ registeredBinding expected resident := by
  apply non_live_cannot_register
  simp [stopped]

theorem unregistered_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (unregistered : resident.lifecycle = .unregistered) :
    ¬ registeredBinding expected resident := by
  apply non_live_cannot_register
  simp [unregistered]

theorem archived_cannot_register
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (archived : resident.lifecycle = .archived) :
    ¬ registeredBinding expected resident := by
  apply non_live_cannot_register
  simp [archived]

def resumeBinding (resident : ResidentGenerationFact) : ResidentGenerationFact :=
  resident

theorem resume_preserves_exact_live_identity
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (registered : registeredBinding expected resident) :
    registeredBinding expected (resumeBinding resident) :=
  registered

def TerminalBinding (resident : ResidentGenerationFact) : Prop :=
  resident.lifecycle = .completed ∨
    resident.lifecycle = .stopped ∨
    resident.lifecycle = .unregistered ∨
    resident.lifecycle = .archived

theorem terminal_binding_cannot_be_reused
    {expected : ExactAgentBinding}
    {resident : ResidentGenerationFact}
    (terminal : TerminalBinding resident) :
    ¬ registeredBinding expected resident := by
  rcases terminal with completed | stopped | unregistered | archived
  · exact completed_cannot_register completed
  · exact stopped_cannot_register stopped
  · exact unregistered_cannot_register unregistered
  · exact archived_cannot_register archived

structure RecreationEvidence where
  terminalResident : ResidentGenerationFact
  replacementResident : ResidentGenerationFact
  deriving DecidableEq, Repr

def validRecreation
    (expectedReplacement : ExactAgentBinding)
    (evidence : RecreationEvidence) : Prop :=
  TerminalBinding evidence.terminalResident ∧
    registeredBinding expectedReplacement evidence.replacementResident ∧
    evidence.terminalResident.binding.asp.instanceId ≠
      evidence.replacementResident.binding.asp.instanceId ∧
    evidence.terminalResident.binding.asp.registrationId ≠
      evidence.replacementResident.binding.asp.registrationId ∧
    evidence.terminalResident.binding.generation <
      evidence.replacementResident.binding.generation

theorem recreation_requires_new_instance_registration_and_higher_generation
    {expectedReplacement : ExactAgentBinding}
    {evidence : RecreationEvidence}
    (valid : validRecreation expectedReplacement evidence) :
    evidence.terminalResident.binding.asp.instanceId ≠
        evidence.replacementResident.binding.asp.instanceId ∧
      evidence.terminalResident.binding.asp.registrationId ≠
        evidence.replacementResident.binding.asp.registrationId ∧
      evidence.terminalResident.binding.generation <
        evidence.replacementResident.binding.generation :=
  ⟨valid.2.2.1, valid.2.2.2.1, valid.2.2.2.2⟩

end ASPProof.MultiAgentHostIdentityBinding
