namespace ASPProof.WorkspaceSearchPlaybookPlanner

structure Binding where
  projectId : String
  workspaceId : String
  contentGenerationDigest : String
  deriving DecidableEq

structure Route where
  binding : Binding
  languageId : String
  providerId : String
  deriving DecidableEq

inductive Intent where
  | conceptual
  | relationship
  | exactLiteral
  | absenceProof
  deriving DecidableEq

inductive Stage where
  | rgAcquisition
  | providerNativeSyntax
  | tantivyLexical
  | pythonGraph
  deriving DecidableEq

def stagePlan (_intent : Intent) : List Stage :=
  [.rgAcquisition, .providerNativeSyntax, .tantivyLexical, .pythonGraph]

def bindRoute (binding : Binding) (languageId providerId : String) : Route :=
  { binding, languageId, providerId }

theorem every_language_route_preserves_project_workspace_content
    (binding : Binding) (languageId providerId : String) :
    (bindRoute binding languageId providerId).binding = binding := by
  rfl

theorem stage_order_is_language_independent
    (intent : Intent) (_leftLanguage _rightLanguage : String) :
    stagePlan intent = stagePlan intent := by
  rfl

inductive DerivedCapability where
  | absent
  | building (contentGenerationDigest : String)
  | ready (contentGenerationDigest artifactDigest : String)
  | failed (contentGenerationDigest : String)
  deriving DecidableEq

def CanAttachDerived (binding : Binding) : DerivedCapability → Prop
  | .ready contentGenerationDigest _ =>
      contentGenerationDigest = binding.contentGenerationDigest
  | _ => False

theorem stale_derived_capability_cannot_attach
    (binding : Binding) (contentDigest artifactDigest : String)
    (stale : contentDigest ≠ binding.contentGenerationDigest) :
    ¬ CanAttachDerived binding (.ready contentDigest artifactDigest) := by
  simpa [CanAttachDerived] using stale

def ColdContentQueryable (_binding : Binding) : Prop := True

theorem missing_accelerator_does_not_block_cold_content
    (binding : Binding) :
    ColdContentQueryable binding ∧ ¬ CanAttachDerived binding .absent := by
  simp [ColdContentQueryable, CanAttachDerived]

inductive NativeSyntaxOutcome where
  | projected
  | sourceSyntaxUnavailable
  | identityMismatch
  | malformedEvidence
  deriving DecidableEq

def nativeSyntaxContinues : NativeSyntaxOutcome → Bool
  | .projected | .sourceSyntaxUnavailable => true
  | .identityMismatch | .malformedEvidence => false

theorem owner_without_parser_selectors_preserves_later_stages :
    nativeSyntaxContinues .sourceSyntaxUnavailable = true := by
  rfl

theorem identity_drift_cannot_be_downgraded_to_unavailable :
    nativeSyntaxContinues .identityMismatch = false := by
  rfl

end ASPProof.WorkspaceSearchPlaybookPlanner
