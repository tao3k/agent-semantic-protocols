#[allow(dead_code)]
#[path = "../../src/command/hook_runtime_skill.rs"]
mod hook_runtime_skill;

use agent_semantic_hook::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ProviderExecution, RuntimeProfiles, RuntimeProfilesGeneratedBy,
};

use hook_runtime_skill::{
    render_agent_semantic_protocols_skill, replace_generated_block,
    validate_agent_semantic_protocols_skill,
};

fn activation_provider(
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> ActivatedProviderConfig {
    ActivatedProviderConfig {
        manifest_id: format!("agent.semantic-protocols.providers.{language_id}.{provider_id}"),
        manifest_digest: "sha256:test".to_string(),
        language_id: language_id.to_string(),
        provider_id: provider_id.to_string(),
        binary: binary.to_string(),
        execution: if matches!(language_id, "org" | "md") {
            ProviderExecution::Embedded
        } else {
            ProviderExecution::ExternalProcess
        },
        provider_command_prefix: vec![binary.to_string()],
        coverage: ActivationCoverage {
            package_roots: vec![".".to_string()],
            source_roots: Vec::new(),
            config_files: Vec::new(),
            source_extensions: Vec::new(),
            ignored_path_prefixes: Vec::new(),
        },
    }
}

fn test_activation() -> HookActivation {
    HookActivation {
        schema_id: "agent.semantic-protocols.hook.activation".to_string(),
        schema_version: "1".to_string(),
        protocol_id: "agent.semantic-protocols.hook".to_string(),
        protocol_version: "1".to_string(),
        project_root: "/tmp/asp-test".to_string(),
        generated_by: ActivationGeneratedBy {
            runtime: "asp".to_string(),
            version: "0.1.0".to_string(),
        },
        generated_at: None,
        providers: vec![
            activation_provider("rust", "rs-harness", "rs-harness"),
            activation_provider("org", "orgize", "asp"),
        ],
    }
}

fn test_runtime_profiles() -> RuntimeProfiles {
    RuntimeProfiles {
        schema_id: "agent.semantic-protocols.runtime.profiles".to_string(),
        schema_version: "1".to_string(),
        protocol_id: "agent.semantic-protocols.runtime".to_string(),
        protocol_version: "1".to_string(),
        project_root: "/tmp/asp-test".to_string(),
        runtime_home: "/tmp/asp-test/.cache/agent-semantic-protocol/runtime".to_string(),
        generated_by: RuntimeProfilesGeneratedBy {
            runtime: "asp".to_string(),
            version: "0.1.0".to_string(),
        },
        generated_at: None,
        providers: Vec::new(),
    }
}

#[test]
fn replaces_named_generated_block_without_removing_markers() {
    let template =
        "before\n# BEGIN_ASP_GENERATED providers\nold\n# END_ASP_GENERATED providers\nafter";
    let rendered = replace_generated_block(template, "providers", "new").unwrap();

    assert!(
        rendered.contains("# BEGIN_ASP_GENERATED providers\nnew\n# END_ASP_GENERATED providers")
    );
    assert!(!rendered.contains("old"));
}

#[test]
fn renders_org_contract_provider_subtrees_from_activation() {
    let rendered =
        render_agent_semantic_protocols_skill(&test_activation(), &test_runtime_profiles())
            .unwrap();

    assert!(rendered.contains("# BEGIN_ASP_GENERATED activation"));
    assert!(rendered.contains("# BEGIN_ASP_GENERATED providers"));
    assert!(rendered.contains("** rust"));
    assert!(rendered.contains(":LANGUAGE_ID: rust"));
    assert!(rendered.contains(":FACADE: asp rust"));
    assert!(rendered.contains(":DOCUMENT_PROVIDER: false"));
    assert!(rendered.contains("** org"));
    assert!(rendered.contains(":DOCUMENT_PROVIDER: true"));
    assert!(
        rendered.contains("Use =asp org query= for parser-owned document elements and metadata.")
    );
    assert!(!rendered.contains("SKILL.md"));
    assert!(!rendered.contains("/tmp/asp-test"));
}

#[test]
fn rendered_skill_satisfies_org_contract() {
    let rendered =
        render_agent_semantic_protocols_skill(&test_activation(), &test_runtime_profiles())
            .unwrap();

    validate_agent_semantic_protocols_skill(&rendered).unwrap();
}

#[test]
fn org_contract_rejects_missing_provider_contracts_section() {
    let rendered =
        render_agent_semantic_protocols_skill(&test_activation(), &test_runtime_profiles())
            .unwrap();
    let broken = rendered.replace("* Provider Contracts", "* Provider Contract Drift");

    let error = validate_agent_semantic_protocols_skill(&broken).unwrap_err();

    assert!(error.contains("generated SKILL.org does not match Org contract"));
    assert!(error.contains("asp.skill.section.provider-contracts"));
}
