#[allow(clippy::module_inception)]
#[path = "../../src/command/hook_runtime_skill.rs"]
mod hook_runtime_skill;

use agent_semantic_hook::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ProviderExecution, RuntimeProfiles, RuntimeProfilesGeneratedBy,
};
use std::time::{SystemTime, UNIX_EPOCH};

use hook_runtime_skill::hook_runtime_skill_render::validate_agent_semantic_protocols_skill;
use hook_runtime_skill::{
    install_agent_semantic_protocols_agent_config, install_agent_semantic_protocols_plugin_skill,
    install_agent_semantic_protocols_skill,
};

fn activation_provider(
    language_id: &str,
    provider_id: &str,
    binary: &str,
) -> ActivatedProviderConfig {
    let manifest = agent_semantic_hook::builtin_provider_manifests()
        .into_iter()
        .find(|manifest| manifest.language_id == language_id && manifest.provider_id == provider_id)
        .expect("canonical builtin provider manifest");
    ActivatedProviderConfig {
        manifest_id: manifest.manifest_id.clone(),
        manifest_digest: agent_semantic_hook::provider_manifest_digest(&manifest)
            .expect("digest canonical builtin provider manifest"),
        language_id: manifest.language_id.clone(),
        provider_id: manifest.provider_id.clone(),
        binary: binary.to_string(),
        execution: manifest.execution,
        provider_command_prefix: vec![binary.to_string()],
        search_capabilities: manifest.search_capabilities.clone(),
        semantic_facts_descriptor: manifest.semantic_facts_descriptor.clone(),
        query_pack_descriptor: manifest.query_pack_descriptor.clone(),
        semantic_registry_digest: agent_semantic_hook::semantic_registry_digest(),
        routes: agent_semantic_hook::materialize_provider_routes(&manifest)
            .expect("materialize canonical builtin provider routes"),
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
        schema_authority: "https://tao3k.github.io/agent-semantic-protocols/schemas/".to_string(),
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
fn renders_org_skill_from_languages_org_contract() {
    let rendered = installed_skill_text("rendered-provider-contracts");

    assert!(rendered.contains("* ASP Org"));
    assert!(rendered.contains(":SKILL_ID: asp-org"));
    assert!(rendered.contains(":SKILL_DESCRIPTION: Use when"));
    assert!(rendered.contains("** Use Boundary"));
    assert!(rendered.contains("** State Workflow"));
    assert!(rendered.contains("asp paths --get orgArtifacts"));
    assert!(rendered.contains("asp paths --get orgStateSkill"));
    assert!(!rendered.contains("Contract Assertions"));
    assert!(!rendered.contains("asp-skill-has-root-heading"));
    assert!(!rendered.contains("SKILL.contract.org"));
    assert!(!rendered.contains("Generated from the repository root =SKILL.org="));
    assert!(!rendered.contains("#+CONTRACT_ORG:"));
    assert!(!rendered.contains("SKILL.md"));
    assert!(!rendered.contains("/tmp/asp-test"));
}

#[test]
fn rendered_skill_satisfies_org_contract() {
    let rendered = installed_skill_text("rendered-skill-contract");

    validate_agent_semantic_protocols_skill(&rendered).unwrap();
}

#[test]
fn skill_contract_template_keeps_repo_local_refer_org() {
    let contract = include_str!("../../../../languages/org/contracts/asp.skill.v1.org");

    assert!(
        contract.contains(":SKILL_ID: asp-org"),
        "source asp.skill.v1.org must own the ASP Org skill template"
    );
    assert!(contract.contains("** Contract Assertions"));
    assert!(
        !contract.contains(":REFER_ORG: .cache/agent-semantic-protocol"),
        "source asp.skill.v1.org must not hard-code an installed state-tree path"
    );
}

#[test]
fn org_contract_rejects_missing_state_workflow_section() {
    let rendered = installed_skill_text("broken-provider-contract");
    let broken = rendered.replace("** State Workflow", "** State Drift");

    let error = validate_agent_semantic_protocols_skill(&broken).unwrap_err();

    assert!(error.contains("generated SKILL.org does not match Org contract"));
    assert!(error.contains("asp.skill.has-state-workflow"), "{error}");
}

fn installed_skill_text(name: &str) -> String {
    let root = temp_project_root(name);
    let installed =
        install_agent_semantic_protocols_skill(&root, &test_activation(), &test_runtime_profiles())
            .unwrap();
    let skill_path = installed.skill_path.expect("skill path");
    let rendered = std::fs::read_to_string(&skill_path).expect("read installed skill");
    let _ = std::fs::remove_dir_all(root);
    rendered
}

#[test]
fn install_project_skill_does_not_write_codex_plugin_skill() {
    let root = temp_project_root("skill-project-only");
    write_plugin_manifest(&root);
    let project_contract_path = root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org");
    let plugin_contract_path = root
        .join("asp-codex-plugin")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org");
    write_stale_contract(&project_contract_path);
    write_stale_contract(&plugin_contract_path);

    let installed =
        install_agent_semantic_protocols_skill(&root, &test_activation(), &test_runtime_profiles())
            .unwrap();
    let project_skill_path = installed.skill_path.expect("project skill path");
    assert!(
        installed.plugin_skill_path.is_none(),
        "project skill install must not mirror SKILL.org into the Codex plugin"
    );
    assert!(project_skill_path.is_file(), "project SKILL.org missing");
    assert!(
        !project_skill_path
            .with_file_name("SKILL.contract.org")
            .exists(),
        "project skill install must remove stale SKILL.contract.org"
    );
    assert!(
        !plugin_contract_path.with_file_name("SKILL.org").exists(),
        "project skill install must not create plugin SKILL.org"
    );
    assert!(
        plugin_contract_path.exists(),
        "project skill install must not manage plugin SKILL.contract.org"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn install_plugin_skill_writes_only_codex_plugin_skill() {
    let root = temp_project_root("skill-plugin-only");
    let project_skill_path = root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org");
    let plugin_contract_path =
        codex_plugin_cache_skill_path(&root).with_file_name("SKILL.contract.org");
    write_stale_contract(&plugin_contract_path);

    let _global_scope = crate::hook_runtime_skill::hook_runtime_skill::PluginSkillScope::Global;
    let installed = install_agent_semantic_protocols_plugin_skill(
        &root,
        crate::hook_runtime_skill::hook_runtime_skill::PluginSkillScope::Project,
        &test_activation(),
        &test_runtime_profiles(),
    )
    .unwrap();
    assert!(
        installed.skill_path.is_none(),
        "plugin skill install must not create project SKILL.org"
    );
    let plugin_skill_path = installed.plugin_skill_path.expect("plugin skill path");
    assert_eq!(plugin_skill_path, codex_plugin_cache_skill_path(&root));

    let plugin_skill = std::fs::read_to_string(&plugin_skill_path).expect("read plugin skill");
    assert!(plugin_skill.contains("* ASP Org"));
    assert!(plugin_skill.contains(":SKILL_ID: asp-org"));
    assert!(
        plugin_skill.contains("asp paths --get orgStateSkill"),
        "{plugin_skill}"
    );
    assert!(
        plugin_skill.contains("asp paths --get orgArtifacts"),
        "{plugin_skill}"
    );
    assert!(!plugin_skill.contains("SKILL.contract.org"));
    assert!(!plugin_skill.contains("Contract Assertions"));
    assert!(!plugin_skill.contains("asp-skill-has-root-heading"));
    assert!(!plugin_skill.contains("#+CONTRACT_ORG:"));
    assert!(!plugin_skill.contains(&root.display().to_string()));
    assert!(
        !project_skill_path.exists(),
        "Codex plugin skill install must not write .agents/skills"
    );
    assert!(
        !plugin_skill_path
            .with_file_name("SKILL.contract.org")
            .exists(),
        "plugin cache must not contain SKILL.contract.org"
    );
    assert!(
        !root.join("asp-codex-plugin").exists(),
        "plugin skill render must not create downstream asp-codex-plugin"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn install_agent_config_preserves_providers_and_adds_skill_config() {
    let root = temp_project_root("agent-config");
    let config_path = root.join(".agents").join("asp.toml");
    std::fs::create_dir_all(config_path.parent().expect("agent config parent"))
        .expect("create agent config parent");
    std::fs::write(
        &config_path,
        "[providers.rust]\n\
bin = \"tools/rs-harness\"\n\
\n\
[skills.agent-semantic-protocols]\n\
aspOrg = \"/old/ASP_ORG_SKILL.org#asp-org\"\n\
orgArtifacts = \"/old/artifacts/org\"\n\
\n\
[hook.agentOrgArtifacts]\n\
enabled = true\n\
inactiveAfterMinutes = 30\n\
artifactsPath = \"/old/artifacts/org\"\n\
entrySkillPath = \"/old/ASP_ORG_SKILL.org\"\n",
    )
    .expect("write provider config");

    let installed_path = install_agent_semantic_protocols_agent_config(&root).unwrap();
    assert_eq!(installed_path, config_path);
    let config = std::fs::read_to_string(&installed_path).expect("read agent config");

    assert!(config.contains("[providers.rust]"), "{config}");
    assert!(config.contains("bin = \"tools/rs-harness\""), "{config}");
    assert!(
        config.contains("[skills.agent-semantic-protocols]"),
        "{config}"
    );
    assert!(
        config.contains(
            "pluginSkill = \".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org\""
        ),
        "{config}"
    );
    assert!(!config.contains("template = \"SKILL.org\""), "{config}");
    assert!(!config.contains("projectSkill = "), "{config}");
    assert!(!config.contains("aspOrg"), "{config}");
    assert!(!config.contains("orgArtifacts"), "{config}");
    assert!(!config.contains("[hook.agentOrgArtifacts]"), "{config}");
    assert!(!config.contains("artifactsPath"), "{config}");
    assert!(!config.contains("entrySkillPath"), "{config}");
    assert!(!config.contains("orgSkill"), "{config}");

    let _ = std::fs::remove_dir_all(root);
}

fn write_stale_contract(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().expect("contract parent"))
        .expect("create stale contract parent");
    std::fs::write(path, "* stale user-layer contract\n").expect("write stale contract");
}

fn write_plugin_manifest(root: &std::path::Path) {
    let manifest_path = root
        .join("asp-codex-plugin")
        .join(".codex-plugin")
        .join("plugin.json");
    std::fs::create_dir_all(manifest_path.parent().unwrap()).expect("create plugin manifest dir");
    std::fs::write(
        manifest_path,
        r#"{"name":"asp-codex-plugin","version":"0.1.0","description":"test","author":{"name":"ASP"},"skills":"./skills/"}"#,
    )
    .expect("write plugin manifest");
}

fn codex_plugin_cache_skill_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".codex")
        .join("plugins")
        .join("cache")
        .join("asp-project")
        .join("asp-codex-plugin")
        .join("0.1.0")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org")
}

fn temp_project_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-protocol-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp project root");
    std::fs::create_dir_all(root.join(".git")).expect("create temp git marker");
    root.canonicalize().expect("canonical temp project root")
}
