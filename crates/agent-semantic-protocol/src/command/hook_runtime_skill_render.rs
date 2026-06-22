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
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

const AGENT_SEMANTIC_PROTOCOLS_SKILL_ORG: &str = include_str!("../../../../SKILL.org");
const AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_ORG: &str =
    include_str!("../../../../SKILL.contract.org");
const AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_REFERENCE: &str = "./SKILL.contract.org#asp.skill.v1";

pub(crate) fn render_agent_semantic_protocols_skill_contract(
    contract_path: &Path,
    org_state_skill_path: &Path,
) -> Result<String, String> {
    let refer_org = refer_org_from_contract_path(contract_path, org_state_skill_path)?;
    let refer_org_line = format!(":REFER_ORG: {refer_org}");
    let mut replaced = false;
    let rendered = AGENT_SEMANTIC_PROTOCOLS_SKILL_CONTRACT_ORG
        .lines()
        .map(|line| {
            if line.starts_with(":REFER_ORG:") {
                replaced = true;
                refer_org_line.as_str()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if replaced {
        Ok(rendered)
    } else {
        Err("SKILL.contract.org template missing :REFER_ORG: property".to_string())
    }
}

fn refer_org_from_contract_path(
    contract_path: &Path,
    org_state_skill_path: &Path,
) -> Result<String, String> {
    let contract_dir = contract_path.parent().ok_or_else(|| {
        format!(
            "failed to compute REFER_ORG for contract path without parent: {}",
            contract_path.display()
        )
    })?;
    let relative_path = relative_path_between(contract_dir, org_state_skill_path);
    Ok(format!(
        "{}#asp-org",
        relative_path.to_string_lossy().replace('\\', "/")
    ))
}

fn relative_path_between(from_dir: &Path, target: &Path) -> PathBuf {
    let from_components = normalized_path_components(from_dir);
    let target_components = normalized_path_components(target);
    let common_prefix_len = from_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative_path = PathBuf::new();
    for _ in common_prefix_len..from_components.len() {
        relative_path.push("..");
    }
    for component in &target_components[common_prefix_len..] {
        relative_path.push(component);
    }
    if relative_path.as_os_str().is_empty() {
        relative_path.push(".");
    }
    relative_path
}

fn normalized_path_components(path: &Path) -> Vec<OsString> {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component.to_os_string()),
            Component::ParentDir => Some(OsString::from("..")),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => None,
        })
        .collect()
}

pub(crate) fn render_agent_semantic_protocols_installed_skill(
    project_root: &Path,
    org_state_skill_path: &Path,
    org_artifacts_path: &Path,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    render_agent_semantic_protocols_skill_with_reference(
        installed_asp_org_reference(project_root, org_state_skill_path, org_artifacts_path),
        activation,
        runtime_profiles,
    )
}

pub(crate) fn render_agent_semantic_protocols_plugin_skill(
    project_root: &Path,
    org_state_skill_path: &Path,
    org_artifacts_path: &Path,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    render_agent_semantic_protocols_skill_with_reference(
        plugin_asp_org_reference(project_root, org_state_skill_path, org_artifacts_path),
        activation,
        runtime_profiles,
    )
}

fn render_agent_semantic_protocols_skill_with_reference(
    asp_org_reference: String,
    activation: &HookActivation,
    runtime_profiles: &RuntimeProfiles,
) -> Result<String, String> {
    let rendered = replace_generated_block(
        AGENT_SEMANTIC_PROTOCOLS_SKILL_ORG,
        "notice",
        installed_skill_notice(),
    )?;
    let rendered = replace_generated_block(&rendered, "asp-org-reference", &asp_org_reference)?;
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

fn plugin_asp_org_reference(
    project_root: &Path,
    org_state_skill_path: &Path,
    org_artifacts_path: &Path,
) -> String {
    let asp_org_path = display_project_path(project_root, org_state_skill_path);
    let artifacts_path = display_project_path(project_root, org_artifacts_path);
    asp_org_reference_section(
        &format!("{asp_org_path}#asp-org"),
        &asp_org_path,
        &artifacts_path,
    )
}

fn installed_asp_org_reference(
    project_root: &Path,
    org_state_skill_path: &Path,
    org_artifacts_path: &Path,
) -> String {
    let asp_org_path = display_project_path(project_root, org_state_skill_path);
    let artifacts_path = display_project_path(project_root, org_artifacts_path);
    asp_org_reference_section(
        &format!("{asp_org_path}#asp-org"),
        &asp_org_path,
        &artifacts_path,
    )
}

fn asp_org_reference_section(
    refer_org: &str,
    asp_org_path: &str,
    org_artifacts_path: &str,
) -> String {
    [
        "The ASP Org operational skill is materialized in project state, not inside the Codex plugin directory.".to_string(),
        "Use this reference when durable Org planning, contracts, templates, or flow state are needed.".to_string(),
        "Resolve project-local state paths through =asp paths= before opening Org artifacts from a different working directory.".to_string(),
        String::new(),
        "| Field | Value |".to_string(),
        "|-------+-------|".to_string(),
        "| Project root | =asp paths --get projectRoot= |".to_string(),
        format!("| REFER_ORG | ={refer_org}= |"),
        "| ASP Org skill | =asp paths --get orgStateSkill= |".to_string(),
        "| Org artifacts | =asp paths --get orgArtifacts= |".to_string(),
        format!("| Project-relative ASP Org skill | ={asp_org_path}= |"),
        format!("| Project-relative Org artifacts | ={org_artifacts_path}= |"),
        "| Missing state | Run =asp org capture init= from the project root. |".to_string(),
    ]
    .join("\n")
}

fn display_project_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn validate_agent_semantic_protocols_skill(rendered_skill: &str) -> Result<(), String> {
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

pub(crate) fn replace_generated_block(
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
    "#+begin_quote\nIMPORTANT: Generated from the repository root =SKILL.org= template, validated by =SKILL.contract.org=, and expanded from the current provider activation. Do not edit this installed copy. Edit root =SKILL.org=, =SKILL.contract.org=, or =.agents/asp.toml=, then rerun =asp install plugin --codex .= for Codex or =asp install hook --client claude .= for Claude.\n#+end_quote"
}

fn installed_activation_summary(activation: &HookActivation) -> String {
    [
        "Generated from =.agents/asp.toml=, =PATH=, and project =.bin= provider binaries."
            .to_string(),
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
        "Detected from provider binaries plus =.agents/asp.toml=; only activated languages are listed."
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
            "Use =asp {} search prime --workspace <workspace-root> --view seeds= only when the {} owner map is unknown; exact selectors, owners, symbols, dependencies, or hook frontiers should go straight to the provider-owned query, owner, finder, guide, or dependency route.",
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
