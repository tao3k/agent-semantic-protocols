//! Query-pipeline projection helpers for ASP-owned search pipe.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::search_pipe_action_model::PipeAction;
use super::search_pipe_actions::{
    SearchPipeActionRequest, render_action_frontier, sanitize_evidence_line,
};
use super::search_pipe_quality::{SearchPipeQuality, analyze_search_pipe_quality};
use super::search_pipe_seed_decision::SeedActionIntent;
use super::search_query_wrapper_preview::{fd_query_preview, fd_query_preview_from_candidates};
use super::{search_pipe_model::Candidate, search_pipe_projection::candidate_selector};

pub(super) struct SearchPipePlanRequest<'a> {
    pub(super) language_id: &'a str,
    pub(super) project_root: &'a Path,
    pub(super) locator_root: &'a Path,
    pub(super) scopes: &'a [PathBuf],
    pub(super) query: &'a str,
    pub(super) candidates: &'a [Candidate],
    pub(super) precomputed_quality: Option<SearchPipeQuality>,
    pub(super) ranked_compact: Option<&'a str>,
    pub(super) seed_action_intents: &'a [SeedActionIntent],
    pub(super) read_memory_selectors: &'a [String],
    pub(super) dependency_action_targets: &'a [String],
}

pub(super) fn render_search_pipe_plan(request: SearchPipePlanRequest<'_>) -> String {
    let SearchPipePlanRequest {
        language_id,
        project_root,
        locator_root,
        scopes,
        query,
        candidates,
        precomputed_quality,
        ranked_compact,
        seed_action_intents,
        read_memory_selectors,
        dependency_action_targets,
    } = request;
    let mut quality = precomputed_quality
        .unwrap_or_else(|| analyze_search_pipe_quality(language_id, query, candidates));
    let projected_selector_actions = rank_projected_selector_actions(
        query,
        &quality,
        ranked_compact
            .map(concrete_pipe_actions_from_compact)
            .unwrap_or_default(),
    );
    if quality.query_pack_quality != "low"
        && quality.package_cohesion != "low"
        && ranked_compact
            .map(|compact| compact_has_provider_semantic_answer(query, compact))
            .unwrap_or(false)
    {
        quality.allow_query_selector = true;
    }
    let actions = if !projected_selector_actions.is_empty() {
        projected_selector_actions
    } else if quality.allow_query_selector {
        concrete_pipe_actions(language_id, candidates, ranked_compact)
    } else {
        Vec::new()
    };
    let fd_preview = if !quality.allow_query_selector
        && !candidates.is_empty()
        && !skip_fd_preview_for_action_meta_query(&quality, candidates)
    {
        fd_query_preview_from_candidates(candidates).or_else(|| {
            quality
                .fd_query
                .as_deref()
                .and_then(|query| fd_query_preview(project_root, locator_root, scopes, query))
        })
    } else {
        None
    };
    let action_frontier_lines = render_action_frontier(SearchPipeActionRequest {
        language_id,
        project_root,
        locator_root,
        scopes,
        quality: &quality,
        candidates,
        ranked_compact,
        selector_actions: &actions,
        fd_preview: fd_preview.as_ref(),
        seed_action_intents,
        read_memory_selectors,
        dependency_action_targets,
    });
    format!(
        "seedPlan=seed-query alg=asp-search-pipe-v2 budget=frontier<=3 repeated=0\n\
{action_frontier_lines}\
nextClasses=search-deps,fd-query,rg-query,owner-items,treesitter-query,query-selector\n\
omit=source,full-candidate-list,raw-finder-output,generated-files,long-field-signatures\n\
avoid=repeat-search-pipe,broad-fzf,raw-rg,manual-window-scan,direct-source-read,raw-read\n",
    )
}

fn skip_fd_preview_for_action_meta_query(
    quality: &SearchPipeQuality,
    candidates: &[Candidate],
) -> bool {
    candidates.is_empty()
        && quality.global_matched.is_empty()
        && quality.owner_seed_terms.is_empty()
        && !quality.concept_terms.is_empty()
        && quality.concept_terms.iter().all(|term| {
            matches!(
                term.as_str(),
                "fd-query"
                    | "rg-query"
                    | "rg-query-set"
                    | "owner-items"
                    | "selector-code"
                    | "treesitter-query"
                    | "query-selector"
            )
        })
}

fn compact_has_provider_semantic_answer(query: &str, compact: &str) -> bool {
    let lower_query = query.to_ascii_lowercase();
    let requests_structural_field = [
        "field",
        "fields",
        "type",
        "types",
        "collection",
        "collections",
    ]
    .iter()
    .any(|term| lower_query.contains(term));
    requests_structural_field
        && compact.contains("field:")
        && (compact.contains("collection:") || compact.contains("type:"))
}

fn rank_projected_selector_actions(
    query: &str,
    quality: &SearchPipeQuality,
    mut actions: Vec<PipeAction>,
) -> Vec<PipeAction> {
    if actions.len() <= 1 {
        return actions;
    }
    let structural_query = query_requests_structural_fact(query);
    actions.sort_by_key(|action| {
        (
            -selector_action_score(action, quality, structural_query),
            action.index,
        )
    });
    for (index, action) in actions.iter_mut().enumerate() {
        action.index = index + 1;
    }
    actions
}

fn selector_action_score(
    action: &PipeAction,
    quality: &SearchPipeQuality,
    structural_query: bool,
) -> i32 {
    let mut score = 0;
    if quality
        .page_index_handles
        .iter()
        .any(|handle| handle == &action.owner)
    {
        score += 120;
    }
    if let Some(prefix) = package_prefix(&action.owner) {
        let package_votes = quality
            .page_index_handles
            .iter()
            .filter(|handle| package_prefix(handle).as_deref() == Some(prefix.as_str()))
            .count() as i32;
        score += package_votes * 30;
    }
    if action.owner.contains("/src/") {
        score += 20;
    }
    if action.owner.contains("/test/") || action.owner.contains("/tests/") {
        score -= 40;
    }
    if !structural_query && fact_source_alias(&action.source_alias) {
        score -= 100;
    }
    score
}

fn query_requests_structural_fact(query: &str) -> bool {
    query
        .split(|character: char| !(character == '_' || character.is_ascii_alphanumeric()))
        .map(str::to_ascii_lowercase)
        .any(|term| {
            matches!(
                term.as_str(),
                "field" | "fields" | "type" | "types" | "collection" | "collections"
            )
        })
}

fn fact_source_alias(alias: &str) -> bool {
    alias.starts_with('F') || alias.starts_with('Y') || alias.starts_with('C')
}

fn package_prefix(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    let root = parts.next()?;
    let package = parts.next()?;
    (root == "packages" && !package.is_empty()).then(|| format!("{root}/{package}"))
}

fn concrete_pipe_actions(
    language_id: &str,
    candidates: &[Candidate],
    ranked_compact: Option<&str>,
) -> Vec<PipeAction> {
    if let Some(compact) = ranked_compact {
        let actions = concrete_pipe_actions_from_compact(compact);
        if !actions.is_empty() {
            return actions;
        }
    }
    concrete_pipe_actions_from_candidates(language_id, candidates)
}

pub(super) fn concrete_pipe_actions_from_candidates(
    language_id: &str,
    candidates: &[Candidate],
) -> Vec<PipeAction> {
    let mut actions = Vec::new();
    let mut selectors = HashSet::new();
    for candidate in candidates.iter().take(12) {
        let selector = candidate_selector(language_id, candidate);
        if !selectors.insert(selector.clone()) {
            continue;
        }
        actions.push(PipeAction {
            index: actions.len() + 1,
            owner: candidate.path.clone(),
            selector,
            symbol: candidate.symbol.clone(),
            source_alias: String::new(),
        });
        if actions.len() >= 3 {
            break;
        }
    }
    actions
}

fn concrete_pipe_actions_from_compact(compact: &str) -> Vec<PipeAction> {
    let mut projected = concrete_pipe_actions_from_projected_frontier(compact);
    let ranked_actions = concrete_pipe_actions_from_ranked_compact(compact);
    if !projected.is_empty() {
        let mut selectors = projected
            .iter()
            .map(|action| action.selector.clone())
            .collect::<HashSet<_>>();
        for action in ranked_actions {
            if selectors.insert(action.selector.clone()) {
                projected.push(action);
            }
        }
        for (index, action) in projected.iter_mut().enumerate() {
            action.index = index + 1;
        }
        return projected.into_iter().take(8).collect();
    }
    ranked_actions
}

fn concrete_pipe_actions_from_ranked_compact(compact: &str) -> Vec<PipeAction> {
    let mut nodes = HashMap::new();
    let mut rank = Vec::new();
    for line in compact.lines() {
        if let Some(rank_value) = line.strip_prefix("rank=") {
            rank = rank_value
                .split_whitespace()
                .next()
                .unwrap_or(rank_value)
                .split(',')
                .map(str::to_string)
                .collect();
        }
        for segment in line.split(';') {
            if let Some((alias, action)) = pipe_action_from_node_segment(segment.trim()) {
                nodes.insert(alias, action);
            }
        }
    }

    let mut ranked_actions = Vec::new();
    let mut selectors = HashSet::new();
    for alias in rank {
        if let Some(action) = nodes.get(&alias) {
            if !selectors.insert(action.selector.clone()) {
                continue;
            }
            let mut action = action.clone();
            action.index = ranked_actions.len() + 1;
            ranked_actions.push(action);
        }
    }
    let mut preferred = ranked_actions
        .iter()
        .filter(|action| is_source_preferred_owner(&action.owner))
        .take(3)
        .cloned()
        .collect::<Vec<_>>();
    if preferred.is_empty() {
        preferred = ranked_actions.into_iter().take(3).collect();
    }
    for (index, action) in preferred.iter_mut().enumerate() {
        action.index = index + 1;
    }
    preferred
}

fn concrete_pipe_actions_from_projected_frontier(compact: &str) -> Vec<PipeAction> {
    for line in compact.lines() {
        let Some(value) = line.strip_prefix("frontierActions=") else {
            continue;
        };
        let actions = parse_projected_frontier_actions(value);
        if !actions.is_empty() {
            return actions;
        }
    }
    Vec::new()
}

fn parse_projected_frontier_actions(value: &str) -> Vec<PipeAction> {
    let segments = action_segments(value);
    let reasoning_owners = segments
        .iter()
        .filter_map(|part| parse_reasoning_action(part.trim()))
        .collect::<HashMap<_, _>>();
    let mut selector_actions = segments
        .iter()
        .filter_map(|part| {
            let part = part.trim();
            parse_selector_action(part).or_else(|| parse_query_code_action(part))
        })
        .map(|mut action| {
            if let Some(owner) = reasoning_owners.get(&action.index) {
                action.owner = owner.clone();
            }
            action
        })
        .collect::<Vec<_>>();
    selector_actions.sort_by_key(|action| action.index);
    selector_actions.into_iter().take(3).collect()
}

fn action_segments(value: &str) -> Vec<&str> {
    let (mut segments, _, start) = value.char_indices().fold(
        (Vec::new(), 0usize, 0usize),
        |(mut segments, depth, start), (index, character)| match character {
            '(' => (segments, depth + 1, start),
            ')' => (segments, depth.saturating_sub(1), start),
            ',' if depth == 0 => {
                segments.push(&value[start..index]);
                (segments, depth, index + 1)
            }
            _ => (segments, depth, start),
        },
    );
    segments.push(&value[start..]);
    segments
}

fn parse_reasoning_action(value: &str) -> Option<(usize, String)> {
    let rest = value.strip_prefix('R')?;
    let (index, rest) = rest.split_once(".reasoning(")?;
    let index = index.parse::<usize>().ok()?;
    let fields = rest.split_once(")!")?.0;
    let owner = action_field(fields, "owner")?;
    Some((index, owner.to_string()))
}

fn parse_selector_action(value: &str) -> Option<PipeAction> {
    let rest = value.strip_prefix('S')?;
    let (index, rest) = rest.split_once(".selector(")?;
    let index = index.parse::<usize>().ok()?;
    let fields = rest.split_once(")!")?.0;
    let selector = action_field(fields, "sourceLocatorHint")
        .or_else(|| action_field(fields, "structuralSelector"))
        .or_else(|| action_field(fields, "selector"))?
        .to_string();
    let owner = action_field(fields, "owner")
        .unwrap_or_default()
        .to_string();
    let symbol = action_field(fields, "symbol")
        .unwrap_or("match")
        .to_string();
    let source_alias = action_field(fields, "source")
        .unwrap_or_default()
        .to_string();
    Some(PipeAction {
        index,
        owner,
        selector,
        symbol,
        source_alias,
    })
}

fn parse_query_code_action(value: &str) -> Option<PipeAction> {
    let rest = value.strip_prefix('C')?;
    let (index, rest) = rest.split_once(".query-code(")?;
    let index = index.parse::<usize>().ok()?;
    let fields = rest.split_once(")!")?.0;
    let selector = action_field(fields, "sourceLocatorHint")
        .or_else(|| action_field(fields, "structuralSelector"))
        .or_else(|| action_field(fields, "selector"))?
        .to_string();
    let owner = action_field(fields, "owner")
        .unwrap_or_default()
        .to_string();
    let symbol = action_field(fields, "symbol")
        .unwrap_or("match")
        .to_string();
    let source_alias = action_field(fields, "source")
        .unwrap_or_default()
        .to_string();
    Some(PipeAction {
        index,
        owner,
        selector,
        symbol,
        source_alias,
    })
}

fn action_field<'a>(fields: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    fields
        .split(',')
        .find_map(|field| field.strip_prefix(&prefix))
}

fn pipe_action_from_node_segment(segment: &str) -> Option<(String, PipeAction)> {
    let (alias, node) = segment.split_once('=')?;
    if alias.is_empty() || !alias.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }
    if !(node.starts_with("I")
        || node.starts_with("H")
        || node.starts_with("item:")
        || node.starts_with("hot:"))
    {
        return None;
    }
    let locator = node.split_once('@')?.1.split_once('!')?.0;
    let (owner, selector) = owner_and_selector(locator)?;
    let symbol = node_symbol(node).unwrap_or_else(|| alias.to_ascii_lowercase());
    Some((
        alias.to_string(),
        PipeAction {
            index: 0,
            owner,
            selector,
            symbol,
            source_alias: alias.to_string(),
        },
    ))
}

fn owner_and_selector(locator: &str) -> Option<(String, String)> {
    let mut parts = locator.rsplitn(3, ':');
    let end = parts.next()?;
    let start = parts.next()?;
    let owner = parts.next()?;
    if owner.is_empty() || start.is_empty() || end.is_empty() {
        return None;
    }
    Some((owner.to_string(), format!("{owner}:{start}:{end}")))
}

fn node_symbol(node: &str) -> Option<String> {
    let start = node.find('(')? + 1;
    let end = node[start..].find(')')? + start;
    let symbol = &node[start..end];
    if symbol.is_empty() {
        None
    } else {
        Some(symbol.to_string())
    }
}

pub(super) fn render_primary_frontier_actions_only(compact: &str) -> String {
    let mut rendered = String::new();
    for line in compact.lines() {
        if is_graph_debug_line(line) {
            continue;
        }
        if line.starts_with("Q=query:") {
            continue;
        }
        if let Some(filtered) = seedless_rank_or_frontier_line(line) {
            rendered.push_str(&filtered);
            rendered.push('\n');
            continue;
        }
        if line.starts_with("frontierActions=") {
            continue;
        }
        rendered.push_str(&sanitize_evidence_line(line));
        rendered.push('\n');
    }
    rendered
}

pub(super) fn render_search_pipe_decision_projection(compact: &str) -> String {
    let mut rendered = String::new();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut visible_aliases = HashSet::new();
    for line in compact.lines() {
        if is_graph_debug_line(line)
            || line.starts_with("legend:")
            || line.starts_with("aliases=")
            || line.starts_with("Q=query:")
            || line.starts_with("frontierActions=")
            || line.starts_with("avoid=")
        {
            continue;
        }
        if let Some(filtered) = seedless_rank_or_frontier_line(line) {
            rendered.push_str(&filtered);
            rendered.push('\n');
            continue;
        }
        if line.starts_with("[graph-frontier]") {
            rendered.push_str(line);
            rendered.push('\n');
            continue;
        }
        if is_graph_edge_line(line) {
            if !line.starts_with("G>{") && !line.starts_with("Q>{") {
                edges.push(line.to_string());
            }
            continue;
        }
        if is_graph_node_line(line) {
            if let Some((alias, _)) = line.split_once('=') {
                visible_aliases.insert(alias.to_string());
            }
            nodes.push(sanitize_evidence_line(line));
            continue;
        }
        rendered.push_str(&sanitize_evidence_line(line));
        rendered.push('\n');
    }
    if !nodes.is_empty() {
        rendered.push_str("evidenceNodes=");
        rendered.push_str(&nodes.join(";"));
        rendered.push('\n');
    }
    if !edges.is_empty() {
        let edges = edges
            .into_iter()
            .filter_map(|edge| visible_graph_edge_line(&edge, &visible_aliases))
            .collect::<Vec<_>>();
        if edges.is_empty() {
            return rendered;
        }
        rendered.push_str("evidenceEdges=");
        rendered.push_str(&edges.join(";"));
        rendered.push('\n');
    }
    rendered
}

fn seedless_rank_or_frontier_line(line: &str) -> Option<String> {
    let (prefix, value) = if let Some(value) = line.strip_prefix("rank=") {
        ("rankedEvidence", value)
    } else if let Some(value) = line.strip_prefix("frontier=") {
        ("evidenceFrontier", value)
    } else {
        return None;
    };
    let filtered = value
        .split(',')
        .filter(|entry| {
            let entry = entry.trim();
            entry != "Q" && entry != "Q.fzf"
        })
        .map(evidence_frontier_entry)
        .collect::<Vec<_>>();
    Some(format!("{prefix}={}", filtered.join(",")))
}

fn is_graph_node_line(line: &str) -> bool {
    let Some((alias, value)) = line.split_once('=') else {
        return false;
    };
    !alias.is_empty()
        && alias
            .chars()
            .all(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
        && value.contains(':')
}

fn is_graph_edge_line(line: &str) -> bool {
    line.contains(">{") && line.ends_with('}')
}

fn visible_graph_edge_line(line: &str, visible_aliases: &HashSet<String>) -> Option<String> {
    let (source, targets) = line.split_once(">{")?;
    if !visible_aliases.contains(source) {
        return None;
    }
    let visible_targets = targets
        .trim_end_matches('}')
        .split(',')
        .filter(|target| {
            target
                .split_once(':')
                .map(|(alias, _)| visible_aliases.contains(alias.trim()))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    (!visible_targets.is_empty()).then(|| format!("{source}>{{{}}}", visible_targets.join(",")))
}

fn evidence_frontier_entry(entry: &str) -> String {
    let Some((alias, action)) = entry.split_once('.') else {
        return entry.to_string();
    };
    if action != "code" {
        return entry.to_string();
    }
    let replacement = if alias.starts_with('H') {
        "hot"
    } else if alias.starts_with('F') || alias.starts_with('Y') || alias.starts_with('C') {
        "evidence"
    } else {
        "syntax"
    };
    format!("{alias}.{replacement}")
}

fn is_graph_debug_line(line: &str) -> bool {
    matches!(
        line.split_once('=').map(|(key, _)| key),
        Some(
            "scores"
                | "paths"
                | "trace"
                | "explain"
                | "cache"
                | "queryCoverage"
                | "metrics"
                | "profiles"
                | "omit"
                | "pipeChoice"
                | "pipePolicy"
                | "pipePlan"
                | "pipeProjections"
                | "pipeExecution"
                | "pipeStages"
                | "selectorPolicy"
                | "recommendedNext"
                | "nextCommand"
                | "pipeCommands"
                | "conditionalActions"
        )
    )
}

fn is_source_preferred_owner(owner: &str) -> bool {
    !(owner.contains("/tests/")
        || owner.ends_with("/tests")
        || owner.contains("/benches/")
        || owner.ends_with("/benches")
        || owner.contains("/examples/")
        || owner.ends_with("/examples")
        || owner.contains("stress-test/"))
}
