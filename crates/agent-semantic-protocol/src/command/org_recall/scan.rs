use super::model::OrgPlanCandidate;
use orgize::{
    agent::{DocumentWalkConfig, OrgMemorySearchOptions, query_org_memory_records},
    ast::MemoryRecordState,
};
use std::{collections::BTreeMap, fs, path::Path};

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
            reflection_complete: reflection_complete(&record.path),
            path: record.path,
            title: record.title,
            todo: record.todo.unwrap_or_default(),
            todo_type: todo_type(record.state).to_string(),
            properties: record.properties.into_iter().collect::<BTreeMap<_, _>>(),
            mtime: record.mtime,
        })
        .collect())
}

fn reflection_complete(path: &Path) -> bool {
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    let mut in_reflection = false;
    let mut saw_reflection_answer = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('*') {
            let heading = trimmed.trim_start_matches('*').trim();
            if heading.eq_ignore_ascii_case("Reflection") {
                in_reflection = true;
                continue;
            }
            if in_reflection {
                break;
            }
        }
        if !in_reflection || !trimmed.starts_with('|') {
            continue;
        }
        let cells = table_cells(trimmed);
        if cells.len() < 3 || is_table_header(&cells) || is_table_separator(&cells) {
            continue;
        }
        let (question, value) = reflection_question_value(&cells);
        if question.is_empty() {
            continue;
        }
        if reflection_value_missing(value) {
            return false;
        }
        saw_reflection_answer = true;
    }
    saw_reflection_answer
}

fn table_cells(line: &str) -> Vec<&str> {
    line.trim_matches('|').split('|').map(str::trim).collect()
}

fn reflection_question_value<'a>(cells: &'a [&'a str]) -> (&'a str, &'a str) {
    if cells.len() >= 4 {
        (cells[1], cells[2])
    } else {
        (cells[0], cells[1])
    }
}

fn is_table_header(cells: &[&str]) -> bool {
    cells.iter().any(|cell| cell.eq_ignore_ascii_case("value"))
        && cells
            .iter()
            .any(|cell| cell.eq_ignore_ascii_case("question"))
}

fn is_table_separator(cells: &[&str]) -> bool {
    cells
        .iter()
        .all(|cell| !cell.is_empty() && cell.chars().all(|ch| matches!(ch, '-' | '+' | ' ' | '\t')))
}

fn reflection_value_missing(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "0" | "-" | "pending" | "todo" | "tbd" | "none" | "null" | "n/a"
        )
}

fn todo_type(state: MemoryRecordState) -> &'static str {
    match state {
        MemoryRecordState::Closed => "Done",
        MemoryRecordState::Current => "Todo",
        MemoryRecordState::Archived => "Done",
        MemoryRecordState::Background => "",
    }
}
