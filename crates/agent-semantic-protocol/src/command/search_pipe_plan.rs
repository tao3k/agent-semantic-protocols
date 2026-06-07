//! Query-pipeline projection helpers for ASP-owned search pipe.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use super::search_pipe_render::Candidate;

#[derive(Clone, Debug)]
struct PipeAction {
    index: usize,
    owner: String,
    selector: String,
    symbol: String,
}

pub(super) fn render_search_pipe_plan(
    language_id: &str,
    query: &str,
    candidates: &[Candidate],
    ranked_compact: Option<&str>,
) -> String {
    let quoted_query = shell_quote(query);
    let actions = concrete_pipe_actions(candidates, ranked_compact);
    let action_stages = if actions.is_empty() {
        "pipeStages=search-prime,search-pipe,query-selector,search-reasoning\n\
selectorPolicy=defer reason=no-exact-selector next=search-reasoning\n"
            .to_string()
    } else {
        render_action_lines(&actions)
    };
    let next_action_lines = render_next_action_lines(language_id, &actions);
    let command_line = if actions.is_empty() {
        format!(
            "pipeCommands=context=>asp {language_id} search prime --view seeds .,pipe=>asp {language_id} search pipe {quoted_query} --view seeds .,owner-query=>asp {language_id} search reasoning owner-query --owner <owner-path> --query {quoted_query} --view seeds .,selector=>asp {language_id} query --selector <selector> --code .\n"
        )
    } else {
        render_concrete_pipe_commands(language_id, query, &actions)
    };
    let choice_line = pipe_choice_lines(ranked_compact);
    format!(
        "pipePlan=query-pipeline alg=asp-search-pipe-v1 budget=asp<=8,search<=4,query<=4,repeated=0\n\
pipeExpr=prime |> search(term={quoted_query}) |> rank(profile=owner-query) |> filter(path=source-preferred) |> project(frontierActions,pipeCommands,selectors) |> choose(branch=bounded,max=3,repeat=false,rewrite=false)\n\
pipeProjections=graph-frontier,frontierActions,pipeCommands\n\
{choice_line}\
{action_stages}\
{next_action_lines}\
{command_line}\
stop=after-first-query-selector-read-or-after-projected-branches answer-from-evidence=true no-search-after-projected-branches=true\n\
avoid=repeat-prime,repeat-pipe,query-rewrite-pipe,reasoning-before-selector,repeat-fzf,broad-fzf,post-projection-owner-search,post-projection-fzf,post-projection-treesitter-guide,raw-read,manual-window-scan,wide-windows\n"
    )
}

fn pipe_choice_lines(ranked_compact: Option<&str>) -> &'static str {
    let graph_choice = ranked_compact
        .map(|compact| compact.lines().any(|line| line.starts_with("pipeChoice=")))
        .unwrap_or(false);
    if graph_choice {
        "pipeExecution=each-branch-at-most-once\n"
    } else {
        "pipeChoiceFallback=bounded-fanout maxBranches=3 repeat=false owner=asp-rust-fallback reason=missing-graph-turbo-projection\n\
pipeExecution=each-branch-at-most-once\n"
    }
}

fn concrete_pipe_actions(
    candidates: &[Candidate],
    ranked_compact: Option<&str>,
) -> Vec<PipeAction> {
    if let Some(compact) = ranked_compact {
        let actions = concrete_pipe_actions_from_compact(compact);
        if !actions.is_empty() {
            return actions;
        }
    }
    concrete_pipe_actions_from_candidates(candidates)
}

fn concrete_pipe_actions_from_candidates(candidates: &[Candidate]) -> Vec<PipeAction> {
    let mut actions = Vec::new();
    let mut selectors = HashSet::new();
    for candidate in candidates.iter().take(12) {
        let selector = format!("{}:{}:{}", candidate.path, candidate.line, candidate.line);
        if !selectors.insert(selector.clone()) {
            continue;
        }
        actions.push(PipeAction {
            index: actions.len() + 1,
            owner: candidate.path.clone(),
            selector,
            symbol: candidate.symbol.clone(),
        });
        if actions.len() >= 3 {
            break;
        }
    }
    actions
}

fn concrete_pipe_actions_from_compact(compact: &str) -> Vec<PipeAction> {
    let projected = concrete_pipe_actions_from_projected_frontier(compact);
    if !projected.is_empty() {
        return projected;
    }

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
        .filter_map(|part| parse_selector_action(part.trim()))
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
    let selector = action_field(fields, "selector")?.to_string();
    let owner = action_field(fields, "owner")
        .unwrap_or_default()
        .to_string();
    let symbol = action_field(fields, "symbol")
        .unwrap_or("match")
        .to_string();
    Some(PipeAction {
        index,
        owner,
        selector,
        symbol,
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

fn render_action_lines(actions: &[PipeAction]) -> String {
    let mut rendered = "pipeStages=search-prime,search-pipe,query-selector,search-reasoning\n\
selectorPolicy=run-first reason=exact-selector-present before=search-reasoning\n"
        .to_string();
    for action in actions {
        let _ = writeln!(
            rendered,
            "frontierActions=S{index}.selector(selector={selector},owner={owner},symbol={symbol})!query-selector",
            index = action.index,
            selector = action.selector,
            owner = action.owner,
            symbol = action.symbol,
        );
        let _ = writeln!(
            rendered,
            "frontierActions=R{index}.reasoning(owner={owner},querySource=search-pipe)!search-reasoning",
            index = action.index,
            owner = action.owner,
        );
    }
    rendered
}

fn render_next_action_lines(language_id: &str, actions: &[PipeAction]) -> String {
    let Some(action) = actions.first() else {
        return String::new();
    };
    let command = format!(
        "asp {language_id} query --selector {selector} --code .",
        selector = shell_arg(&action.selector),
    );
    format!(
        "recommendedNext=S{index}.query-selector\nnextCommand={command}\n",
        index = action.index,
    )
}

fn render_concrete_pipe_commands(language_id: &str, query: &str, actions: &[PipeAction]) -> String {
    let quoted_query = shell_quote(query);
    let mut commands = vec![
        format!("context=>asp {language_id} search prime --view seeds ."),
        format!("pipe=>asp {language_id} search pipe {quoted_query} --view seeds ."),
    ];
    for action in actions {
        commands.push(format!(
            "S{index}=>asp {language_id} query --selector {selector} --code .",
            index = action.index,
            selector = shell_arg(&action.selector),
        ));
        commands.push(format!(
            "R{index}=>asp {language_id} search reasoning owner-query --owner {owner} --query {quoted_query} --view seeds .",
            index = action.index,
            owner = shell_arg(&action.owner),
        ));
    }
    format!("pipeCommands={}\n", commands.join(","))
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

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':'))
    {
        value.to_string()
    } else {
        shell_quote(value)
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
