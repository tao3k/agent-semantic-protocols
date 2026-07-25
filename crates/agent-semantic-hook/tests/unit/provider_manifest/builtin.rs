use agent_semantic_hook::builtin_provider_manifests;

#[test]
fn builtin_manifests_include_julia_juliac_provider() {
    let manifests = builtin_provider_manifests();
    let julia = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == "julia")
        .expect("julia manifest");
    let julia_routes =
        agent_semantic_hook::materialize_provider_routes(julia).expect("julia routes");

    assert_eq!(julia.provider_id().as_str(), "julia-lang-project-harness");
    assert_eq!(julia.binary(), "asp-julia-harness");
    assert!(
        julia
            .source()
            .default_extensions
            .contains(&".jl".to_string())
    );
    assert!(
        julia
            .source()
            .default_config_files
            .contains(&"Project.toml".to_string())
    );
    assert_eq!(
        julia_routes.guide.as_ref().expect("guide route").argv,
        ["asp-julia-harness", "guide", "{workspace}"]
    );
    assert_eq!(
        julia_routes.query.as_ref().expect("query route").argv,
        [
            "asp-julia-harness",
            "query",
            "--selector",
            "{owner}",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    assert_eq!(
        julia_routes.ingest.argv,
        [
            "asp-julia-harness",
            "search",
            "ingest",
            "owner",
            "tests",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    assert!(
        julia
            .source()
            .default_ignored_path_prefixes
            .contains(&".devenv".to_string())
    );
}

fn json_string_paths_containing(
    value: &serde_json::Value,
    path: &str,
    needle: &str,
    matches: &mut Vec<String>,
) {
    match value {
        serde_json::Value::String(text) if text.contains(needle) => {
            matches.push(path.to_string());
        }
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                json_string_paths_containing(value, &format!("{path}/{index}"), needle, matches);
            }
        }
        serde_json::Value::Object(fields) => {
            for (field, value) in fields {
                json_string_paths_containing(value, &format!("{path}/{field}"), needle, matches);
            }
        }
        _ => {}
    }
}

#[test]
fn registered_provider_query_routes_are_exact_selector_only() {
    let registry_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/semantic-language-registry.providers.v1.json"
    ));
    let registry_json =
        serde_json::from_str::<serde_json::Value>(registry_source).expect("valid v1 registry JSON");
    for legacy_method in ["query/document", "query/direct-source-read"] {
        let mut legacy_paths = Vec::new();
        json_string_paths_containing(&registry_json, "", legacy_method, &mut legacy_paths);
        assert!(
            legacy_paths.is_empty(),
            "shared v1 registry retains legacy query method {legacy_method} at {legacy_paths:?}"
        );
    }

    let manifests = builtin_provider_manifests();
    let mut exact_selector_languages = Vec::new();
    for manifest in &manifests {
        let language_id = manifest.language_id().as_str();
        let routes = agent_semantic_hook::materialize_provider_routes(manifest)
            .unwrap_or_else(|error| panic!("materialize {language_id} routes: {error}"));
        let Some(query) = routes.query.as_ref() else {
            continue;
        };
        exact_selector_languages.push(language_id);

        assert!(
            query.argv.iter().any(|arg| arg == "--selector"),
            "{language_id} query route must require an exact selector: {:?}",
            query.argv
        );
        assert!(
            !query.argv.iter().any(|arg| {
                matches!(
                    arg.as_str(),
                    "--from-hook" | "direct-source-read" | "--term" | "{query}"
                )
            }),
            "{language_id} exact-selector route retains discovery or direct-read arguments: {:?}",
            query.argv
        );
        let synthetic_selector =
            format!("{language_id}://src/provider#item/function/provider_contract");
        let canonical =
            agent_semantic_content_identity::CanonicalItemSelectorV1::parse(&synthetic_selector)
                .unwrap_or_else(|error| {
                    panic!(
                        "{language_id} query provider has no canonical selector identity: {error}"
                    )
                });
        assert_eq!(canonical.language_id.as_str(), language_id);
    }
    assert!(
        !exact_selector_languages.is_empty(),
        "provider registry must expose at least one exact-selector query route"
    );

    let rust = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == "rust")
        .expect("rust provider manifest");
    assert!(
        rust.search_capabilities().owner_items,
        "Rust harness implements owner-items and must advertise the capability"
    );
}

#[test]
fn builtin_manifests_include_document_language_providers() {
    let manifests = builtin_provider_manifests();
    let org = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == "org")
        .expect("org manifest");
    let md = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == "md")
        .expect("md manifest");
    let org_routes = agent_semantic_hook::materialize_provider_routes(org).expect("org routes");
    let md_routes = agent_semantic_hook::materialize_provider_routes(md).expect("md routes");

    assert_eq!(org.provider_id().as_str(), "orgize");
    assert_eq!(org.binary(), "orgize");
    assert_eq!(org.execution().as_str(), "external-process");
    assert!(org.search_capabilities().owner_items);
    assert!(
        org.source()
            .default_extensions
            .contains(&".org".to_string())
    );
    assert_eq!(
        org_routes.query.as_ref().expect("org query route").argv,
        [
            "asp",
            "org",
            "query",
            "--selector",
            "{selector}",
            "--view",
            "metadata",
            "{workspace}"
        ]
    );
    assert_eq!(
        org_routes.owner.argv,
        [
            "asp",
            "org",
            "search",
            "owner",
            "{owner}",
            "items",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    assert_eq!(
        org_routes.lexical.argv,
        [
            "asp",
            "org",
            "search",
            "lexical",
            "{query}",
            "owner",
            "tests",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );

    assert_eq!(md.provider_id().as_str(), "orgize");
    assert_eq!(md.binary(), "orgize");
    assert_eq!(md.execution().as_str(), "external-process");
    assert!(md.search_capabilities().owner_items);
    assert!(md.source().default_extensions.contains(&".md".to_string()));
    assert_eq!(
        md_routes.query.as_ref().expect("md query route").argv,
        [
            "asp",
            "md",
            "query",
            "--selector",
            "{selector}",
            "--view",
            "metadata",
            "{workspace}"
        ]
    );
    assert_eq!(
        md_routes.owner.argv,
        [
            "asp",
            "md",
            "search",
            "owner",
            "{owner}",
            "items",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
    assert_eq!(
        md_routes.lexical.argv,
        [
            "asp",
            "md",
            "search",
            "lexical",
            "{query}",
            "owner",
            "tests",
            "--workspace",
            "{workspace}",
            "--view",
            "seeds"
        ]
    );
}
