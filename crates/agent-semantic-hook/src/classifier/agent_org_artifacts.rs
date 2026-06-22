//! Agent Org artifact recovery hints for hook deny/block decisions.

use serde_json::Value;
use std::path::Path;

use crate::{
    ClientHookConfig, DecisionKind, HookDecision,
    hook_config_agent_org::AgentOrgArtifactsArchiveWarning,
};

pub(super) fn with_agent_org_artifact_recovery(
    mut decision: HookDecision,
    config: &ClientHookConfig,
    project_root: &str,
) -> HookDecision {
    decision = with_agent_org_artifact_archive_warning(decision, config, project_root);
    if !matches!(decision.decision, DecisionKind::Deny | DecisionKind::Block) {
        return decision;
    }
    let Some(recovery) = config.agent_org_artifacts_recovery(Path::new(project_root)) else {
        return decision;
    };

    if !decision.message.contains("ASP Org Artifact Entry:") {
        decision.message.push_str("\n\n");
        decision.message.push_str(&format!(
            "ASP Org Artifact Entry: no recent contract-bound Org artifact was observed under `{}` within the last {} minutes. Read @{} before continuing, render a checked entry with `{}`, then write the returned org-entry under `{}`.",
            recovery.artifacts_path,
            recovery.inactive_after_minutes,
            recovery.entry_skill_path,
            recovery.capture_contract_command,
            recovery.artifacts_path
        ));
    }
    decision.fields.insert(
        "agentOrgArtifactsStatus".to_string(),
        Value::String("missing-contract-bound-artifact".to_string()),
    );
    decision.fields.insert(
        "agentOrgArtifactsPath".to_string(),
        Value::String(recovery.artifacts_path),
    );
    decision.fields.insert(
        "agentOrgArtifactsEntrySkillPath".to_string(),
        Value::String(recovery.entry_skill_path),
    );
    decision.fields.insert(
        "agentOrgArtifactsInactiveAfterMinutes".to_string(),
        Value::from(recovery.inactive_after_minutes),
    );
    decision.fields.insert(
        "agentOrgCaptureContractCommand".to_string(),
        Value::String(recovery.capture_contract_command),
    );
    decision
}

fn with_agent_org_artifact_archive_warning(
    mut decision: HookDecision,
    config: &ClientHookConfig,
    project_root: &str,
) -> HookDecision {
    let Some(warning) = config.agent_org_artifacts_archive_warning(Path::new(project_root)) else {
        return decision;
    };
    if !decision.message.contains("ASP Org Archive Warning:") {
        if !decision.message.is_empty() {
            decision.message.push_str("\n\n");
        }
        decision
            .message
            .push_str(&archive_warning_message(&warning));
    }
    decision.fields.insert(
        "agentOrgArtifactsArchiveWarning".to_string(),
        Value::String("unarchived-done".to_string()),
    );
    decision.fields.insert(
        "agentOrgArtifactsActiveOrgFileCount".to_string(),
        Value::from(warning.active_org_file_count),
    );
    decision.fields.insert(
        "agentOrgArtifactsActiveOrgFileThreshold".to_string(),
        Value::from(warning.active_org_file_threshold),
    );
    decision.fields.insert(
        "agentOrgArtifactsPath".to_string(),
        Value::String(warning.artifacts_path.clone()),
    );
    decision.fields.insert(
        "agentOrgArtifactsArchiveDir".to_string(),
        Value::String(warning.archives_dir.clone()),
    );
    decision.fields.insert(
        "agentOrgArtifactsArchiveQueryCommand".to_string(),
        Value::String(archive_query_command(&warning)),
    );
    decision.fields.insert(
        "agentOrgArtifactsUnarchivedDoneFiles".to_string(),
        Value::Array(
            warning
                .done_org_files
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    decision
}

fn archive_warning_message(warning: &AgentOrgArtifactsArchiveWarning) -> String {
    let files = warning.done_org_files.join(", ");
    let command = archive_query_command(warning);
    format!(
        "ASP Org Archive Warning: `{}` contains {} active .org files, above threshold {}; DONE records not under `{}` should be archived after selector review: {}.\nRun `{}` to list the unarchived DONE tasks from the Org parser.",
        warning.artifacts_path,
        warning.active_org_file_count,
        warning.active_org_file_threshold,
        warning.archives_dir,
        files,
        command
    )
}

fn archive_query_command(warning: &AgentOrgArtifactsArchiveWarning) -> String {
    format!(
        "asp org query --kind task --field todo=DONE --exclude-dir {} --workspace {} --content",
        shell_arg(&warning.archives_dir),
        shell_arg(&warning.artifacts_path)
    )
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
