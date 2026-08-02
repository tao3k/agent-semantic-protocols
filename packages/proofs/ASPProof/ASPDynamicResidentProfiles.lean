namespace ASPProof.DynamicResidentProfiles

structure AgentProfile where
  name : String
  deriving DecidableEq, Repr

structure Resident where
  profile : AgentProfile
  generation : Nat
  deriving DecidableEq, Repr

/-- A registry is canonical by construction: one optional resident slot exists
    for every dynamically configured profile name. -/
structure ResidentRegistry where
  resolve : String → Option Resident

def Configured (profiles : List AgentProfile) (name : String) : Prop :=
  ∃ profile ∈ profiles, profile.name = name

def Routable
    (profiles : List AgentProfile)
    (registry : ResidentRegistry)
    (name : String) : Prop :=
  Configured profiles name ∧ ∃ resident, registry.resolve name = some resident

def CanonicalPerName (registry : ResidentRegistry) : Prop :=
  ∀ name first second,
    registry.resolve name = some first →
    registry.resolve name = some second →
    first = second

theorem function_registry_is_canonical_per_name
    (registry : ResidentRegistry) :
    CanonicalPerName registry := by
  intro name first second hFirst hSecond
  rw [hFirst] at hSecond
  cases hSecond
  rfl

def install
    (registry : ResidentRegistry)
    (profile : AgentProfile)
    (resident : Resident) : ResidentRegistry :=
  { resolve := fun name =>
      if name = profile.name then some resident else registry.resolve name }

theorem install_resolves_arbitrary_configured_name
    (registry : ResidentRegistry)
    (profile : AgentProfile)
    (resident : Resident) :
    (install registry profile resident).resolve profile.name = some resident := by
  simp [install]

theorem install_preserves_other_profile
    (registry : ResidentRegistry)
    (profile : AgentProfile)
    (resident : Resident)
    (otherName : String)
    (hDifferent : otherName ≠ profile.name) :
    (install registry profile resident).resolve otherName = registry.resolve otherName := by
  simp [install, hDifferent]

theorem configured_names_are_not_a_fixed_enumeration
    (profiles : List AgentProfile)
    (profile : AgentProfile)
    (hMember : profile ∈ profiles) :
    Configured profiles profile.name := by
  exact ⟨profile, hMember, rfl⟩

theorem artifact_without_config_is_not_routable
    (profiles : List AgentProfile)
    (registry : ResidentRegistry)
    (name : String)
    (hNotConfigured : ¬ Configured profiles name) :
    ¬ Routable profiles registry name := by
  intro hRoutable
  exact hNotConfigured hRoutable.1

/-- Replacement is pointwise: a newer generation occupies the same configured
    name slot instead of creating a suffixed or second resident identity. -/
def replace
    (registry : ResidentRegistry)
    (profile : AgentProfile)
    (newGeneration : Nat) : ResidentRegistry :=
  install registry profile { profile, generation := newGeneration }

theorem replacement_keeps_the_same_dynamic_name
    (registry : ResidentRegistry)
    (profile : AgentProfile)
    (newGeneration : Nat) :
    (replace registry profile newGeneration).resolve profile.name =
      some { profile, generation := newGeneration } := by
  simp [replace, install]

end ASPProof.DynamicResidentProfiles
