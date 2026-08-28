use super::{checkpoint::CheckpointSyncResult, memory::MemoryRankedPlans, model::RankedOrgPlan};
use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
};

pub(super) fn print_text_report(
    root: &Path,
    archive_dir: &str,
    ranked: &MemoryRankedPlans,
    session: Option<&str>,
    checkpoint_sync: Option<&CheckpointSyncResult>,
) {
    println!(
        "[org-recall-plans] owner=rust session={} hits={}",
        field(session.unwrap_or_default()),
        ranked.plans.len()
    );
    if let Some(item) = ranked.plans.first() {
        let row = plan_row(
            root,
            archive_dir,
            item,
            session,
            checkpoint_sync,
            ranked.plans.len().saturating_sub(1),
        );
        let receipt = &row["selectionReceipt"];
        println!(
            "|next action={} rank=1 plan={} title={} status={} nextAction={}",
            field(row["action"].as_str().unwrap_or_default()),
            field(row["id"].as_str().unwrap_or_default()),
            field(row["title"].as_str().unwrap_or_default()),
            field(row["status"].as_str().unwrap_or_default()),
            field(row["nextAction"].as_str().unwrap_or_default())
        );
        println!(
            "|why session={} sessionMatched={} selectedBy={} taskHits={} checkpointHits={} alternatives={} memoryTransport={}",
            field(receipt["session"].as_str().unwrap_or_default()),
            receipt["sessionMatched"].as_bool().unwrap_or(false),
            field(receipt["selectedBy"].as_str().unwrap_or_default()),
            receipt["taskHits"].as_u64().unwrap_or_default(),
            receipt["checkpointHits"].as_u64().unwrap_or_default(),
            receipt["alternatives"].as_u64().unwrap_or_default(),
            field(ranked.transport)
        );
        println!(
            "|query action={} command={}",
            field(row["action"].as_str().unwrap_or_default()),
            field(row["actionCommand"].as_str().unwrap_or_default())
        );
        for (task_index, task) in row["taskCandidates"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            println!(
                "|evidence rank={} kind={} status={} section={} title={} sourceLocator={}",
                task_index + 1,
                field(task["kind"].as_str().unwrap_or_default()),
                field(task["status"].as_str().unwrap_or_default()),
                field(task["section"].as_str().unwrap_or_default()),
                field(task["title"].as_str().unwrap_or_default()),
                field(task["sourceLocator"].as_str().unwrap_or_default())
            );
        }
    }

    for (index, item) in ranked.plans.iter().enumerate() {
        let row = plan_row(
            root,
            archive_dir,
            item,
            session,
            checkpoint_sync,
            ranked.plans.len().saturating_sub(1),
        );
        println!(
            "|candidate rank={} action={} plan={} title={} status={} nextAction={}",
            index + 1,
            field(row["action"].as_str().unwrap_or_default()),
            field(row["id"].as_str().unwrap_or_default()),
            field(row["title"].as_str().unwrap_or_default()),
            field(row["status"].as_str().unwrap_or_default()),
            field(row["nextAction"].as_str().unwrap_or_default())
        );
    }
    if let Some(sync) = checkpoint_sync {
        println!(
            "|checkpoint-sync checkpoints={} skippedSessionPlans={} memoryTransport={}",
            sync.checkpoints,
            sync.skipped_session_plans,
            field(sync.transport)
        );
    }
}

pub(super) fn print_json_report(
    root: &Path,
    archive_dir: &str,
    ranked: &MemoryRankedPlans,
    session: Option<&str>,
    checkpoint_sync: Option<&CheckpointSyncResult>,
) -> Result<(), String> {
    let mut payload = json!({
        "schemaId": "agent.semantic-protocols.org-plan-recall",
        "schemaVersion": "1",
        "owner": "rust",
        "memoryEngine": "asp-memory-engine",
        "ranker": "memory-engine",
        "memoryTransport": ranked.transport,
        "session": session,
        "artifactsRoot": root.display().to_string(),
        "plans": ranked.plans.iter().map(|item| {
            plan_row(
                root,
                archive_dir,
                item,
                session,
                checkpoint_sync,
                ranked.plans.len().saturating_sub(1),
            )
        }).collect::<Vec<_>>(),
    });
    if let Some(sync) = checkpoint_sync {
        payload["checkpointSync"] = json!({
            "checkpoints": sync.checkpoints,
            "skippedSessionPlans": sync.skipped_session_plans,
            "memoryTransport": sync.transport,
        });
    }
    println!(
        "{}",
        serde_json::to_string(&payload)
            .map_err(|error| format!("failed to encode org recall JSON: {error}"))?
    );
    Ok(())
}

fn plan_row(
    root: &Path,
    archive_dir: &str,
    ranked: &RankedOrgPlan,
    session: Option<&str>,
    checkpoint_sync: Option<&CheckpointSyncResult>,
    alternatives: usize,
) -> serde_json::Value {
    let candidate = &ranked.candidate;
    let path = display_path(&candidate.path);
    let resume_terms = [
        candidate.plan_id(),
        "recovery".to_string(),
        "evidence".to_string(),
        "next-action".to_string(),
    ];
    let resume_command = format!("asp org query {}", resume_terms.join(" "));
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
            "unfinished plan ranked by session context, memory, and recency",
            resume_command.clone(),
        )
    };
    let task_candidates = candidate
        .task_candidates
        .iter()
        .map(|task| task_candidate_row(&candidate.path, task))
        .collect::<Vec<_>>();
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
        "contextScore": ranked.context_score,
        "memoryScore": ranked.memory_score,
        "recencyScore": ranked.recency_score,
        "resumeCommand": resume_command,
        "selectionReceipt": selection_receipt(ranked, session, checkpoint_sync, alternatives),
        "taskCandidates": task_candidates,
        "artifactsRoot": root.display().to_string(),
    })
}

fn selection_receipt(
    ranked: &RankedOrgPlan,
    session: Option<&str>,
    checkpoint_sync: Option<&CheckpointSyncResult>,
    alternatives: usize,
) -> serde_json::Value {
    let candidate = &ranked.candidate;
    let plan_session = candidate
        .properties
        .get("SESSION_ID")
        .map(String::as_str)
        .unwrap_or_default();
    let session_match_property = session
        .filter(|session| !session.is_empty())
        .and_then(|session| candidate.session_match_property(session))
        .unwrap_or_default();
    let session_matched = !session_match_property.is_empty();
    json!({
        "session": session.unwrap_or_default(),
        "planSession": plan_session,
        "sessionMatched": session_matched,
        "sessionMatchProperty": session_match_property,
        "selectedBy": selected_by(ranked, session_matched),
        "taskHits": candidate.task_candidates.len(),
        "checkpointHits": checkpoint_sync.map(|sync| sync.checkpoints).unwrap_or_default(),
        "alternatives": alternatives,
    })
}

fn selected_by(ranked: &RankedOrgPlan, session_matched: bool) -> String {
    let mut sources = Vec::new();
    if session_matched || ranked.context_score > 0.0 {
        sources.push("session");
    }
    sources.push("memory-engine");
    if !ranked.candidate.task_candidates.is_empty() {
        sources.push("org-graph");
    }
    if ranked.recency_score > 0.0 {
        sources.push("recency");
    }
    sources.join("+")
}

fn task_candidate_row(path: &Path, task: &super::model::OrgTaskCandidate) -> serde_json::Value {
    let path = display_path(path);
    let source_locator = task
        .source_line
        .map(|line| format!("{}:{line}-{line}", path.display()))
        .unwrap_or_else(|| path.display().to_string());
    json!({
        "kind": task.kind.as_str(),
        "status": task.status.as_str(),
        "title": task.title.as_str(),
        "section": task.section.as_deref(),
        "sourceLine": task.source_line,
        "sourceLocator": source_locator,
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
