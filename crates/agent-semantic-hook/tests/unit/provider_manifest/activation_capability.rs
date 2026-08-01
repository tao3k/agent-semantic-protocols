use super::{activation_capability_coverage, provider_manifests};

#[test]
fn project_provider_activation_declares_provider_extensions_without_resolving_scope() {
    let manifests = provider_manifests();
    for (provider_id, extension, marker) in [
        ("rs-harness", ".rs", "Cargo.toml"),
        ("ts-harness", ".ts", "package.json"),
        ("py-harness", ".py", "pyproject.toml"),
        ("julia-lang-project-harness", ".jl", "Project.toml"),
        ("gerbil-scheme-harness", ".ss", "gerbil.pkg"),
    ] {
        let manifest = manifests
            .iter()
            .find(|manifest| manifest.provider_id().as_str() == provider_id)
            .unwrap_or_else(|| panic!("provider manifest: {provider_id}"));
        let capability = activation_capability_coverage(manifest)
            .unwrap_or_else(|error| panic!("provider activation: {provider_id}: {error}"));

        assert!(
            capability.package_roots.is_empty(),
            "provider={provider_id}"
        );
        assert!(
            capability
                .source_extensions
                .iter()
                .any(|candidate| candidate == extension),
            "provider={provider_id} extension={extension}"
        );
        assert!(
            capability
                .config_files
                .iter()
                .any(|candidate| candidate == marker),
            "provider={provider_id} marker={marker}"
        );
    }
}

#[test]
fn document_provider_activation_declares_extensions_without_scanning_candidates() {
    let manifests = provider_manifests();
    let org = manifests
        .iter()
        .find(|manifest| manifest.language_id().as_str() == "org")
        .expect("org provider manifest");
    let capability = activation_capability_coverage(org).expect("org activation capability");

    assert!(capability.package_roots.is_empty());
    assert!(capability.config_files.is_empty());
    assert!(
        capability
            .source_extensions
            .iter()
            .any(|extension| extension == ".org")
    );
}
