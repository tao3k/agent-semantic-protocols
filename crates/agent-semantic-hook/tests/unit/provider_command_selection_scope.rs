use super::ProviderCommandSelectionScopeV1;

#[test]
fn target_language_excludes_unrelated_provider_receipts() {
    let scope = ProviderCommandSelectionScopeV1::TargetLanguage("gerbil-scheme".into());

    assert!(scope.selects(&"gerbil-scheme".into(), &"gerbil-scheme-harness".into()));
    assert!(!scope.selects(&"rust".into(), &"rs-harness".into()));
}

#[test]
fn target_provider_id_selects_only_the_requested_provider() {
    let scope = ProviderCommandSelectionScopeV1::TargetProviderId("gerbil-scheme-harness".into());

    assert!(scope.selects(&"gerbil-scheme".into(), &"gerbil-scheme-harness".into()));
    assert!(!scope.selects(&"rust".into(), &"rs-harness".into()));
}

#[test]
fn complete_generation_selects_every_provider() {
    let scope = ProviderCommandSelectionScopeV1::CompleteGeneration;

    assert!(scope.selects(&"gerbil-scheme".into(), &"gerbil-scheme-harness".into()));
    assert!(scope.selects(&"rust".into(), &"rs-harness".into()));
}
