// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Installation owner for hook runtime and Codex plugin surfaces.

use super::display_path;
use super::ensure_supported_client;
use super::flag_value;
use super::hook_runtime_skill::install_agent_semantic_protocols_agent_config;
use super::hook_runtime_skill::install_agent_semantic_protocols_skill;
use super::hook_runtime_subagent::install_claude_resident_agents;
use super::hook_runtime_subagent::subagent_model_arg;
use super::optional_flag_value;
use super::project_root_arg;
use crate::command::ProtocolBinaryInstallPlan;
use agent_semantic_hook::claude_hook_block;
use agent_semantic_hook::default_claude_settings_path;
use agent_semantic_hook::merge_claude_settings;
use agent_semantic_hook::remove_incompatible_hook_event_state;
use agent_semantic_hook::validate_claude_settings_json;
use agent_semantic_runtime::project_runtime_state;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Instant;

pub(super) async fn run_install(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    if client == "codex" {
        return Err(
            "Codex plugin publication uses `asp install plugin <status|publish> --codex [PROJECT_ROOT]`; direct hook configuration is not a Codex surface."
                .to_string(),
        );
    }
    let project_root = project_root_arg(args)?;
    let subagent_model =
        subagent_model_arg(client, optional_flag_value(args, "--subagent-model")?)?;
    run_install_for_client(client, project_root, Some(subagent_model), "agent-install").await
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_install.rs"]
mod hook_runtime_install_tests;

#[derive(Debug)]
struct CodexPluginInstallRequest {
    operation: CodexPluginInstallOperation,
    project_root: PathBuf,
    source_root_source: CodexPluginSourceRootSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexPluginInstallOperation {
    Status,
    Publish,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexPluginSourceRootSource {
    StateHomeDev,
    ExplicitOverride,
}

impl CodexPluginSourceRootSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::StateHomeDev => "state-home-dev",
            Self::ExplicitOverride => "explicit-override",
        }
    }
}

fn parse_codex_plugin_install_args(args: &[String]) -> Result<CodexPluginInstallRequest, String> {
    let matches = crate::command::cli_help::install_plugin_command()
        .disable_help_subcommand(true)
        .try_get_matches_from(
            std::iter::once("asp install plugin".to_string()).chain(args.iter().cloned()),
        )
        .map_err(|error| error.to_string())?;
    let (operation, operation_matches) = match matches.subcommand() {
        Some(("status", matches)) => (CodexPluginInstallOperation::Status, matches),
        Some(("publish", matches)) => (CodexPluginInstallOperation::Publish, matches),
        _ => {
            return Err(
                "asp install plugin requires an explicit `status` or `publish` subcommand"
                    .to_owned(),
            );
        }
    };
    let (project_root, source_root_source) = match operation_matches
        .get_one::<String>("project-root")
    {
        Some(project_root) => (
            fs::canonicalize(project_root)
                .map_err(|error| format!("failed to resolve plugin project root: {error}"))?,
            CodexPluginSourceRootSource::ExplicitOverride,
        ),
        None => {
            let state_home = agent_semantic_runtime::resolve_state_home()?;
            (
                agent_semantic_artifacts::runtime_artifact_catalog::load_runtime_developer_root(
                    &state_home,
                )?
                .ok_or_else(|| {
                    "global plugin publication requires ASP_STATE_HOME [dev].root or an explicit PROJECT_ROOT override"
                        .to_owned()
                })?,
                CodexPluginSourceRootSource::StateHomeDev,
            )
        }
    };
    Ok(CodexPluginInstallRequest {
        operation,
        project_root,
        source_root_source,
    })
}

pub(in crate::command) async fn run_codex_plugin_install_args(
    args: &[String],
) -> Result<(), String> {
    let request = parse_codex_plugin_install_args(args)?;
    match request.operation {
        CodexPluginInstallOperation::Status => {
            super::hook_runtime_codex_plugin::inspect_codex_plugin_publication(
                &request.project_root,
                request.source_root_source.as_str(),
            )
        }
        CodexPluginInstallOperation::Publish => {
            super::hook_runtime_codex_plugin::publish_codex_plugin_payload(
                &request.project_root,
                request.source_root_source.as_str(),
            )
        }
    }
}

async fn run_install_for_client(
    client: &str,
    project_root: PathBuf,
    subagent_model: Option<String>,
    receipt_label: &str,
) -> Result<(), String> {
    let mut timings = InstallTimings::new();
    ensure_supported_client(client)?;
    timings.mark("args");
    let runtime_state = project_runtime_state(&project_root)?;
    let state_layout = agent_semantic_artifacts::StateHomeLayout::new(&runtime_state.protocol_home);
    let runtime_artifact_root = state_layout
        .runtime_state()
        .artifacts()
        .root()
        .to_path_buf();
    let binary_install_plan = ProtocolBinaryInstallPlan::capture(runtime_artifact_root.clone())?;
    timings.mark("runtime-state");
    let org_state_sync =
        crate::command::org_capture::require_materialized_org_state(&project_root)?;
    timings.mark("org-state");
    let hook_runtime =
        crate::command::install_binary_config_admission::admit_embedded_hook_runtime_candidate(
            binary_install_plan.current_exe(),
        )
        .await?;
    let user_config_status =
        crate::command::install_binary_config_admission::publish_embedded_hook_config(
            &runtime_state.protocol_home,
        )?;
    timings.mark("hook-runtime");
    let binary_install =
        crate::command::protocol_binary::ensure_protocol_binary_bundle_installed_transaction(
            &binary_install_plan,
            &hook_runtime.source,
        )
        .await?;
    let bundle_digest = binary_install.bundle_digest.as_deref().ok_or_else(|| {
        "reasonKind=runtime-binary-bundle-receipt-incomplete missing bundleDigest".to_owned()
    })?;
    timings.mark("binary");
    let activation_path = runtime_state.activation_path.clone();
    let client_config_path =
        agent_semantic_artifacts::StateHomeLayout::new(&runtime_state.protocol_home)
            .control()
            .hook_client_config();
    let hook_binary_digest = hook_runtime.artifact_digest.to_string();
    timings.mark("user-config");
    remove_incompatible_hook_event_state(&project_root)?;
    timings.mark("event-state");
    let (config_path, extra_config_receipt) = install_claude_project_hooks(
        &project_root,
        subagent_model
            .as_deref()
            .ok_or_else(|| "Claude install requires a configured subagent model".to_owned())?,
    )?;
    timings.mark("project-hooks");
    let agent_config_path = install_agent_semantic_protocols_agent_config(&project_root)?;
    timings.mark("agent-config");
    let agent_config_receipt = display_path(&project_root, &agent_config_path);
    let installed_skill = Some(install_agent_semantic_protocols_skill(&project_root)?);
    timings.mark("skill");
    let plugin_cache_path = Option::<PathBuf>::None;
    timings.mark("plugin-cache");
    let active_artifact = agent_semantic_hook::materialize_active_asp_artifact_receipt(
        &binary_install.path,
        &binary_install.artifact_digest,
        &activation_path,
    )?;
    timings.mark("active-artifact-receipt");
    timings.mark("removed-artifact-cleanup");
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
        user_config_status
    );
    println!(
        "[{receipt_label}] client={client} activation={} activationRuntime=derived activationSync={}{} hookBinaryDigest={} bundleDigest={} activeArtifactRoot={} activeArtifactByteReads={} activeArtifactBytesRead={} activeArtifactReceiptWrites={} agentConfig={} orgState={} orgStateSync={} orgSourceIndex={} config={}{}{}{}{} binary=asp binaryPath={} binaryInstall={} binaryContentDigest={} digestAlgorithm=blake3-256 binarySwitch=atomic mode=updated",
        display_path(&project_root, &activation_path),
        "server-register",
        user_config_receipt,
        hook_binary_digest,
        bundle_digest,
        active_artifact.receipt.artifact_root_digest().as_str(),
        active_artifact.artifact_byte_reads,
        active_artifact.artifact_bytes_read,
        active_artifact.receipt_writes,
        agent_config_receipt,
        display_path(&project_root, &state_layout.resources().org()),
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
