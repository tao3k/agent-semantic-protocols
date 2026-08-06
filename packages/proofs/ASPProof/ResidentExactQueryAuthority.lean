namespace ASPProof.ResidentExactQueryAuthority

inductive Process where
  | client
  | daemon
  deriving DecidableEq

inductive ExactEffect where
  | resolveSession
  | sendTypedRead
  | readResidentGeneration
  | renderProjection
  | openPointer
  | openMmap
  | openDatabase
  | runProvider
  | publishGeneration
  deriving DecidableEq

structure Step where
  process : Process
  effect : ExactEffect

def legalStep : Step → Prop
  | ⟨.client, .resolveSession⟩ => True
  | ⟨.client, .sendTypedRead⟩ => True
  | ⟨.client, .renderProjection⟩ => True
  | ⟨.daemon, .readResidentGeneration⟩ => True
  | _ => False

def legalTrace (trace : List Step) : Prop := ∀ step ∈ trace, legalStep step

theorem client_never_opens_pointer
    (trace : List Step) (h : legalTrace trace) :
    ⟨Process.client, ExactEffect.openPointer⟩ ∉ trace := by
  intro member
  have legal := h ⟨Process.client, ExactEffect.openPointer⟩ member
  exact legal

theorem client_never_opens_mmap
    (trace : List Step) (h : legalTrace trace) :
    ⟨Process.client, ExactEffect.openMmap⟩ ∉ trace := by
  intro member
  have legal := h ⟨Process.client, ExactEffect.openMmap⟩ member
  exact legal

theorem exact_read_never_publishes_generation
    (trace : List Step) (h : legalTrace trace) (process : Process) :
    ⟨process, ExactEffect.publishGeneration⟩ ∉ trace := by
  intro member
  have legal := h ⟨process, ExactEffect.publishGeneration⟩ member
  cases process <;> exact legal

theorem client_never_opens_database_or_provider
    (trace : List Step) (h : legalTrace trace) :
    ⟨Process.client, ExactEffect.openDatabase⟩ ∉ trace ∧
      ⟨Process.client, ExactEffect.runProvider⟩ ∉ trace := by
  constructor <;> intro member
  · exact h ⟨Process.client, ExactEffect.openDatabase⟩ member
  · exact h ⟨Process.client, ExactEffect.runProvider⟩ member

end ASPProof.ResidentExactQueryAuthority
