use agent_semantic_hook::registered_provider_development_v1;
use serde_json::Value;

const WORKSPACE_INSTALL_DESCRIPTORS: &[(&str, &str, &str)] = &[
    (
        "rust",
        "provider/asp-provider-workspace-install.json",
        include_str!(
            "../../../../../languages/rust-lang-project-harness/provider/asp-provider-workspace-install.json"
        ),
    ),
    (
        "typescript",
        "provider/asp-provider-workspace-install.json",
        include_str!(
            "../../../../../languages/typescript-lang-project-harness/provider/asp-provider-workspace-install.json"
        ),
    ),
    (
        "python",
        "provider/asp-provider-workspace-install.json",
        include_str!(
            "../../../../../languages/python-lang-project-harness/provider/asp-provider-workspace-install.json"
        ),
    ),
    (
        "gerbil-scheme",
        "provider/asp-provider-workspace-install.json",
        include_str!(
            "../../../../../languages/gerbil-scheme-language-project-harness/provider/asp-provider-workspace-install.json"
        ),
    ),
    (
        "julia",
        "asp-provider-workspace-install.json",
        include_str!(
            "../../../../../languages/JuliaLangProjectHarness.jl/juliac/asp-provider-workspace-install.json"
        ),
    ),
];

#[test]
fn registered_external_providers_reference_valid_workspace_install_descriptors() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../../../schemas/provider-workspace-install.v1.schema.json"
    ))
    .expect("workspace install schema");
    let validator = jsonschema::validator_for(&schema).expect("compile workspace install schema");

    for (language_id, expected_reference, descriptor_json) in WORKSPACE_INSTALL_DESCRIPTORS {
        let registration =
            registered_provider_development_v1(language_id).expect("registered provider");
        assert_eq!(
            registration.development.build_binding,
            "provider-workspace-install-v1"
        );
        assert_eq!(
            registration.development.workspace_install.as_deref(),
            Some(*expected_reference)
        );
        let descriptor: Value =
            serde_json::from_str(descriptor_json).expect("workspace install descriptor");
        validator
            .validate(&descriptor)
            .unwrap_or_else(|error| panic!("{language_id} descriptor failed schema: {error}"));
        assert_eq!(
            descriptor["providerId"],
            Value::String(registration.provider_id.as_str().to_string())
        );
        assert_eq!(
            descriptor["binary"],
            Value::String(registration.binary.clone())
        );
    }
}
