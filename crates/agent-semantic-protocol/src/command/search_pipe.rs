//! ASP-owned search pipeline wrapper.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

const PIPE_CANDIDATE_LINE_LIMIT: usize = 256;

#[derive(Debug, Eq, PartialEq)]
struct SearchPipeArgs {
    query: String,
    pipes: Vec<String>,
    owners: Vec<PathBuf>,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct OwnerQueryArgs {
    owner: PathBuf,
    query: String,
    view: String,
}

#[derive(Debug, Eq, PartialEq)]
struct Candidate {
    path: String,
    line: usize,
    symbol: String,
    text: String,
}

pub(super) fn is_asp_fast_search(args: &[String]) -> bool {
    is_search_pipe(args) || is_reasoning_owner_query(args)
}

pub(super) fn run_asp_fast_search_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
) -> Result<(), String> {
    if is_search_pipe(args) {
        return run_search_pipe_command(language_id, args, project_root);
    }
    if is_reasoning_owner_query(args) {
        return run_reasoning_owner_query_command(args, project_root);
    }
    Err("unsupported ASP fast search command".to_string())
}

fn is_search_pipe(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("pipe"))
}

fn is_reasoning_owner_query(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("search"))
        && matches!(args.get(1).map(String::as_str), Some("reasoning"))
        && matches!(args.get(2).map(String::as_str), Some("owner-query"))
}

fn run_search_pipe_command(
    language_id: &str,
    args: &[String],
    project_root: &Path,
) -> Result<(), String> {
    let pipe_args = parse_search_pipe_args(args)?;
    if pipe_args.view == "commands" {
        print!("{}", render_search_pipe_commands(language_id, &pipe_args));
        return Ok(());
    }
    let candidates = collect_candidates(language_id, project_root, &pipe_args)?;
    print!("{}", render_ingest_frontier(&candidates, &pipe_args.pipes));
    Ok(())
}

fn run_reasoning_owner_query_command(args: &[String], project_root: &Path) -> Result<(), String> {
    let owner_query_args = parse_owner_query_args(args)?;
    if owner_query_args.view != "seeds" {
        return Err("search reasoning owner-query fast path supports --view seeds".to_string());
    }
    print!(
        "{}",
        render_owner_query_frontier(project_root, &owner_query_args)
    );
    Ok(())
}

fn parse_search_pipe_args(args: &[String]) -> Result<SearchPipeArgs, String> {
    if !is_search_pipe(args) {
        return Err("expected search pipe command".to_string());
    }
    let query = args
        .get(2)
        .filter(|query| !query.starts_with('-'))
        .ok_or_else(|| "search pipe requires a query".to_string())?
        .clone();
    let mut pipes = Vec::new();
    let mut owners = Vec::new();
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--pipe" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--pipe requires a value".to_string())?;
                pipes.extend(split_csv(value));
                index += 2;
            }
            "--owners" | "--owner" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{} requires a value", args[index]))?;
                owners.extend(split_csv(value).into_iter().map(PathBuf::from));
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown search pipe option: {value}"));
            }
            value => {
                owners.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    if !matches!(view.as_str(), "seeds" | "commands") {
        return Err("search pipe supports --view seeds or --view commands".to_string());
    }
    if pipes.is_empty() {
        pipes.extend(["items".to_string(), "tests".to_string()]);
    }
    Ok(SearchPipeArgs {
        query,
        pipes,
        owners,
        view,
    })
}

fn parse_owner_query_args(args: &[String]) -> Result<OwnerQueryArgs, String> {
    if !is_reasoning_owner_query(args) {
        return Err("expected search reasoning owner-query command".to_string());
    }
    let mut owner = None;
    let mut query = None;
    let mut view = "seeds".to_string();
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--owner" => {
                owner = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--owner requires a value".to_string())?,
                ));
                index += 2;
            }
            "--query" => {
                query = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--query requires a value".to_string())?
                        .clone(),
                );
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown search reasoning owner-query option: {value}"
                ));
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(OwnerQueryArgs {
        owner: owner.ok_or_else(|| "search reasoning owner-query requires --owner".to_string())?,
        query: query.ok_or_else(|| "search reasoning owner-query requires --query".to_string())?,
        view,
    })
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn render_search_pipe_commands(language_id: &str, args: &SearchPipeArgs) -> String {
    let pipe = args.pipes.join(",");
    let mut rendered = format!(
        "[search-pipe] lang={} q={} view=commands pipe={}\n",
        language_id, args.query, pipe
    );
    let _ = writeln!(
        rendered,
        "|replace slow=\"search fzf {} --view seeds\" with=\"search pipe '{}' --pipe {} --view seeds\"",
        args.query, args.query, pipe
    );
    let _ = writeln!(
        rendered,
        "|manual shell=\"rg -n '{}' <owners> | asp {} search ingest {} --view seeds <root>\"",
        args.query,
        language_id,
        pipe.replace(',', " ")
    );
    rendered
}

fn collect_candidates(
    language_id: &str,
    project_root: &Path,
    args: &SearchPipeArgs,
) -> Result<Vec<Candidate>, String> {
    let terms = query_terms(&args.query);
    if terms.is_empty() {
        return Err("search pipe requires a non-empty query".to_string());
    }
    let roots = if args.owners.is_empty() {
        vec![project_root.to_path_buf()]
    } else {
        args.owners
            .iter()
            .map(|owner| {
                if owner.is_absolute() {
                    owner.clone()
                } else {
                    project_root.join(owner)
                }
            })
            .collect()
    };
    let extensions = language_extensions(language_id);
    let mut candidates = Vec::new();
    let mut remaining = PIPE_CANDIDATE_LINE_LIMIT;
    for root in roots {
        if remaining == 0 {
            break;
        }
        append_candidates(
            project_root,
            &root,
            extensions,
            &terms,
            &mut candidates,
            &mut remaining,
        )?;
    }
    Ok(candidates)
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == ',' || character == '|' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn language_extensions(language_id: &str) -> &'static [&'static str] {
    match language_id {
        "rust" => &["rs"],
        "typescript" => &["ts", "tsx", "js", "jsx"],
        "python" => &["py"],
        "julia" => &["jl"],
        _ => &[],
    }
}

fn append_candidates(
    project_root: &Path,
    root: &Path,
    extensions: &[&str],
    terms: &[String],
    candidates: &mut Vec<Candidate>,
    remaining: &mut usize,
) -> Result<(), String> {
    if *remaining == 0 || !root.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(root).map_err(|error| {
        format!(
            "failed to inspect search pipe root {}: {error}",
            root.display()
        )
    })?;
    if metadata.is_file() {
        append_file_candidates(project_root, root, extensions, terms, candidates, remaining)?;
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|error| {
        format!(
            "failed to read search pipe root {}: {error}",
            root.display()
        )
    })? {
        if *remaining == 0 {
            break;
        }
        let entry = entry.map_err(|error| {
            format!(
                "failed to read search pipe entry under {}: {error}",
                root.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect search pipe path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            append_candidates(
                project_root,
                &path,
                extensions,
                terms,
                candidates,
                remaining,
            )?;
        } else if file_type.is_file() {
            append_file_candidates(
                project_root,
                &path,
                extensions,
                terms,
                candidates,
                remaining,
            )?;
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.starts_with('.')
        || matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | "__pycache__" | "vendor"
        )
}

fn append_file_candidates(
    project_root: &Path,
    path: &Path,
    extensions: &[&str],
    terms: &[String],
    candidates: &mut Vec<Candidate>,
    remaining: &mut usize,
) -> Result<(), String> {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return Ok(());
    };
    if !extensions.contains(&extension) {
        return Ok(());
    }
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };
    for (index, line) in text.lines().enumerate() {
        if *remaining == 0 {
            break;
        }
        let lower = line.to_lowercase();
        if !terms.iter().any(|term| lower.contains(term)) {
            continue;
        }
        candidates.push(Candidate {
            path: display_path(project_root, path),
            line: index + 1,
            symbol: terms
                .iter()
                .find(|term| lower.contains(term.as_str()))
                .cloned()
                .unwrap_or_else(|| "match".to_string()),
            text: line.to_string(),
        });
        *remaining -= 1;
    }
    Ok(())
}

fn render_ingest_frontier(candidates: &[Candidate], pipes: &[String]) -> String {
    let mut owners = unique_candidate_paths(candidates);
    if owners.is_empty() {
        owners.push(".".to_string());
    }
    let include_tests = pipes.is_empty() || pipes.iter().any(|pipe| pipe == "tests");
    let mut rendered = String::from(
        "[search-ingest] root=. alg=asp-fast-seed-frontier-v1\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,O=owner,T=test,S=symbol}\n",
    );
    for (index, owner) in owners.iter().enumerate() {
        let owner_id = numbered_id("O", index);
        let _ = write!(rendered, "{owner_id}=owner:path({owner})!owner;");
    }
    if include_tests {
        for (index, owner) in owners.iter().enumerate() {
            let test_id = numbered_id("T", index);
            let _ = write!(rendered, "{test_id}=test:path({owner})!tests;");
        }
    }
    for (index, candidate) in candidates.iter().take(12).enumerate() {
        let symbol_id = numbered_id("S", index);
        let _ = write!(
            rendered,
            "{symbol_id}=symbol:symbol({})@{}:{}:{}!symbol;",
            candidate.symbol, candidate.path, candidate.line, candidate.line
        );
    }
    rendered.push('\n');
    let mut edge_targets = Vec::new();
    for index in 0..owners.len() {
        edge_targets.push(format!("{}:selects", numbered_id("O", index)));
    }
    if include_tests {
        for index in 0..owners.len() {
            edge_targets.push(format!("{}:covers", numbered_id("T", index)));
        }
    }
    for index in 0..candidates.iter().take(12).count() {
        edge_targets.push(format!("{}:contains", numbered_id("S", index)));
    }
    let _ = writeln!(rendered, "G>{{{}}}", edge_targets.join(","));
    let mut rank = Vec::new();
    let mut frontier = Vec::new();
    for index in 0..owners.len() {
        let id = numbered_id("O", index);
        rank.push(id.clone());
        frontier.push(format!("{id}.owner"));
    }
    if include_tests {
        for index in 0..owners.len() {
            let id = numbered_id("T", index);
            rank.push(id.clone());
            frontier.push(format!("{id}.tests"));
        }
    }
    for index in 0..candidates.iter().take(12).count() {
        let id = numbered_id("S", index);
        rank.push(id.clone());
        frontier.push(format!("{id}.symbol"));
    }
    let _ = writeln!(
        rendered,
        "rank={} frontier={}",
        rank.join(","),
        frontier.join(",")
    );
    rendered.push_str("entries=owner-tests(O=>covering-tests+test-entrypoints+fixtures)\n");
    rendered
}

fn render_owner_query_frontier(project_root: &Path, args: &OwnerQueryArgs) -> String {
    let owner_path = if args.owner.is_absolute() {
        args.owner.clone()
    } else {
        project_root.join(&args.owner)
    };
    let display_owner = display_path(project_root, &owner_path);
    let (item_start, item_end) = find_first_term_range(&owner_path, &args.query).unwrap_or((1, 1));
    let mut rendered = String::from(
        "[search-reasoning] q=owner-query alg=asp-fast-owner-query-v1\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,Q=query,T=test,O=owner,I=item}\n",
    );
    let _ = writeln!(
        rendered,
        "Q=query:term({})!query;T=test:path({})!tests;O=owner:path({})!owner;I=item:symbol({})@{}:{}:{}!code",
        args.query, display_owner, display_owner, args.query, display_owner, item_start, item_end
    );
    rendered.push_str("G>{Q:matches,T:covers,O:selects,I:contains}\n");
    rendered.push_str("rank=Q,T,O,I frontier=Q.query,T.tests,O.owner,I.code\n");
    rendered.push_str("entries=owner-query(O,Q=>items+tests+dependency-usage),owner-tests(O=>covering-tests+test-entrypoints+fixtures)\n");
    rendered
}

fn unique_candidate_paths(candidates: &[Candidate]) -> Vec<String> {
    let mut paths = Vec::new();
    for candidate in candidates {
        if !paths.contains(&candidate.path) {
            paths.push(candidate.path.clone());
        }
    }
    paths
}

fn numbered_id(prefix: &str, index: usize) -> String {
    if index == 0 {
        prefix.to_string()
    } else {
        format!("{prefix}{}", index + 1)
    }
}

fn find_first_term_range(path: &Path, query: &str) -> Option<(usize, usize)> {
    let text = fs::read_to_string(path).ok()?;
    let terms = query_terms(query);
    let lines: Vec<_> = text.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let lower = line.to_lowercase();
        if terms.iter().any(|term| lower.contains(term)) {
            let start = index + 1;
            return Some((
                start,
                python_block_end(path, &lines, index).unwrap_or(start),
            ));
        }
    }
    None
}

fn python_block_end(path: &Path, lines: &[&str], start_index: usize) -> Option<usize> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("py") {
        return None;
    }
    let line = lines.get(start_index)?;
    let trimmed = line.trim_start();
    if !(trimmed.starts_with("def ")
        || trimmed.starts_with("async def ")
        || trimmed.starts_with("class "))
    {
        return None;
    }
    let indent = line.len().saturating_sub(trimmed.len());
    let mut end = start_index + 1;
    for (index, next_line) in lines.iter().enumerate().skip(start_index + 1) {
        let next_trimmed = next_line.trim();
        if next_trimmed.is_empty() {
            continue;
        }
        let next_indent = next_line.len().saturating_sub(next_line.trim_start().len());
        if next_indent <= indent {
            break;
        }
        end = index + 1;
    }
    Some(end)
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
