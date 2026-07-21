//! Codex rollout JSONL session index parser.

use std::collections::{BTreeMap, BTreeSet};

use super::parse::parse_rollout_file_at_path;
use super::paths::{
    codex_rollout_paths_for_session_id, codex_sessions_dir, rg_rollout_paths_for_session_id,
};
use super::topology::{
    rollout_topology_lines, spawned_agent_ids_for_rollout, spawned_agent_paths_for_rollout,
    thread_spawn_child_session_ids_for_rollout,
};
use super::types::CodexRolloutSessionIndex;

/// Build a root-scoped Codex rollout session index.
pub fn codex_rollout_session_index(
    root_session_id: &str,
) -> Result<Option<CodexRolloutSessionIndex>, String> {
    codex_rollout_session_index_for_sessions(root_session_id, std::iter::empty::<&str>())
}

/// Build a root-scoped Codex rollout session index, including known child sessions.
pub fn codex_rollout_session_index_for_sessions<'a, I>(
    root_session_id: &str,
    session_ids: I,
) -> Result<Option<CodexRolloutSessionIndex>, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let sessions_dir = codex_sessions_dir()?;
    if !sessions_dir.is_dir() {
        return Ok(None);
    }
    let mut child_session_ids: BTreeSet<String> = session_ids
        .into_iter()
        .filter(|session_id| !session_id.is_empty() && *session_id != root_session_id)
        .map(str::to_string)
        .collect();
    let mut rollout_paths = match codex_rollout_paths_for_session_id(&sessions_dir, root_session_id)
    {
        Ok(paths) => paths,
        Err(error)
            if error.starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
        {
            Vec::new()
        }
        Err(error) => return Err(error),
    };
    let root_attributed_rollout_paths =
        rg_rollout_paths_for_session_id(&sessions_dir, root_session_id)?;
    rollout_paths.extend(root_attributed_rollout_paths.iter().cloned());
    let trace_rollout_index = std::env::var_os("ASP_CODEX_ROLLOUT_INDEX_TRACE").is_some();
    if trace_rollout_index {
        eprintln!(
            "{}",
            serde_json::json!({
                "trace": "codex-rollout-index-discovery",
                "rootSessionId": root_session_id,
                "rootAttributedPathCount": root_attributed_rollout_paths.len(),
                "rootAttributedPaths": root_attributed_rollout_paths,
            })
        );
    }
    let mut missing_rollout_by_session = BTreeMap::new();
    let mut host_agent_path_by_session = BTreeMap::new();
    let mut pending_session_ids = child_session_ids.iter().cloned().collect::<Vec<_>>();
    let mut processed_session_ids = BTreeSet::new();
    let mut pending_spawn_rollout_paths = rollout_paths.clone();
    let mut processed_spawn_rollout_paths = BTreeSet::new();

    while !(pending_session_ids.is_empty() && pending_spawn_rollout_paths.is_empty()) {
        while let Some(child_session_id) = pending_session_ids.pop() {
            if !processed_session_ids.insert(child_session_id.clone()) {
                continue;
            }
            match codex_rollout_paths_for_session_id(&sessions_dir, &child_session_id) {
                Ok(paths) => {
                    for path in paths {
                        if !rollout_paths.iter().any(|existing| existing == &path) {
                            pending_spawn_rollout_paths.push(path.clone());
                            rollout_paths.push(path);
                        }
                    }
                }
                Err(error)
                    if error
                        .starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
                {
                    missing_rollout_by_session.insert(child_session_id, error);
                }
                Err(error) => return Err(error),
            }
        }

        let Some(rollout_path) = pending_spawn_rollout_paths.pop() else {
            continue;
        };
        if !processed_spawn_rollout_paths.insert(rollout_path.clone()) {
            continue;
        }
        let topology_lines = rollout_topology_lines(&rollout_path)?;
        let mut spawned_child_session_ids =
            thread_spawn_child_session_ids_for_rollout(&topology_lines, root_session_id);
        spawned_child_session_ids.extend(spawned_agent_ids_for_rollout(&topology_lines));
        host_agent_path_by_session.extend(spawned_agent_paths_for_rollout(&topology_lines));
        for child_session_id in spawned_child_session_ids {
            if child_session_ids.insert(child_session_id.clone()) {
                pending_session_ids.push(child_session_id);
            }
        }
    }

    for child_session_id in child_session_ids {
        if processed_session_ids.contains(&child_session_id) {
            continue;
        }
        match codex_rollout_paths_for_session_id(&sessions_dir, &child_session_id) {
            Ok(paths) => rollout_paths.extend(paths),
            Err(error)
                if error.starts_with("Codex rollout invariant broken: no rollout JSONL found") =>
            {
                missing_rollout_by_session.insert(child_session_id, error);
            }
            Err(error) => return Err(error),
        }
    }
    rollout_paths.sort();
    rollout_paths.dedup();
    let mut records = Vec::new();
    let mut activity_by_session = BTreeMap::new();
    let mut scanned_rollout_count = 0_usize;
    let mut skipped_rollout_count = 0_usize;
    for rollout_path in rollout_paths {
        let Some((mut metadata, activity)) = parse_rollout_file_at_path(&rollout_path)? else {
            skipped_rollout_count += 1;
            continue;
        };
        if let Some(agent_path) = host_agent_path_by_session.get(&metadata.session_id) {
            metadata
                .agent_path
                .get_or_insert_with(|| agent_path.clone());
            metadata
                .parent_thread_id
                .get_or_insert_with(|| root_session_id.to_string());
            metadata
                .root_session_id
                .get_or_insert_with(|| root_session_id.to_string());
            if metadata.parent_thread_id.as_deref() == Some(root_session_id) {
                metadata
                    .thread_source
                    .get_or_insert_with(|| "subagent".to_string());
                metadata.spawn_depth.get_or_insert(1);
            }
        }
        let root_attribution_matches = metadata.root_session_id.as_deref() == Some(root_session_id)
            || metadata.session_id == root_session_id
            || metadata.parent_thread_id.as_deref() == Some(root_session_id);
        if trace_rollout_index {
            eprintln!(
                "{}",
                serde_json::json!({
                    "trace": "codex-rollout-index-candidate",
                    "rootSessionId": root_session_id,
                    "sessionId": metadata.session_id,
                    "actualRootSessionId": metadata.root_session_id,
                    "parentThreadId": metadata.parent_thread_id,
                    "threadSource": metadata.thread_source,
                    "agentRole": metadata.agent_role,
                    "agentPath": metadata.agent_path,
                    "spawnDepth": metadata.spawn_depth,
                    "model": metadata.model,
                    "reasoningEffort": metadata.reasoning_effort,
                    "rootAttributionMatches": root_attribution_matches,
                    "rolloutPath": metadata.rollout_path,
                })
            );
        }
        if !root_attribution_matches {
            skipped_rollout_count += 1;
            continue;
        }
        scanned_rollout_count += 1;
        activity_by_session.insert(metadata.session_id.clone(), activity);
        if metadata.session_id == root_session_id {
            continue;
        }
        records.push(metadata);
    }
    if records.is_empty() {
        return Ok(None);
    }
    Ok(Some(CodexRolloutSessionIndex {
        root_session_id: root_session_id.to_string(),
        sessions_dir,
        scanned_rollout_count,
        skipped_rollout_count,
        records,
        activity_by_session,
        missing_rollout_by_session,
    }))
}
