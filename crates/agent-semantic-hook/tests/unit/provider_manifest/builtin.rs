use agent_semantic_hook::builtin_provider_manifests;

#[test]
fn builtin_manifests_include_julia_juliac_provider() {
    let manifests = builtin_provider_manifests();
    let julia = manifests
        .iter()
        .find(|manifest| manifest.language_id == "julia")
        .expect("julia manifest");

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
        julia.routes.guide.as_ref().expect("guide route").argv,
        ["asp-julia-harness", "guide", "{projectRoot}"]
    );
    assert_eq!(
        julia.routes.query.as_ref().expect("query route").argv,
        [
            "asp-julia-harness",
            "search",
            "query",
            "--from-hook",
            "direct-source-read",
            "--selector",
            "{selector}",
            "{termArgs}",
            "--surface",
            "owners,tests",
            "--view",
            "seeds",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        julia.routes.ingest.argv,
        [
            "asp-julia-harness",
            "search",
            "ingest",
            "owner",
            "tests",
            "--view",
            "seeds",
            "{projectRoot}"
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

    assert_eq!(org.provider_id, "orgize");
    assert_eq!(org.binary, "asp");
    assert_eq!(org.execution.as_str(), "embedded");
    assert!(org.source.default_extensions.contains(&".org".to_string()));
    assert_eq!(
        org.routes.query.as_ref().expect("org query route").argv,
        [
            "asp",
            "org",
            "query",
            "{termArgs}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        org.routes.owner.argv,
        [
            "asp",
            "org",
            "query",
            "--selector",
            "{path}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        org.routes.fzf.argv,
        [
            "asp",
            "org",
            "query",
            "--term",
            "{query}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );

    assert_eq!(md.provider_id, "orgize");
    assert_eq!(md.binary, "asp");
    assert_eq!(md.execution.as_str(), "embedded");
    assert!(md.source.default_extensions.contains(&".md".to_string()));
    assert_eq!(
        md.routes.query.as_ref().expect("md query route").argv,
        [
            "asp",
            "md",
            "query",
            "{termArgs}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        md.routes.owner.argv,
        [
            "asp",
            "md",
            "query",
            "--selector",
            "{path}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
    assert_eq!(
        md.routes.fzf.argv,
        [
            "asp",
            "md",
            "query",
            "--term",
            "{query}",
            "--view",
            "metadata",
            "{projectRoot}"
        ]
    );
}
