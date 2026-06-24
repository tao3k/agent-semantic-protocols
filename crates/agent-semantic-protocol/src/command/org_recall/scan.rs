use super::model::OrgPlanCandidate;
use orgize::{
    agent::{DocumentWalkConfig, OrgMemorySearchOptions, query_org_memory_records},
    ast::MemoryRecordState,
};
use std::{collections::BTreeMap, path::Path};

pub(super) fn scan_org_plan_candidates(
    artifacts_root: &Path,
    archive_dir: &str,
    include_done: bool,
    _org_query_bin: &str,
) -> Result<Vec<OrgPlanCandidate>, String> {
    let mut walk_config = DocumentWalkConfig::default();
    for dir in ["archive", "archives", archive_dir] {
        if !walk_config.ignore_dirs.iter().any(|ignored| ignored == dir) {
            walk_config.ignore_dirs.push(dir.to_string());
        }
    }
    let mut options = OrgMemorySearchOptions::plan_ledgers();
    options.include_closed = include_done;
    let records = query_org_memory_records(artifacts_root, &walk_config, &options)?;
    Ok(records
        .into_iter()
        .filter(|record| include_done || record.state != MemoryRecordState::Closed)
        .map(|record| OrgPlanCandidate {
            path: record.path,
            title: record.title,
            todo: record.todo.unwrap_or_default(),
            todo_type: todo_type(record.state).to_string(),
            properties: record.properties.into_iter().collect::<BTreeMap<_, _>>(),
            mtime: record.mtime,
        })
        .collect())
}

fn todo_type(state: MemoryRecordState) -> &'static str {
    match state {
        MemoryRecordState::Closed => "Done",
        MemoryRecordState::Current => "Todo",
        MemoryRecordState::Archived => "Done",
        MemoryRecordState::Background => "",
    }
}
