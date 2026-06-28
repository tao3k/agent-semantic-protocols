use super::model::{OrgPlanCandidate, OrgTaskCandidate};
use orgize::{
    agent::{DocumentWalkConfig, OrgMemorySearchOptions, query_org_memory_records},
    ast::MemoryRecordState,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const MAX_TASK_CANDIDATES_PER_PLAN: usize = 5;

pub(super) fn scan_org_plan_candidates(
    artifacts_root: &Path,
    archive_dir: &str,
    include_done: bool,
    _org_query_bin: &str,
) -> Result<Vec<OrgPlanCandidate>, String> {
    let mut walk_config = DocumentWalkConfig::default();
    let mut ignored_dirs = walk_config
        .ignore_dirs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for dir in ["archive", "archives", archive_dir] {
        if ignored_dirs.insert(dir.to_string()) {
            walk_config.ignore_dirs.push(dir.to_string());
        }
    }
    let mut options = OrgMemorySearchOptions::plan_ledgers();
    options.include_closed = include_done;
    let records = query_org_memory_records(artifacts_root, &walk_config, &options)?;
    Ok(records
        .into_iter()
        .filter(|record| include_done || record.state != MemoryRecordState::Closed)
        .map(|record| {
            let properties = record.properties.into_iter().collect::<BTreeMap<_, _>>();
            OrgPlanCandidate {
                reflection_complete: reflection_complete(&record.path),
                task_candidates: task_candidates(&record.path, &properties),
                path: record.path,
                title: record.title,
                todo: record.todo.unwrap_or_default(),
                todo_type: todo_type(record.state).to_string(),
                properties,
                mtime: record.mtime,
            }
        })
        .collect())
}

fn task_candidates(path: &Path, properties: &BTreeMap<String, String>) -> Vec<OrgTaskCandidate> {
    let mut candidates = Vec::new();
    let mut seen_titles = BTreeSet::new();
    push_property_task_candidate(
        &mut candidates,
        &mut seen_titles,
        properties,
        "NEXT_ACTION",
        "next-action",
    );

    if let Ok(source) = fs::read_to_string(path) {
        let mut section = None;
        for (line_index, line) in source.lines().enumerate() {
            if candidates.len() >= MAX_TASK_CANDIDATES_PER_PLAN {
                break;
            }
            let trimmed = line.trim();
            if let Some((level, heading)) = heading(trimmed) {
                let clean_heading = clean_task_title(heading);
                if !clean_heading.is_empty() {
                    section = Some(clean_heading);
                }
                if level >= 2 {
                    if let Some((status, title)) = todo_heading_task(heading) {
                        push_task_candidate(
                            &mut candidates,
                            &mut seen_titles,
                            OrgTaskCandidate {
                                kind: "heading".to_string(),
                                status,
                                title,
                                section: section.clone(),
                                source_line: Some(line_index + 1),
                            },
                        );
                    }
                }
                continue;
            }
            if let Some((status, title)) = checklist_task(trimmed) {
                push_task_candidate(
                    &mut candidates,
                    &mut seen_titles,
                    OrgTaskCandidate {
                        kind: "checklist".to_string(),
                        status,
                        title,
                        section: section.clone(),
                        source_line: Some(line_index + 1),
                    },
                );
            }
        }
    }

    if candidates.is_empty() {
        push_property_task_candidate(
            &mut candidates,
            &mut seen_titles,
            properties,
            "OBJECTIVE",
            "objective",
        );
    }
    candidates.truncate(MAX_TASK_CANDIDATES_PER_PLAN);
    candidates
}

fn push_property_task_candidate(
    candidates: &mut Vec<OrgTaskCandidate>,
    seen_titles: &mut BTreeSet<String>,
    properties: &BTreeMap<String, String>,
    key: &str,
    status: &str,
) {
    let Some(value) = properties.get(key).map(|value| value.trim()) else {
        return;
    };
    if value.is_empty() {
        return;
    }
    push_task_candidate(
        candidates,
        seen_titles,
        OrgTaskCandidate {
            kind: "property".to_string(),
            status: status.to_string(),
            title: clean_task_title(value),
            section: None,
            source_line: None,
        },
    );
}

fn push_task_candidate(
    candidates: &mut Vec<OrgTaskCandidate>,
    seen_titles: &mut BTreeSet<String>,
    candidate: OrgTaskCandidate,
) {
    if candidate.title.is_empty() {
        return;
    }
    if !seen_titles.insert(candidate.title.to_ascii_lowercase()) {
        return;
    }
    candidates.push(candidate);
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|ch| *ch == '*').count();
    if level == 0 {
        return None;
    }
    let rest = line.get(level..)?.trim();
    if rest.is_empty() {
        None
    } else {
        Some((level, rest))
    }
}

fn todo_heading_task(heading: &str) -> Option<(String, String)> {
    let split_at = heading
        .char_indices()
        .find_map(|(index, ch)| ch.is_whitespace().then_some(index))?;
    let keyword = heading.get(..split_at)?;
    let title = heading.get(split_at..)?;
    let status = match keyword {
        "TODO" | "NEXT" | "STARTED" | "WAIT" | "WAITING" | "BLOCKED" => keyword,
        _ => return None,
    };
    let title = clean_task_title(title);
    if title.is_empty() {
        None
    } else {
        Some((status.to_ascii_lowercase(), title))
    }
}

fn checklist_task(line: &str) -> Option<(String, String)> {
    let item = strip_list_marker(line)?;
    let mut chars = item.chars();
    if chars.next()? != '[' {
        return None;
    }
    let state = chars.next()?;
    if chars.next()? != ']' {
        return None;
    }
    let status = match state {
        ' ' => "unchecked",
        '-' => "partial",
        'X' | 'x' => return None,
        _ => return None,
    };
    let title = clean_task_title(item.get(3..).unwrap_or_default());
    if title.is_empty() {
        None
    } else {
        Some((status.to_string(), title))
    }
}

fn strip_list_marker(line: &str) -> Option<&str> {
    for marker in ["- ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return Some(rest.trim_start());
        }
    }
    let split_at = [line.find('.'), line.find(')')]
        .into_iter()
        .flatten()
        .min()?;
    if split_at == 0 || !line[..split_at].chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    line.get(split_at + 1..).map(str::trim_start)
}

fn clean_task_title(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|token| !is_progress_cookie(token) && !is_tag_cookie(token))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn is_progress_cookie(token: &str) -> bool {
    let Some(inner) = token
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return false;
    };
    if let Some(percent) = inner.strip_suffix('%') {
        return percent.chars().all(|ch| ch.is_ascii_digit());
    }
    let Some((left, right)) = inner.split_once('/') else {
        return false;
    };
    left.chars().all(|ch| ch.is_ascii_digit()) && right.chars().all(|ch| ch.is_ascii_digit())
}

fn is_tag_cookie(token: &str) -> bool {
    token.len() > 2 && token.starts_with(':') && token.ends_with(':')
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
