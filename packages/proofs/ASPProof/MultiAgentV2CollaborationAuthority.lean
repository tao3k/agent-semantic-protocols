namespace ASPProof.MultiAgentV2CollaborationAuthority

inductive CollaborationTool where
  | spawnAgent
  | listAgents
  | followupTask
  | sendMessage
  | interruptAgent
  | waitAgent
  deriving DecidableEq, Repr

structure HostCall where
  hostNamespace : String
  tool : CollaborationTool
  deriving DecidableEq, Repr

def admitted (call : HostCall) : Bool :=
  call.hostNamespace == "collaboration"

theorem collaboration_is_the_only_codex_namespace (tool : CollaborationTool) :
    admitted ⟨"collaboration", tool⟩ = true := by
  rfl

theorem foreign_namespace_fails_closed (hostNamespace : String) (tool : CollaborationTool)
    (different : hostNamespace ≠ "collaboration") :
    admitted ⟨hostNamespace, tool⟩ = false := by
  simp [admitted, different]

structure RegistrationFact where
  workspaceIdentity : String
  rootSessionId : String
  parentSessionId : String
  childSessionId : String
  canonicalAgentPath : String
  physicalGeneration : Nat
  deriving DecidableEq, Repr

def sameChild (host runtime : RegistrationFact) : Bool := host == runtime

theorem runtime_registration_requires_the_exact_host_fact
    (host runtime : RegistrationFact) (different : host ≠ runtime) :
    sameChild host runtime = false := by
  simp [sameChild, different]

end ASPProof.MultiAgentV2CollaborationAuthority
