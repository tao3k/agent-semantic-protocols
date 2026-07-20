use agent_semantic_hook::builtin_provider_manifests;

#[test]
fn builtin_manifests_include_julia_juliac_provider() {
    let manifests = builtin_provider_manifests();
    let julia = manifests
        .iter()
        .find(|manifest| manifest.language_id == "julia")
        .expect("julia manifest");
    let julia_routes =
        agent_semantic_hook::materialize_provider_routes(julia).expect("julia routes");

    assert_eq!(julia.provider_id, "julia-lang-project-harness");
    assert_eq!(julia.binary, "asp-julia-harness");
    assert!(julia.source.default_extensions.contains(&".jl".to_string()));
    assert!(
        julia
            .source
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
            "--from-hook",
            "direct-source-read",
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
            .source
            .default_ignored_path_prefixes
            .contains(&".devenv".to_string())
    );
}

#[test]
fn builtin_manifests_include_document_language_providers() {
    let manifests = builtin_provider_manifests();
    let org = manifests
        .iter()
        .find(|manifest| manifest.language_id == "org")
        .expect("org manifest");
    let md = manifests
        .iter()
        .find(|manifest| manifest.language_id == "md")
        .expect("md manifest");
    let org_routes = agent_semantic_hook::materialize_provider_routes(org).expect("org routes");
    let md_routes = agent_semantic_hook::materialize_provider_routes(md).expect("md routes");

    assert_eq!(org.provider_id, "orgize");
    assert_eq!(org.binary, "orgize");
    assert_eq!(org.execution.as_str(), "external-process");
    assert!(org.search_capabilities.owner_items);
    assert!(org.source.default_extensions.contains(&".org".to_string()));
    assert_eq!(
        org_routes.query.as_ref().expect("org query route").argv,
        [
            "asp",
            "org",
            "query",
            "--term",
            "{query}",
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

    assert_eq!(md.provider_id, "orgize");
    assert_eq!(md.binary, "orgize");
    assert_eq!(md.execution.as_str(), "external-process");
    assert!(md.search_capabilities.owner_items);
    assert!(md.source.default_extensions.contains(&".md".to_string()));
    assert_eq!(
        md_routes.query.as_ref().expect("md query route").argv,
        [
            "asp",
            "md",
            "query",
            "--term",
            "{query}",
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
