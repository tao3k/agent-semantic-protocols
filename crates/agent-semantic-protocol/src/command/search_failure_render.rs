//! Failure-frontier graph-turbo request adapter for ASP-owned search helpers.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_provider_transport::byte_text;
use serde_json::{Value, json};

use super::graph::rank_graph_turbo_packet;
use super::search_pipe_model::Candidate;

const GRAPH_TURBO_REQUEST_SCHEMA_ID: &str = "agent.semantic-protocols.semantic-graph-turbo-request";
const FAILURE_HOT_BLOCK_MAX_LINES: usize = 80;

pub(super) fn render_failure_frontier(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    failure_text: &str,
    candidates: &[Candidate],
) -> Result<String, String> {
    let packet = failure_graph_turbo_request(
        language_id,
        project_root,
        locator_root,
        failure_text,
        candidates,
    );
    let packet_bytes = serde_json::to_vec(&packet)
        .map_err(|error| format!("failed to serialize failure graph turbo request: {error}"))?;
    let ranked_packet = rank_graph_turbo_packet(&packet_bytes)?.ok_or_else(|| {
        "search failure requires asp-graph-turbo with failure-frontier support".to_string()
    })?;
    let request = agent_semantic_search_projection::SearchProjectionRequestV1::new(
        "ranked-frontier",
        agent_semantic_search_projection::SearchProjectionDensityV1::Terse,
    );
    let renderer = agent_semantic_search_projection::RankedFrontierSearchProjectionRenderer;
    let rendered = agent_semantic_search_projection::SearchProjectionRenderer::render(
        &renderer,
        &ranked_packet,
        &request,
    )
    .map_err(|error| format!("failed to render ranked failure frontier: {error}"))?;
    Ok(rendered.content)
}

pub(super) fn render_failure_graph_turbo_request(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    failure_text: &str,
    candidates: &[Candidate],
) -> Result<String, String> {
    let packet = failure_graph_turbo_request(
        language_id,
        project_root,
        locator_root,
        failure_text,
        candidates,
    );
    serde_json::to_string_pretty(&packet)
        .map(|mut text| {
            text.push('\n');
            text
        })
        .map_err(|error| format!("failed to serialize failure graph turbo request: {error}"))
}

fn failure_graph_turbo_request(
    language_id: &str,
    project_root: &Path,
    locator_root: &Path,
    failure_text: &str,
    candidates: &[Candidate],
) -> Value {
    let failure_kind = if failure_text.contains("::") || failure_text.contains("test") {
        "test-failure"
    } else {
        "check-failure"
    };
    let failure_label = compact_label(&failure_label(failure_text));
    let test_path = failure_test_path(candidates);
    let key = compact_label(&failure_key(failure_text));
    let evidence = compact_label(&failure_evidence(failure_text));
    let failure_id = stable_node_id("failure", &failure_label);
    let assert_id = stable_node_id("assert", &format!("{failure_label}:assert"));
    let test_id = stable_node_id("test", &test_path);
    let key_id = stable_node_id("key", &key);
    let evidence_id = stable_node_id("evidence", &evidence);

    let hot_blocks = ordered_failure_candidates(candidates)
        .filter_map(|candidate| failure_hot_block(project_root, locator_root, candidate))
        .fold(Vec::<FailureHotBlock>::new(), |mut hot_blocks, hot| {
            if !hot_blocks
                .iter()
                .any(|existing| existing.selector == hot.selector)
            {
                hot_blocks.push(hot);
            }
            hot_blocks
        })
        .into_iter()
        .take(4)
        .collect::<Vec<_>>();
    let owners = failure_owner_paths(&hot_blocks, candidates);

    let mut nodes = vec![
        json!({
            "id": failure_id.clone(),
            "kind": "failure",
            "role": failure_kind,
            "value": failure_label.clone(),
            "action": "failure",
            "languageId": language_id,
            "failureKind": failure_kind,
        }),
        json!({
            "id": assert_id.clone(),
            "kind": "assert",
            "role": "failure",
            "value": "expected=pass,actual=fail",
            "action": "evidence",
            "languageId": language_id,
        }),
        json!({
            "id": test_id.clone(),
            "kind": "test",
            "role": "path",
            "value": test_path.clone(),
            "action": "code",
            "path": test_path.clone(),
            "languageId": language_id,
        }),
        json!({
            "id": key_id.clone(),
            "kind": "key",
            "role": "signal",
            "value": key.clone(),
            "action": "evidence",
            "languageId": language_id,
        }),
        json!({
            "id": evidence_id.clone(),
            "kind": "evidence",
            "role": "signal",
            "value": evidence.clone(),
            "action": "evidence",
            "languageId": language_id,
        }),
    ];
    for owner in &owners {
        nodes.push(json!({
            "id": stable_node_id("owner", owner),
            "kind": "owner",
            "role": "path",
            "value": owner,
            "action": "owner",
            "path": owner,
            "languageId": language_id,
        }));
    }
    for hot in &hot_blocks {
        let display_line_range = format!("{}:{}", hot.start, hot.end);
        let structural_selector = format!(
            "{}://{}#item/{}/{}",
            language_id, hot.path, hot.kind, hot.symbol
        );
        nodes.push(json!({
            "id": hot.id.clone(),
            "kind": "hot",
            "role": hot.kind,
            "value": hot.symbol.clone(),
            "target": hot.symbol.clone(),
            "action": "code",
            "path": hot.path.clone(),
            "ownerPath": hot.path.clone(),
            "itemName": hot.symbol.clone(),
            "itemKind": hot.kind,
            "symbol": hot.symbol.clone(),
            "startLine": hot.start,
            "endLine": hot.end,
            "displayLineRange": display_line_range.clone(),
            "sourceLocatorHint": hot.selector.clone(),
            "structuralSelector": structural_selector.clone(),
            "projection": "code",
            "codePolicy": "requires-exact-code",
            "requiresExact": true,
            "locator": structural_selector.clone(),
            "matchedTerm": hot.matched_term.clone(),
            "matchLine": hot.match_line,
            "boundarySource": "syntax-header-scan",
            "languageId": language_id,
            "fields": {
                "ownerPath": hot.path.clone(),
                "itemName": hot.symbol.clone(),
                "itemKind": hot.kind,
                "displayLineRange": display_line_range,
                "sourceLocatorHint": hot.selector.clone(),
                "structuralSelector": structural_selector,
                "projection": "code",
                "codePolicy": "requires-exact-code",
                "requiresExact": true
            }
        }));
    }

    let mut edges = vec![
        edge(&failure_id, &test_id, "fails"),
        edge(&failure_id, &assert_id, "explains"),
        edge(&assert_id, &key_id, "checks"),
        edge(&assert_id, &evidence_id, "gates"),
    ];
    for owner in &owners {
        edges.push(edge(
            &failure_id,
            &stable_node_id("owner", owner),
            "selects",
        ));
    }
    for hot in &hot_blocks {
        edges.push(edge(&assert_id, &hot.id, "checks"));
        edges.push(edge(
            &stable_node_id("owner", &hot.path),
            &hot.id,
            "contains",
        ));
        edges.push(edge(&hot.id, &key_id, "relates"));
        edges.push(edge(&hot.id, &evidence_id, "validates"));
    }

    json!({
        "schemaId": GRAPH_TURBO_REQUEST_SCHEMA_ID,
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "packetKind": "graph-turbo-request",
        "profile": "failure-frontier",
        "algorithm": "typed-ppr-diverse",
        "seedIds": [failure_id.clone()],
        "budget": 10,
        "kindBudgets": {
            "failure": 1,
            "assert": 1,
            "hot": 4,
            "owner": 4,
            "test": 1,
            "key": 1,
            "evidence": 1
        },
        "windowMerge": {"enabled": true, "maxGapLines": 8},
        "pathBudget": 5,
        "pathMaxHops": 4,
        "cache": {"enabled": true},
        "graph": {
            "nodes": nodes,
            "edges": edges,
        },
    })
}

struct FailureHotBlock {
    id: String,
    selector: String,
    symbol: String,
    kind: &'static str,
    path: String,
    start: usize,
    end: usize,
    matched_term: String,
    match_line: usize,
}

fn failure_hot_block(
    project_root: &Path,
    locator_root: &Path,
    candidate: &Candidate,
) -> Option<FailureHotBlock> {
    let path = candidate_absolute_path(project_root, locator_root, candidate);
    let item = hot_item(&path, candidate.line)?;
    let display = display_path(locator_root, &path);
    let selector = format!("{}:{}:{}", display, item.start, item.end);
    Some(FailureHotBlock {
        id: stable_node_id("hot", &format!("{selector}:{}", item.symbol)),
        selector,
        symbol: item.symbol,
        kind: item.kind,
        path: display,
        start: item.start,
        end: item.end,
        matched_term: candidate.symbol.clone(),
        match_line: candidate.line,
    })
}

fn candidate_absolute_path(
    project_root: &Path,
    locator_root: &Path,
    candidate: &Candidate,
) -> PathBuf {
    let path = Path::new(&candidate.path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let locator_path = locator_root.join(path);
    if locator_path.exists() {
        return locator_path;
    }
    let project_path = project_root.join(path);
    if project_path.exists() {
        return project_path;
    }
    locator_path
}

struct HotItem {
    symbol: String,
    kind: &'static str,
    start: usize,
    end: usize,
}

fn hot_item(path: &Path, line: usize) -> Option<HotItem> {
    let bytes = fs::read(path).ok()?;
    let lines = byte_text::line_slices(&bytes);
    let start_index = line.saturating_sub(1);
    for index in (0..=start_index).rev() {
        let text = byte_text::lossy_string(lines.get(index)?);
        let Some((kind, symbol)) = item_from_line(&text) else {
            continue;
        };
        let start = index + 1;
        let end = rust_block_end(path, &lines, index)
            .or_else(|| python_block_end(path, &lines, index))
            .unwrap_or(start)
            .min(start.saturating_add(FAILURE_HOT_BLOCK_MAX_LINES - 1));
        if line <= end {
            return Some(HotItem {
                symbol,
                kind,
                start,
                end,
            });
        }
    }
    None
}

fn item_from_line(line: &str) -> Option<(&'static str, String)> {
    for (keyword, kind) in [
        ("fn", "fn"),
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("type", "type"),
        ("mod", "mod"),
        ("const", "const"),
        ("static", "static"),
        ("def", "function"),
        ("class", "class"),
    ] {
        if let Some(name) = name_after_keyword(line, keyword) {
            return Some((kind, name));
        }
    }
    None
}

fn name_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let needle = format!("{keyword} ");
    let start = line.find(&needle)? + needle.len();
    let name = line[start..]
        .trim_start()
        .chars()
        .take_while(|character| *character == '_' || character.is_ascii_alphanumeric())
        .collect::<String>();
    if name.is_empty() { None } else { Some(name) }
}

fn failure_test_path(candidates: &[Candidate]) -> String {
    candidates
        .iter()
        .find(|candidate| is_test_path(&candidate.path))
        .or_else(|| candidates.first())
        .map(|candidate| candidate.path.clone())
        .unwrap_or_else(|| ".".to_string())
}

fn ordered_failure_candidates(candidates: &[Candidate]) -> impl Iterator<Item = &Candidate> {
    candidates
        .iter()
        .filter(|candidate| !is_test_path(&candidate.path))
        .chain(
            candidates
                .iter()
                .filter(|candidate| is_test_path(&candidate.path)),
        )
}

fn failure_owner_paths(hot_blocks: &[FailureHotBlock], candidates: &[Candidate]) -> Vec<String> {
    let mut seen = HashSet::new();
    hot_blocks
        .iter()
        .map(|hot| hot.path.clone())
        .chain(ordered_failure_candidates(candidates).map(|candidate| candidate.path.clone()))
        .filter(|path| path != ".")
        .filter_map(|path| seen.insert(path.clone()).then_some(path))
        .take(4)
        .collect()
}

fn is_test_path(path: &str) -> bool {
    path.starts_with("tests/") || path.contains("/tests/") || path.contains("_test")
}

fn failure_label(text: &str) -> String {
    text.split_whitespace()
        .map(|token| token.trim_matches(|character: char| !is_label_character(character)))
        .find(|token| token.contains("::"))
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .or_else(|| {
            text.lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown-failure".to_string())
}

fn failure_key(text: &str) -> String {
    interesting_failure_token(
        text,
        &["fingerprint", "hash", "expected", "actual", "hit", "miss"],
    )
    .unwrap_or_else(|| "failure-signal".to_string())
}

fn failure_evidence(text: &str) -> String {
    interesting_failure_token(text, &["file_hash", "hash", "observed", "expected"])
        .map(|token| format!("{token}(observed=failure)"))
        .unwrap_or_else(|| "failure-observed".to_string())
}

fn interesting_failure_token(text: &str, needles: &[&str]) -> Option<String> {
    let tokens = text
        .split(|character: char| !is_label_character(character))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    for needle in needles {
        if let Some(token) = tokens
            .iter()
            .find(|token| token.to_ascii_lowercase().contains(needle))
        {
            return Some((*token).to_string());
        }
    }
    None
}

fn compact_label(value: &str) -> String {
    let compact = value
        .chars()
        .map(|character| match character {
            '\n' | '\r' | ';' | '{' | '}' => ' ',
            _ => character,
        })
        .collect::<String>();
    let compact = compact.split_whitespace().collect::<Vec<_>>().join("-");
    if compact.len() > 96 {
        compact.chars().take(96).collect()
    } else {
        compact
    }
}

fn is_label_character(character: char) -> bool {
    character == '_' || character == '-' || character == ':' || character.is_ascii_alphanumeric()
}

fn edge(source: &str, target: &str, relation: &str) -> Value {
    json!({
        "source": source,
        "target": target,
        "relation": relation,
    })
}

fn rust_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return None;
    }
    let mut saw_open = false;
    let mut brace_depth = 0isize;
    for (line_index, line) in lines.iter().enumerate().skip(start_index) {
        for byte in *line {
            match byte {
                b'{' => {
                    saw_open = true;
                    brace_depth += 1;
                }
                b'}' if saw_open => {
                    brace_depth -= 1;
                }
                _ => {}
            }
        }
        if saw_open && brace_depth <= 0 {
            let end = line_index + 1;
            return Some(if end == start_index + 1 { end + 1 } else { end });
        }
    }
    None
}

fn python_block_end(path: &Path, lines: &[&[u8]], start_index: usize) -> Option<usize> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("py") {
        return None;
    }
    let base_indent = leading_spaces(lines.get(start_index)?);
    for (line_index, line) in lines.iter().enumerate().skip(start_index + 1) {
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        let indent = leading_spaces(line);
        if indent <= base_indent {
            return Some(line_index);
        }
    }
    Some(lines.len())
}

fn leading_spaces(line: &[u8]) -> usize {
    line.iter().take_while(|byte| **byte == b' ').count()
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn stable_node_id(kind: &str, value: &str) -> String {
    let mut rendered = String::with_capacity(kind.len() + value.len() + 1);
    rendered.push_str(kind);
    rendered.push(':');
    for character in value.chars() {
        if character == '_' || character == '-' || character == '/' || character == '.' {
            rendered.push(character);
        } else if character.is_ascii_alphanumeric() {
            rendered.push(character.to_ascii_lowercase());
        } else {
            rendered.push('-');
        }
    }
    while rendered.ends_with('-') {
        rendered.pop();
    }
    if rendered.len() == kind.len() + 1 {
        rendered.push_str("node");
    }
    rendered
}
