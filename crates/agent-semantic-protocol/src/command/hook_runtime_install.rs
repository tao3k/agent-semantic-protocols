//! Installation owner for hook runtime and Codex plugin surfaces.

use super::hook_runtime_codex_plugin::{CodexPluginScope, install_codex_plugin_hooks};
use super::hook_runtime_skill::{
    PluginSkillScope, install_agent_semantic_protocols_agent_config,
    install_agent_semantic_protocols_plugin_skill, install_agent_semantic_protocols_skill,
};
use super::hook_runtime_subagent::{install_claude_resident_agents, subagent_model_arg};
use super::{
    display_path, ensure_supported_client, flag_value, optional_flag_value, project_root_arg,
};
use crate::command::{ProtocolBinaryInstallPlan, ensure_protocol_binary_installed};
use agent_semantic_hook::{
    claude_hook_block, default_claude_settings_path, load_or_refresh_default_activation,
    merge_claude_settings, remove_incompatible_hook_event_state, runtime_profiles_for_activation,
    validate_claude_settings_json,
};
use agent_semantic_runtime::project_runtime_state;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn run_install(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    if client == "codex" {
        return Err(
            "Codex plugin installation uses `asp install plugin --codex`; direct hook configuration is not a Codex surface."
                .to_string(),
        );
    }
    let project_root = project_root_arg(args)?;
    let subagent_model =
        subagent_model_arg(client, optional_flag_value(args, "--subagent-model")?)?;
    run_install_for_client(
        client,
        project_root,
        CodexPluginScope::Global,
        Some(subagent_model),
        "agent-install",
    )
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_install.rs"]
mod hook_runtime_install_tests;

#[derive(Debug)]
struct CodexPluginInstallRequest {
    project_root: PathBuf,
    scope: CodexPluginScope,
}

fn parse_codex_plugin_install_args(args: &[String]) -> Result<CodexPluginInstallRequest, String> {
    let matches = crate::command::cli_help::install_plugin_command()
        .disable_help_subcommand(true)
        .try_get_matches_from(
            std::iter::once("asp install plugin".to_string()).chain(args.iter().cloned()),
        )
        .map_err(|error| error.to_string())?;
    let project_root = fs::canonicalize(
        matches
            .get_one::<String>("project-root")
            .expect("clap supplies default project root"),
    )
    .map_err(|error| format!("failed to resolve plugin project root: {error}"))?;
    let codex_plugin_scope = if matches.get_flag("project") {
        CodexPluginScope::Project
    } else {
        CodexPluginScope::Global
    };
    Ok(CodexPluginInstallRequest {
        project_root,
        scope: codex_plugin_scope,
    })
}

pub(in crate::command) fn run_codex_plugin_install_args(args: &[String]) -> Result<(), String> {
    let request = parse_codex_plugin_install_args(args)?;
    run_install_for_client(
        "codex",
        request.project_root,
        request.scope,
        None,
        "plugin-install",
    )
}

fn run_install_for_client(
    client: &str,
    project_root: PathBuf,
    codex_plugin_scope: CodexPluginScope,
    subagent_model: Option<String>,
    receipt_label: &str,
) -> Result<(), String> {
    let mut timings = InstallTimings::new();
    ensure_supported_client(client)?;
    timings.mark("args");
    let runtime_state = project_runtime_state(&project_root)?;
    let _reconciliation_guard =
        crate::command::protocol_binary::ProtocolBinaryReconciliationGuard::acquire(
            &runtime_state.protocol_home,
        )?;
    let runtime_artifact_root = runtime_state.protocol_home.join("runtime/artifacts");
    let binary_install_plan = ProtocolBinaryInstallPlan::capture(runtime_artifact_root.clone())?;
    timings.mark("runtime-state");
    let org_state_sync =
        crate::command::org_capture::require_materialized_org_state(&project_root)?;
    timings.mark("org-state");
    let binary_install = ensure_protocol_binary_installed(&binary_install_plan)?;
    timings.mark("binary");
    let provider_binary_reconciliation =
        crate::command::install_provider_runtime_reconcile::reconcile_registered_provider_runtime_binaries(
            &runtime_state.runtime_bin_dir,
            &runtime_artifact_root,
            &runtime_state.provider_lock_dir,
        )?;
    timings.mark("provider-binaries");
    let activation_path = runtime_state.activation_path.clone();
    let activation_sync = load_or_refresh_default_activation(&activation_path, &project_root)?;
    let activation_status = activation_sync.status;
    let activation = activation_sync.activation;
    timings.mark("activation");
    let runtime_profiles = runtime_profiles_for_activation(&project_root, &activation)?;
    timings.mark("runtime-profiles");
    let client_config_path = runtime_state
        .protocol_home
        .join("hooks")
        .join("config.toml");
    let user_config_status = crate::command::managed_hook_config::materialize(&client_config_path)?;
    let installed_hook_config =
        agent_semantic_hook::load_client_config_for_project(&client_config_path, &project_root)?;
    agent_semantic_hook::validate_match_policy_rule_coverage(&installed_hook_config).map_err(
        |error| {
            format!(
                "managed Hook matcher rule-coverage gate failed before install publication: {error}"
            )
        },
    )?;
    timings.mark("user-config");
    let mut provider_artifacts = runtime_profiles
        .providers
        .iter()
        .map(|provider| {
            let binary = provider.resolved_binary.as_ref().ok_or_else(|| {
                format!(
                    "active provider has no resolved binary: language={} provider={} \
                     commandPrefix={:?} argv={:?} health={:?} reason={:?}",
                    provider.language_id,
                    provider.provider_id,
                    provider.provider_command_prefix,
                    provider.argv,
                    provider.health.status,
                    provider.health.reason,
                )
            })?;
            agent_semantic_hook::active_provider_artifact_input(
                &project_root,
                &provider.language_id,
                &provider.provider_id,
                PathBuf::from(binary),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    let provider_profiles_are_lattice_current = provider_artifacts.iter().all(|artifact| {
        artifact
            .materialized_path
            .starts_with(&runtime_state.runtime_bin_dir)
            && std::fs::symlink_metadata(&artifact.materialized_path)
                .is_ok_and(|metadata| metadata.file_type().is_file())
    });
    let client_config_digest =
        agent_semantic_content_identity::file_content_digest_v1(&client_config_path)?;
    provider_artifacts.push(agent_semantic_hook::ActiveAspArtifactInput {
        logical_path: "runtime/hooks/config.toml".to_string(),
        artifact_kind: agent_semantic_content_identity::active_artifact_merkle_v1::ActiveArtifactKindV1::RuntimeConfig,
        materialized_path: client_config_path.clone(),
        artifact_digest: client_config_digest,
    });
    remove_incompatible_hook_event_state(&project_root)?;
    timings.mark("event-state");
    let (config_path, extra_config_receipt) = match client {
        "codex" => {
            install_codex_plugin_hooks(&project_root, codex_plugin_scope, &binary_install.path)?
        }
        "claude" => install_claude_project_hooks(
            &project_root,
            subagent_model
                .as_deref()
                .ok_or_else(|| "Claude install requires a configured subagent model".to_owned())?,
        )?,
        _ => unreachable!("client support checked before install"),
    };
    timings.mark("project-hooks");
    let agent_config_receipt = if client == "codex" {
        "not-on-plugin-install".to_owned()
    } else {
        let agent_config_path = install_agent_semantic_protocols_agent_config(&project_root)?;
        timings.mark("agent-config");
        display_path(&project_root, &agent_config_path)
    };
    let installed_skill = Some(match client {
        "codex" => install_agent_semantic_protocols_plugin_skill(
            &project_root,
            match codex_plugin_scope {
                CodexPluginScope::Project => PluginSkillScope::Project,
                CodexPluginScope::Global => PluginSkillScope::Global,
            },
            &activation,
            &runtime_profiles,
        )?,
        "claude" => {
            install_agent_semantic_protocols_skill(&project_root, &activation, &runtime_profiles)?
        }
        _ => unreachable!("client support checked before install"),
    });
    timings.mark("skill");
    let plugin_cache_path = Option::<PathBuf>::None;
    if client == "codex" {
        let manual_project_cache = project_root.join(".codex/plugins/cache/asp-project");
        if manual_project_cache.exists() {
            fs::remove_dir_all(&manual_project_cache).map_err(|error| {
                format!(
                    "failed to remove retired manual Codex project plugin cache {}: {error}",
                    manual_project_cache.display()
                )
            })?;
        }
    }
    timings.mark("plugin-cache");
    let active_artifact = agent_semantic_hook::materialize_active_asp_artifact_receipt(
        &binary_install.path,
        &binary_install.artifact_digest,
        &activation_path,
        &provider_artifacts,
    )?;
    timings.mark("active-artifact-receipt");
    let retired_artifact_cleanup = if provider_binary_reconciliation.missing_count == 0
        && provider_profiles_are_lattice_current
    {
        Some(
            crate::command::protocol_binary::prune_runtime_binary_artifacts(
                &runtime_artifact_root,
            )?,
        )
    } else {
        None
    };
    timings.mark("retired-artifact-cleanup");
    let project_skill_receipt = installed_skill
        .as_ref()
        .and_then(|installed_skill| installed_skill.skill_path.as_ref())
        .map(|skill_path| format!(" skill={}", display_path(&project_root, skill_path)))
        .unwrap_or_default();
    let plugin_skill_path = installed_skill
        .as_ref()
        .and_then(|installed_skill| installed_skill.plugin_skill_path.clone());
    let plugin_skill_receipt = plugin_skill_path
        .as_ref()
        .map(|skill_path| format!(" pluginSkill={}", display_path(&project_root, skill_path)))
        .unwrap_or_default();
    let plugin_cache_receipt = plugin_cache_path
        .as_ref()
        .map(|cache_path| format!(" pluginCache={}", display_path(&project_root, cache_path)))
        .unwrap_or_default();
    let user_config_receipt = format!(
        " userConfig={} userConfigStatus={}",
        display_path(&project_root, &client_config_path),
        user_config_status.as_str()
    );
    println!(
        "[{receipt_label}] client={client} activation={} activationRuntime=derived activationSync={}{} activeArtifactReceipt={} activeArtifactRoot={} activeArtifactByteReads={} activeArtifactBytesRead={} activeArtifactReceiptWrites={} agentConfig={} orgState={} orgStateSync={} orgSourceIndex={} config={}{}{}{}{} binary=asp binaryPath={} binaryInstall={} binaryArtifactDigest={} binarySwitch=atomic providerBinariesMissing={} retiredArtifactCleanup={} retiredArtifactGenerationsRemoved={} mode=updated",
        display_path(&project_root, &activation_path),
        activation_status,
        user_config_receipt,
        display_path(&project_root, &active_artifact.receipt_path),
        active_artifact.receipt.artifact_root_digest().as_str(),
        active_artifact.artifact_byte_reads,
        active_artifact.artifact_bytes_read,
        active_artifact.receipt_writes,
        agent_config_receipt,
        display_path(&project_root, &runtime_state.protocol_home.join("org")),
        org_state_sync.status,
        org_state_sync.source_index_status,
        display_path(&project_root, &config_path),
        extra_config_receipt,
        project_skill_receipt,
        plugin_skill_receipt,
        plugin_cache_receipt,
        binary_install.path.display(),
        binary_install.status,
        binary_install.artifact_digest,
        provider_binary_reconciliation.missing_count,
        if retired_artifact_cleanup.is_some() {
            "complete"
        } else {
            "deferred"
        },
        retired_artifact_cleanup
            .as_ref()
            .map_or(0, |receipt| receipt.removed_generation_count),
    );
    Ok(())
}

struct InstallTimings {
    start: Option<Instant>,
    last: Option<Instant>,
}

impl InstallTimings {
    fn new() -> Self {
        if env::var_os("ASP_HOOK_INSTALL_TIMINGS").is_some() {
            let now = Instant::now();
            Self {
                start: Some(now),
                last: Some(now),
            }
        } else {
            Self {
                start: None,
                last: None,
            }
        }
    }

    fn mark(&mut self, label: &str) {
        let (Some(start), Some(last)) = (self.start, self.last) else {
            return;
        };
        let now = Instant::now();
        eprintln!(
            "[agent-install-timing] step={label} stepMs={:.3} totalMs={:.3}",
            (now - last).as_secs_f64() * 1000.0,
            (now - start).as_secs_f64() * 1000.0,
        );
        self.last = Some(now);
    }
}

fn install_claude_project_hooks(
    project_root: &Path,
    subagent_model: &str,
) -> Result<(PathBuf, String), String> {
    let settings_path = default_claude_settings_path(&project_root.to_string_lossy());
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let existing = fs::read_to_string(&settings_path).unwrap_or_default();
    if settings_path.is_file() {
        validate_claude_settings_json(&existing)
            .map_err(|error| format!("refusing to write invalid Claude settings JSON: {error}"))?;
    }
    let merged = merge_claude_settings(&existing, &claude_hook_block(project_root))?;
    validate_claude_settings_json(&merged)
        .map_err(|error| format!("refusing to write invalid Claude settings JSON: {error}"))?;
    fs::write(&settings_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", settings_path.display()))?;
    let subagent_path = install_claude_resident_agents(project_root, subagent_model)?;
    Ok((
        settings_path,
        format!(" subagent={}", display_path(project_root, &subagent_path)),
    ))
}
