use super::is_provider_owned_structural_selector_query;

#[test]
fn every_registered_programming_language_exact_selector_enters_the_shared_exact_route() {
    let mut covered = 0usize;

    for manifest in agent_semantic_hook::schema_registry_provider_manifests() {
        let language_id = manifest.language_id().as_str();
        if agent_semantic_hook::registered_provider_kind(language_id)
            .expect("registered provider kind")
            != agent_semantic_hook::RegisteredProviderKind::ProgrammingLanguage
        {
            continue;
        }

        let args = vec![
            "query".to_string(),
            "--selector".to_string(),
            format!("{language_id}://src/example#item/function/example"),
            "--projection".to_string(),
            "source".to_string(),
        ];
        assert!(
            is_provider_owned_structural_selector_query(language_id, &args),
            "registered programming-language provider {language_id} escaped the shared exact route"
        );
        covered += 1;
    }

    assert!(
        covered > 1,
        "expected multiple registered programming-language providers"
    );
}
