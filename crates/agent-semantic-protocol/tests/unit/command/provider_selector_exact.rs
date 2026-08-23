use super::is_provider_owned_structural_selector_query;

#[test]
fn every_registered_programming_language_exact_selector_enters_the_shared_exact_route() {
    let mut covered = 0usize;

    for registration in agent_semantic_provider_protocol::builtin_provider_registrations()
        .expect("builtin provider identities")
    {
        let language_id = registration.language_id.as_str();
        if crate::command::provider_install_registry::provider_install_registration(language_id)
            .is_err()
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
