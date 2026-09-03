import ASPProof.ProviderProjectionFailureIsolation

namespace ASPProof.Audit.ProviderProjectionFailureIsolation

open ASPProof.ProviderProjectionFailureIsolation

theorem v1_failure_isolation_strictly_accepts_more_than_legacy :
    Publishable isolatedCandidate ∧ ¬ LegacyPublishable isolatedCandidate := by
  exact ⟨syntax_failure_does_not_remove_source_membership,
    legacy_all_or_nothing_rejects_isolated_candidate⟩

theorem syntax_failure_cannot_forge_semantic_facts :
    unavailableOwner.items = [] ∧ unavailableOwner.relations = [] := by
  exact unavailable_owner_cannot_publish_items unavailableOwner rfl
    syntax_unavailable_owner_is_admissible

end ASPProof.Audit.ProviderProjectionFailureIsolation
