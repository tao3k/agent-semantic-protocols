use super::{memory::MemoryRankedPlans, model::RankedOrgPlan};
use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
};

pub(super) fn print_text_report(root: &Path, archive_dir: &str, ranked: &MemoryRankedPlans) {
    println!(
        "[org-recall-plans] owner=rust memoryEngine=asp-memory-engine ranker=memory-engine memoryTransport={} artifactsRoot={} hits={}",
        ranked.transport,
        field(&root.display().to_string()),
        ranked.plans.len()
    );
    for (index, item) in ranked.plans.iter().enumerate() {
        let row = plan_row(root, archive_dir, item);
        println!(
            "|plan rank={} score={:.6} textScore={:.6} memoryScore={:.6} recencyScore={:.6} intentScore={:.6} todo={} status={} evidenceStatus={} reviewStatus={} reflectionComplete={} title={} id={} path={} objective={} nextAction={} resumeCommand={}",
            index + 1,
            item.score,
            item.text_score,
            item.memory_score,
            item.recency_score,
            item.intent_score,
            field(row["todo"].as_str().unwrap_or_default()),
            field(row["status"].as_str().unwrap_or_default()),
            field(row["evidenceStatus"].as_str().unwrap_or_default()),
            field(row["reviewStatus"].as_str().unwrap_or_default()),
            field(row["reflectionComplete"].as_str().unwrap_or_default()),
            field(row["title"].as_str().unwrap_or_default()),
            field(row["id"].as_str().unwrap_or_default()),
            field(row["path"].as_str().unwrap_or_default()),
            field(row["objective"].as_str().unwrap_or_default()),
            field(row["nextAction"].as_str().unwrap_or_default()),
            field(row["resumeCommand"].as_str().unwrap_or_default())
        );
        println!(
            "|plan-action rank={} action={} reason={} command={}",
            index + 1,
            field(row["action"].as_str().unwrap_or_default()),
            field(row["actionReason"].as_str().unwrap_or_default()),
            field(row["actionCommand"].as_str().unwrap_or_default())
        );
    }
    if let Some(item) = ranked.plans.first() {
        let row = plan_row(root, archive_dir, item);
        println!(
            "|next recommendedAction={} rank=1 command={}",
            field(row["action"].as_str().unwrap_or_default()),
            field(row["actionCommand"].as_str().unwrap_or_default())
        );
    }
    println!(
        "|next archiveCommand={}",
        field(&format!(
            "asp org archive done --artifacts-root {} --archive-dir {}",
            root.display(),
            archive_dir
        ))
    );
}

pub(super) fn print_json_report(
    root: &Path,
    archive_dir: &str,
    ranked: &MemoryRankedPlans,
) -> Result<(), String> {
    let payload = json!({
        "schemaId": "agent.semantic-protocols.org-plan-recall",
        "schemaVersion": "1",
        "owner": "rust",
        "memoryEngine": "asp-memory-engine",
        "ranker": "memory-engine",
        "memoryTransport": ranked.transport,
        "artifactsRoot": root.display().to_string(),
        "plans": ranked.plans.iter().map(|item| plan_row(root, archive_dir, item)).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string(&payload)
            .map_err(|error| format!("failed to encode org recall JSON: {error}"))?
    );
    Ok(())
}

fn plan_row(root: &Path, archive_dir: &str, ranked: &RankedOrgPlan) -> serde_json::Value {
    let candidate = &ranked.candidate;
    let path = display_path(&candidate.path);
    let resume_command = format!(
        "asp org query --term {} --workspace {} --content",
        candidate.plan_id(),
        path.display()
    );
    let archive_command = format!(
        "asp org archive done --artifacts-root {} --archive-dir {}",
        root.display(),
        archive_dir
    );
    let (action, action_reason, action_command) = if candidate.is_archive_ready() {
        (
            "archive",
            "plan is DONE or archive-ready with completed reflection",
            archive_command.clone(),
        )
    } else if candidate.needs_reflection_completion() {
        (
            "complete-reflection",
            "reflection answers are required before archive",
            resume_command.clone(),
        )
    } else {
        (
            "resume",
            "unfinished plan ranked by memory, intent, and recency",
            resume_command.clone(),
        )
    };
    json!({
        "path": path.display().to_string(),
        "title": candidate.display_title(),
        "todo": candidate.todo,
        "status": candidate.status(),
        "evidenceStatus": candidate.evidence_status(),
        "reviewStatus": candidate.review_status(),
        "reflectionComplete": if candidate.reflection_complete { "true" } else { "false" },
        "id": candidate.plan_id(),
        "objective": candidate.objective(),
        "nextAction": candidate.next_action(),
        "recoveryRef": candidate.recovery_ref(),
        "action": action,
        "actionReason": action_reason,
        "actionCommand": action_command,
        "score": ranked.score,
        "textScore": ranked.text_score,
        "memoryScore": ranked.memory_score,
        "recencyScore": ranked.recency_score,
        "intentScore": ranked.intent_score,
        "resumeCommand": resume_command,
        "artifactsRoot": root.display().to_string(),
    })
}

fn display_path(path: &Path) -> PathBuf {
    env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
}

fn field(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ");
    format!("\"{escaped}\"")
}
