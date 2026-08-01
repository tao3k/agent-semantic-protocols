import ASPProof.Audit.ProjectScopeBootstrap

open ASPProof.ProjectScopeBootstrap

example
    (languageId providerId packageRoot defaultRoot extension : String) :
    deriveProjectScope
      ⟨languageId, providerId, [defaultRoot], [extension]⟩
      ⟨packageRoot, true, []⟩
      [] = some ⟨languageId, providerId, [defaultRoot], [extension]⟩ := by
  apply registered_default_scope_bootstraps_without_source_candidates
  simp

example
    (registered : RegisteredLanguageScope)
    (manifest : PackageManifestSemantics)
    (paths : List String) :
    deriveProjectScope registered manifest [] =
      deriveProjectScope registered manifest paths :=
  scope_is_independent_of_candidate_source_paths registered manifest [] paths
