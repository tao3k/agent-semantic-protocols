use super::is_runtime_exact_query;

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

        let selector = format!("{language_id}://src/example#item/function/example");
        for args in [
            vec![
                "query".to_string(),
                "--selector".to_string(),
                selector.clone(),
                "--projection".to_string(),
                "source".to_string(),
            ],
            vec![
                "query".to_string(),
                selector.clone(),
                "--projection".to_string(),
                "source".to_string(),
            ],
        ] {
            assert!(
                is_runtime_exact_query(&args),
                "registered programming-language provider {language_id} escaped the shared exact route for {args:?}"
            );
        }
        covered += 1;
    }

    assert!(
        covered > 1,
        "expected multiple registered programming-language providers"
    );
}

#[test]
fn normalized_owner_path_enters_the_runtime_exact_route_without_client_parsing() {
    let args = vec![
        "query".to_string(),
        "--selector".to_string(),
        "scheme/reasoning/core.ss".to_string(),
        "--projection".to_string(),
        "source".to_string(),
    ];
    assert!(is_runtime_exact_query(&args));
}
