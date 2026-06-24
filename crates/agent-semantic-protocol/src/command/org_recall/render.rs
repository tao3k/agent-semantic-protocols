use super::model::RankedOrgPlan;
use serde_json::json;
use std::{
    env,
    path::{Path, PathBuf},
};

pub(super) fn print_text_report(root: &Path, archive_dir: &str, ranked: &[RankedOrgPlan]) {
    println!(
        "[org-recall-plans] owner=rust memoryEngine=asp-memory-engine ranker=memory-engine artifactsRoot={} hits={}",
        field(&root.display().to_string()),
        ranked.len()
    );
    for (index, item) in ranked.iter().enumerate() {
        let row = plan_row(root, item);
        println!(
            "|plan rank={} score={:.6} textScore={:.6} memoryScore={:.6} todo={} title={} id={} path={} objective={} nextAction={} resumeCommand={}",
            index + 1,
            item.score,
            item.text_score,
            item.memory_score,
            field(row["todo"].as_str().unwrap_or_default()),
            field(row["title"].as_str().unwrap_or_default()),
            field(row["id"].as_str().unwrap_or_default()),
            field(row["path"].as_str().unwrap_or_default()),
            field(row["objective"].as_str().unwrap_or_default()),
            field(row["nextAction"].as_str().unwrap_or_default()),
            field(row["resumeCommand"].as_str().unwrap_or_default())
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

pub(super) fn print_json_report(root: &Path, ranked: &[RankedOrgPlan]) -> Result<(), String> {
    let payload = json!({
        "schemaId": "agent.semantic-protocols.org-plan-recall",
        "schemaVersion": "1",
        "owner": "rust",
        "memoryEngine": "asp-memory-engine",
        "ranker": "memory-engine",
        "artifactsRoot": root.display().to_string(),
        "plans": ranked.iter().map(|item| plan_row(root, item)).collect::<Vec<_>>(),
    });
    println!(
        "{}",
        serde_json::to_string(&payload)
            .map_err(|error| format!("failed to encode org recall JSON: {error}"))?
    );
    Ok(())
}

fn plan_row(root: &Path, ranked: &RankedOrgPlan) -> serde_json::Value {
    let candidate = &ranked.candidate;
    let path = display_path(&candidate.path);
    json!({
        "path": path.display().to_string(),
        "title": candidate.display_title(),
        "todo": candidate.todo,
        "id": candidate.plan_id(),
        "objective": candidate.objective(),
        "nextAction": candidate.next_action(),
        "recoveryRef": candidate.recovery_ref(),
        "score": ranked.score,
        "textScore": ranked.text_score,
        "memoryScore": ranked.memory_score,
        "recencyScore": ranked.recency_score,
        "intentScore": ranked.intent_score,
        "resumeCommand": format!(
            "asp org query --term {} --workspace {} --content",
            candidate.plan_id(),
            path.display()
        ),
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
