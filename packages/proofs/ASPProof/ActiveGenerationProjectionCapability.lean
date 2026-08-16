namespace ASPProof.ActiveGenerationProjectionCapability

abbrev GenerationDigest := String
abbrev RootDigest := String
abbrev ProviderCatalogDigest := String
abbrev Selector := String

inductive ProjectionMode where
  | source
  | callableSkeleton
  deriving DecidableEq

structure GenerationAuthority where
  generationDigest : GenerationDigest
  rootDigest : RootDigest
  providerCatalogDigest : ProviderCatalogDigest
  providerCatalogReadable : Prop
  ownerAdmitted : Selector → Prop
  selectorExposed : Selector → Prop
  requiredProjection : Selector → ProjectionMode
  projectionAdmitted : Selector → ProjectionMode → Prop

def Publishable (generation : GenerationAuthority) : Prop :=
  generation.providerCatalogReadable ∧
    ∀ selector,
      generation.selectorExposed selector →
        generation.ownerAdmitted selector ∧
          generation.projectionAdmitted
            selector
            (generation.requiredProjection selector)

theorem published_selector_has_same_generation_projection
    (generation : GenerationAuthority)
    (published : Publishable generation)
    (selector : Selector)
    (exposed : generation.selectorExposed selector) :
    generation.projectionAdmitted
      selector
      (generation.requiredProjection selector) := by
  exact (published.2 selector exposed).2

theorem missing_projection_prevents_publication
    (generation : GenerationAuthority)
    (selector : Selector)
    (exposed : generation.selectorExposed selector)
    (missing :
      ¬ generation.projectionAdmitted
        selector
        (generation.requiredProjection selector)) :
    ¬ Publishable generation := by
  intro published
  exact missing (published_selector_has_same_generation_projection
    generation published selector exposed)

theorem unreadable_provider_catalog_prevents_publication
    (generation : GenerationAuthority)
    (unreadable : ¬ generation.providerCatalogReadable) :
    ¬ Publishable generation := by
  intro published
  exact unreadable published.1

inductive LifecycleAction where
  | explicitStart
  | explicitRestart
  | ensure
  deriving DecidableEq

def MayClearOperatorStop : LifecycleAction → Prop
  | .explicitStart => True
  | .explicitRestart => True
  | .ensure => False

theorem background_ensure_preserves_operator_stop :
    ¬ MayClearOperatorStop .ensure := by
  exact id

theorem explicit_start_can_clear_operator_stop :
    MayClearOperatorStop .explicitStart := by
  trivial

theorem explicit_restart_can_clear_operator_stop :
    MayClearOperatorStop .explicitRestart := by
  trivial

end ASPProof.ActiveGenerationProjectionCapability
