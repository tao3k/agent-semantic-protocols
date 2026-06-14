// Installed ASP Org skill rendering for `asp hook install`.

use agent_semantic_hook::{
    ActivatedProviderConfig, HookActivation, RuntimeProfiles, RuntimeProviderProfile,
};
use orgize::{
    Org,
    ast::{
        OrgContractEvaluationScope, OrgContractSeverity, evaluate_org_contract,
        parse_contract_reference, parse_contracts_from_document,
    },
};
use std::fs;
use std::path::{Path, PathBuf};

const AGENT_SEMANTIC_PROTOCOLS_SKILL_ORG: &str = include_str!("../../../../SKILL.org");
const AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_ORG: &str =
    include_str!("../../../../SKILL.contract.org");
const AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_REFERENCE: &str = "./SKILL.contract.org#asp.skill.v1";

pub(super) fn install_agent_semantic_protocols_skill(
    project_root: &Path,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<InstalledAgentSkillPaths, String> {
    let skill_path = default_agent_skill_path(project_root);
    let skill_contract_path = default_agent_skill_contract_path(project_root);
    let rendered_skill = render_agent_semantic_protocols_skill(activation, runtime_profiles)?;
    write_agent_skill_pair(&skill_path, &skill_contract_path, &rendered_skill)?;
    let plugin_paths = optional_plugin_skill_paths(project_root)
        .map(|(plugin_skill_path, plugin_skill_contract_path)| {
            write_agent_skill_pair(
                &plugin_skill_path,
                &plugin_skill_contract_path,
                &rendered_skill,
            )
            .map(|()| (plugin_skill_path, plugin_skill_contract_path))
        })
        .transpose()?;
    Ok(InstalledAgentSkillPaths {
        skill_path: Some(skill_path),
        skill_contract_path: Some(skill_contract_path),
        plugin_skill_path: plugin_paths
            .as_ref()
            .map(|(plugin_skill_path, _)| plugin_skill_path.clone()),
        plugin_skill_contract_path: plugin_paths
            .map(|(_, plugin_skill_contract_path)| plugin_skill_contract_path),
    })
}

pub(super) struct InstalledAgentSkillPaths {
    pub skill_path: Option<PathBuf>,
    pub skill_contract_path: Option<PathBuf>,
    pub plugin_skill_path: Option<PathBuf>,
    pub plugin_skill_contract_path: Option<PathBuf>,
}

fn default_agent_skill_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org")
}

fn default_agent_skill_contract_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.contract.org")
}

fn optional_plugin_skill_paths(project_root: &Path) -> Option<(PathBuf, PathBuf)> {
    let plugin_root = project_root.join("asp-codex-plugin");
    if !plugin_root
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
    {
        return None;
    }
    let skill_dir = plugin_root.join("skills").join("agent-semantic-protocols");
    Some((
        skill_dir.join("SKILL.org"),
        skill_dir.join("SKILL.contract.org"),
    ))
}

fn write_agent_skill_pair(
    skill_path: &Path,
    skill_contract_path: &Path,
    rendered_skill: &str,
) -> Result<(), String> {
    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(
        skill_contract_path,
        format!(
            "{}\n",
            AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_ORG.trim_end()
        ),
    )
    .map_err(|error| format!("failed to write {}: {error}", skill_contract_path.display()))?;
    fs::write(skill_path, format!("{}\n", rendered_skill.trim_end()))
        .map_err(|error| format!("failed to write {}: {error}", skill_path.display()))?;
    Ok(())
}

pub(super) fn render_agent_semantic_protocols_skill(
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    let rendered = replace_generated_block(
        AGENT_SEMANTIC_PROTOCOLS_SKILL_ORG,
        "notice",
        installed_skill_notice(),
    )?;
    let rendered = replace_generated_block(
        &rendered,
        "activation",
        &installed_activation_summary(activation),
    )?;
    replace_generated_block(
        &rendered,
        "providers",
        &installed_provider_contracts(activation, runtime_profiles),
    )
    .and_then(|rendered| {
        validate_agent_semantic_protocols_skill(&rendered)?;
        Ok(rendered)
    })
}

pub(super) fn validate_agent_semantic_protocols_skill(rendered_skill: &str) -> Result<(), String> {
    let contract_document = Org::parse(AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_ORG).document();
    let registry =
        parse_contracts_from_document(&contract_document, Some(Path::new("SKILL.contract.org")));
    let reference = parse_contract_reference(AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_REFERENCE);
    let contract = registry.resolve(&reference).ok_or_else(|| {
        format!(
            "SKILL.contract.org missing contract `{}`",
            AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_REFERENCE
        )
    })?;
    if contract.assertions.is_empty() {
        return Err(format!(
            "SKILL.contract.org contract `{}` has no assertions",
            contract.id
        ));
    }

    let skill_document = Org::parse(rendered_skill).document();
    let evaluation = evaluate_org_contract(
        &skill_document,
        contract,
        OrgContractEvaluationScope::document(),
    );
    let failures = evaluation
        .assertions
        .iter()
        .filter(|assertion| {
            assertion.status.is_failed() && assertion.severity == OrgContractSeverity::Error
        })
        .map(|assertion| {
            format!(
                "- assertion `{}` failed: expected {}, actual {}",
                assertion.assertion_id,
                assertion.expectation.expected_summary(),
                assertion.actual_count
            )
        })
        .collect::<Vec<_>>();

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "generated SKILL.org does not match Org contract `{}`:\n{}\nFix root SKILL.org, SKILL.contract.org, or the Rust skill renderer before installing.",
            evaluation.contract_id,
            failures.join("\n")
        ))
    }
}

pub(super) fn replace_generated_block(
    template: &str,
    name: &str,
    content: &str,
) -> Result<String, String> {
    let begin = format!("# BEGIN_ASP_GENERATED {name}");
    let end = format!("# END_ASP_GENERATED {name}");
    let Some(begin_start) = template.find(&begin) else {
        return Err(format!(
            "SKILL.org template missing generated block begin `{begin}`"
        ));
    };
    let begin_content_start = begin_start + begin.len();
    let Some(relative_end_start) = template[begin_content_start..].find(&end) else {
        return Err(format!(
            "SKILL.org template missing generated block end `{end}`"
        ));
    };
    let end_start = begin_content_start + relative_end_start;
    let mut rendered = String::with_capacity(template.len() + content.len());
    rendered.push_str(&template[..begin_content_start]);
    rendered.push('\n');
    rendered.push_str(content.trim_matches('\n'));
    rendered.push('\n');
    rendered.push_str(&template[end_start..]);
    Ok(rendered)
}

fn installed_skill_notice() -> &'static str {
    "#+begin_quote\nIMPORTANT: Generated by =asp hook install= from the repository root =SKILL.org= template, validated by =SKILL.contract.org=, and expanded from the current provider activation. Do not edit this installed copy. Edit root =SKILL.org=, =SKILL.contract.org=, or =asp.toml=, then rerun =asp hook install --client <client> .=.\n#+end_quote"
}

fn installed_activation_summary(activation: &HookActivation) -> String {
    [
        "Generated from =asp.toml=, =PATH=, and project =.bin= provider binaries.".to_string(),
        String::new(),
        "| Field | Value |".to_string(),
        "|-------+-------|".to_string(),
        format!(
            "| Runtime | ={}= |",
            org_table_cell(&activation.generated_by.runtime)
        ),
        format!(
            "| Version | ={}= |",
            org_table_cell(&activation.generated_by.version)
        ),
        format!("| Active provider count | {} |", activation.providers.len()),
        "| Active language list | See generated =Provider Contracts= subtrees. |".to_string(),
    ]
    .join("\n")
}

fn installed_provider_contracts(
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> String {
    let mut lines = vec![
        "Detected from provider binaries plus =asp.toml=; only activated languages are listed."
            .to_string(),
        String::new(),
    ];
    for provider in &activation.providers {
        let runtime_provider = runtime_profiles.providers.iter().find(|profile| {
            profile.manifest_id == provider.manifest_id
                && profile.language_id == provider.language_id
                && profile.provider_id == provider.provider_id
        });
        lines.extend(provider_contract_lines(
            &activation.project_root,
            provider,
            runtime_provider,
        ));
    }
    lines.join("\n")
}

fn provider_contract_lines(
    project_root: &str,
    provider: &ActivatedProviderConfig,
    runtime_provider: Option<&RuntimeProviderProfile>,
) -> Vec<String> {
    let command = provider_command_display(project_root, provider, runtime_provider);
    let document_provider = is_document_provider(provider);
    let mut lines = vec![
        format!("** {}", provider.language_id),
        ":PROPERTIES:".to_string(),
        format!(":LANGUAGE_ID: {}", provider.language_id),
        format!(":PROVIDER_ID: {}", provider.provider_id),
        format!(":BINARY: {}", provider.binary),
        format!(":EXECUTION: {}", provider.execution.as_str()),
        format!(":FACADE: asp {}", provider.language_id),
        format!(":COMMAND: {}", command),
        format!(":DOCUMENT_PROVIDER: {}", document_provider),
        ":ENABLED: true".to_string(),
        ":END:".to_string(),
        String::new(),
    ];
    if document_provider {
        lines.push(format!(
            "Use =asp {} guide .= before document element navigation.",
            provider.language_id
        ));
        lines.push(format!(
            "Use =asp {} query= for parser-owned document elements and metadata.",
            provider.language_id
        ));
    } else {
        lines.push(format!(
            "Use =asp {} guide .= before {}-specific exploration.",
            provider.language_id, provider.language_id
        ));
        lines.push(format!(
            "Use =asp {} search prime --workspace <workspace-root> --view seeds= before reading {} source.",
            provider.language_id, provider.language_id
        ));
    }
    lines.push(String::new());
    lines
}

fn is_document_provider(provider: &ActivatedProviderConfig) -> bool {
    matches!(provider.language_id.as_str(), "org" | "md") && provider.provider_id == "orgize"
}

fn provider_command_display(
    project_root: &str,
    provider: &ActivatedProviderConfig,
    runtime_provider: Option<&RuntimeProviderProfile>,
) -> String {
    let argv = runtime_provider
        .and_then(runtime_provider_command_argv)
        .or_else(|| {
            if provider.provider_command_prefix.is_empty() {
                None
            } else {
                Some(provider.provider_command_prefix.clone())
            }
        })
        .unwrap_or_else(|| vec![provider_display_binary(project_root, &provider.binary)]);
    argv.iter()
        .map(|arg| {
            shell_display_word(&installed_skill_display_arg(
                project_root,
                &provider.binary,
                arg,
            ))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn runtime_provider_command_argv(profile: &RuntimeProviderProfile) -> Option<Vec<String>> {
    if !profile.argv.is_empty() {
        return Some(profile.argv.clone());
    }
    profile
        .resolved_binary
        .as_ref()
        .map(|binary| vec![binary.clone()])
}

fn provider_display_binary(project_root: &str, binary: &str) -> String {
    let project_bin = Path::new(project_root).join(".bin").join(binary);
    if project_bin.is_file() {
        return format!(".bin/{binary}");
    }
    Path::new(binary)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(binary)
        .to_string()
}

fn installed_skill_display_arg(project_root: &str, provider_binary: &str, arg: &str) -> String {
    let path = Path::new(arg);
    if !path.is_absolute() {
        return arg.to_string();
    }
    let project_bin = Path::new(project_root).join(".bin").join(provider_binary);
    if project_bin.is_file()
        && path.file_name().and_then(|name| name.to_str()) == Some(provider_binary)
    {
        return format!(".bin/{provider_binary}");
    }
    if let Ok(relative) = path.strip_prefix(project_root) {
        return relative.to_string_lossy().replace('\\', "/");
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(arg)
        .to_string()
}

fn shell_display_word(arg: &str) -> String {
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':' | '='))
    {
        return arg.to_string();
    }
    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn org_table_cell(value: &str) -> String {
    value.replace('|', "\\|")
}
