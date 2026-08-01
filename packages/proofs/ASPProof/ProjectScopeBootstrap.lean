namespace ASPProof.ProjectScopeBootstrap

structure RegisteredLanguageScope where
  languageId : String
  providerId : String
  defaultTargetRoots : List String
  sourceExtensions : List String
  deriving DecidableEq, Repr

structure PackageManifestSemantics where
  packageRoot : String
  usesDefaultTarget : Bool
  explicitSourceRoots : List String
  deriving DecidableEq, Repr

def inferredRoots
    (registered : RegisteredLanguageScope)
    (manifest : PackageManifestSemantics) : List String :=
  let defaults :=
    if manifest.usesDefaultTarget then registered.defaultTargetRoots else []
  (manifest.explicitSourceRoots ++ defaults).eraseDups

structure ProjectScope where
  languageId : String
  providerId : String
  roots : List String
  extensions : List String
  deriving DecidableEq, Repr

/-- Package-manager semantics own scope derivation. Candidate source paths are
evidence consumed after scope admission and cannot be a prerequisite. -/
def deriveProjectScope
    (registered : RegisteredLanguageScope)
    (manifest : PackageManifestSemantics)
    (_candidateSourcePaths : List String) : Option ProjectScope :=
  match inferredRoots registered manifest with
  | [] => none
  | roots =>
      some ⟨registered.languageId, registered.providerId, roots,
        registered.sourceExtensions⟩

theorem scope_is_independent_of_candidate_source_paths
    (registered : RegisteredLanguageScope)
    (manifest : PackageManifestSemantics)
    (left right : List String) :
    deriveProjectScope registered manifest left =
      deriveProjectScope registered manifest right := by
  rfl

theorem registered_default_scope_bootstraps_without_source_candidates
    (languageId providerId packageRoot : String)
    (defaultRoots extensions : List String)
    (h : defaultRoots ≠ []) :
    deriveProjectScope
      ⟨languageId, providerId, defaultRoots, extensions⟩
      ⟨packageRoot, true, []⟩
      [] = some ⟨languageId, providerId, defaultRoots.eraseDups, extensions⟩ := by
  cases defaultRoots with
  | nil => contradiction
  | cons head tail =>
      have hne : (head :: tail).eraseDups ≠ [] := by
        intro hnil
        have hmem : head ∈ (head :: tail).eraseDups := by simp
        simp [hnil] at hmem
      simp [deriveProjectScope, inferredRoots]

theorem explicit_scope_preserves_registered_language_contract
    (registered : RegisteredLanguageScope)
    (packageRoot explicitRoot : String) :
    deriveProjectScope registered ⟨packageRoot, false, [explicitRoot]⟩ [] =
      some ⟨registered.languageId, registered.providerId, [explicitRoot],
        registered.sourceExtensions⟩ := by
  rfl

inductive BootstrapPhase where
  | manifestParsed
  | scopeAdmitted
  | sourceCandidatesMaterialized
  | generationCommitted
  deriving DecidableEq, Repr

def phaseRank : BootstrapPhase → Nat
  | .manifestParsed => 3
  | .scopeAdmitted => 2
  | .sourceCandidatesMaterialized => 1
  | .generationCommitted => 0

inductive Advances : BootstrapPhase → BootstrapPhase → Prop where
  | admitScope : Advances .manifestParsed .scopeAdmitted
  | materializeCandidates : Advances .scopeAdmitted .sourceCandidatesMaterialized
  | commitGeneration : Advances .sourceCandidatesMaterialized .generationCommitted

theorem bootstrap_transition_decreases_rank
    {before after : BootstrapPhase} (h : Advances before after) :
    phaseRank after < phaseRank before := by
  cases h <;> decide

end ASPProof.ProjectScopeBootstrap
