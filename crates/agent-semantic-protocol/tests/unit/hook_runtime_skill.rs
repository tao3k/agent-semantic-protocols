#[allow(dead_code)]
#[allow(clippy::module_inception)]
#[path = "../../src/command/hook_runtime_skill.rs"]
mod hook_runtime_skill;

use agent_semantic_hook::{
    ActivatedProviderConfig, ActivationCoverage, ActivationGeneratedBy, HookActivation,
    ProviderExecution, RuntimeProfiles, RuntimeProfilesGeneratedBy,
};
use std::time::{SystemTime, UNIX_EPOCH};

use hook_runtime_skill::hook_runtime_skill_render::{
    replace_generated_block, validate_agent_semantic_protocols_skill,
};
use hook_runtime_skill::{
    install_agent_semantic_protocols_agent_config, install_agent_semantic_protocols_plugin_skill,
    install_agent_semantic_protocols_skill, render_agent_semantic_protocols_skill_contract,
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
    let rendered = installed_skill_text("rendered-provider-contracts");

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
    assert!(rendered.contains("asp paths --get projectRoot"));
    assert!(rendered.contains("asp paths --get orgStateSkill"));
    assert!(rendered.contains("asp paths --get orgArtifacts"));
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
    let contract = include_str!("../../../../SKILL.contract.org");

    assert!(
        contract.contains(":REFER_ORG: ./languages/org/skills/ASP_ORG.org#asp-org"),
        "source SKILL.contract.org must reference the source-tree ASP_ORG.org"
    );
    assert!(
        !contract.contains(":REFER_ORG: .cache/agent-semantic-protocol"),
        "source SKILL.contract.org must not hard-code an installed state-tree path"
    );
}

#[test]
fn renders_skill_contract_refer_org_relative_to_install_target() {
    let root = temp_project_root("skill-contract-refer-org");
    let org_state_skill_path = root
        .join(".cache")
        .join("agent-semantic-protocol")
        .join("org")
        .join("skills")
        .join("ASP_ORG.org");
    let project_contract_path = root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org");
    let plugin_contract_path = root
        .join(".codex")
        .join("plugins")
        .join("asp-codex-plugin")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org");

    let project_contract = render_agent_semantic_protocols_skill_contract(
        &project_contract_path,
        &org_state_skill_path,
    )
    .unwrap();
    let plugin_contract = render_agent_semantic_protocols_skill_contract(
        &plugin_contract_path,
        &org_state_skill_path,
    )
    .unwrap();

    assert!(project_contract.contains(
        ":REFER_ORG: ../../../.cache/agent-semantic-protocol/org/skills/ASP_ORG.org#asp-org"
    ));
    assert!(plugin_contract.contains(
        ":REFER_ORG: ../../../../../.cache/agent-semantic-protocol/org/skills/ASP_ORG.org#asp-org"
    ));
    assert_ne!(project_contract, plugin_contract);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn org_contract_rejects_missing_provider_contracts_section() {
    let rendered = installed_skill_text("broken-provider-contract");
    let broken = rendered.replace("* Provider Contracts", "* Provider Contract Drift");

    let error = validate_agent_semantic_protocols_skill(&broken).unwrap_err();

    assert!(error.contains("generated SKILL.org does not match Org contract"));
    assert!(error.contains("asp.skill.section.provider-contracts"));
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
    let project_skill_contract_path = installed
        .skill_contract_path
        .expect("project skill contract path");
    assert!(
        installed.plugin_skill_path.is_none(),
        "project skill install must not mirror SKILL.org into the Codex plugin"
    );

    assert_eq!(
        project_skill_contract_path,
        project_skill_path.with_file_name("SKILL.contract.org")
    );
    let project_contract =
        std::fs::read_to_string(&project_skill_contract_path).expect("read project contract");
    assert!(project_contract.contains(":REFER_ORG: "));
    assert!(project_contract.contains("org/skills/ASP_ORG.org#asp-org"));
    assert!(!project_contract.contains("./languages/org/skills/ASP_ORG.org"));
    assert!(!project_contract.contains("* stale user-layer contract"));
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
    write_plugin_manifest(&root);
    let project_skill_path = root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org");
    let plugin_contract_path = root
        .join("asp-codex-plugin")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org");
    write_stale_contract(&plugin_contract_path);

    let installed = install_agent_semantic_protocols_plugin_skill(
        &root,
        &test_activation(),
        &test_runtime_profiles(),
    )
    .unwrap();
    assert!(
        installed.skill_path.is_none(),
        "plugin skill install must not create project SKILL.org"
    );
    assert!(
        installed.skill_contract_path.is_none(),
        "plugin skill install must not create project SKILL.contract.org"
    );
    let plugin_skill_path = installed.plugin_skill_path.expect("plugin skill path");

    let plugin_skill = std::fs::read_to_string(&plugin_skill_path).expect("read plugin skill");
    let expected_asp_org = ".cache/agent-semantic-protocol/org/skills/ASP_ORG.org#asp-org";
    let expected_org_artifacts = ".cache/agent-semantic-protocol/artifacts/org";
    assert!(plugin_skill.contains("ASP Org Reference"));
    assert!(plugin_skill.contains("REFER_ORG"));
    assert!(plugin_skill.contains(expected_asp_org), "{plugin_skill}");
    assert!(
        plugin_skill.contains(expected_org_artifacts),
        "{plugin_skill}"
    );
    assert!(!plugin_skill.contains(&root.display().to_string()));
    assert!(
        !project_skill_path.exists(),
        "Codex plugin skill install must not write .agents/skills"
    );
    assert!(
        !plugin_skill_path
            .with_file_name("SKILL.contract.org")
            .exists(),
        "plugin directory must not contain SKILL.contract.org"
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
        "[providers.rust]\nbin = \"tools/rs-harness\"\n",
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
    assert!(config.contains("template = \"SKILL.org\""), "{config}");
    assert!(
        config.contains(
            "pluginSkill = \".codex/plugins/cache/asp-project/asp-codex-plugin/0.1.0/skills/agent-semantic-protocols/SKILL.org\""
        ),
        "{config}"
    );
    assert!(
        config
            .contains("aspOrg = \".cache/agent-semantic-protocol/org/skills/ASP_ORG.org#asp-org\""),
        "{config}"
    );
    assert!(
        config.contains("orgArtifacts = \".cache/agent-semantic-protocol/artifacts/org\""),
        "{config}"
    );
    assert!(config.contains("[hook.agentOrgArtifacts]"), "{config}");
    assert!(config.contains("enabled = true"), "{config}");
    assert!(config.contains("inactiveAfterMinutes = 30"), "{config}");
    assert!(
        config.contains("artifactsPath = \".cache/agent-semantic-protocol/artifacts/org\""),
        "{config}"
    );
    assert!(
        config
            .contains("entrySkillPath = \".cache/agent-semantic-protocol/org/skills/ASP_ORG.org\""),
        "{config}"
    );
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
