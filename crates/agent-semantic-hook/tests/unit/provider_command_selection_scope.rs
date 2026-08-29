use super::ProviderCommandSelectionScopeV1;

#[test]
fn target_language_excludes_unrelated_provider_receipts() {
    let scope = ProviderCommandSelectionScopeV1::TargetLanguage("gerbil-scheme".into());

    assert!(scope.selects(&"gerbil-scheme".into(), &"asp-gerbil-scheme".into()));
    assert!(!scope.selects(&"rust".into(), &"asp-rust".into()));
}

#[test]
fn target_provider_id_selects_only_the_requested_provider() {
    let scope = ProviderCommandSelectionScopeV1::TargetProviderId("asp-gerbil-scheme".into());

    assert!(scope.selects(&"gerbil-scheme".into(), &"asp-gerbil-scheme".into()));
    assert!(!scope.selects(&"rust".into(), &"asp-rust".into()));
}

#[test]
fn complete_generation_selects_every_provider() {
    let scope = ProviderCommandSelectionScopeV1::CompleteGeneration;

    assert!(scope.selects(&"gerbil-scheme".into(), &"asp-gerbil-scheme".into()));
    assert!(scope.selects(&"rust".into(), &"asp-rust".into()));
}
