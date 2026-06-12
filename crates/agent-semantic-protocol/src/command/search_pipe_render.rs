//! Compact frontier rendering for ASP-owned search helpers.

use std::fmt::Write;
use std::path::Path;

use super::search_pipe_model::Candidate;

pub(super) fn render_ingest_frontier(candidates: &[Candidate], pipes: &[String]) -> String {
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
    append_ingest_nodes(&mut rendered, &owners, candidates, include_tests);
    append_ingest_edges(
        &mut rendered,
        owners.len(),
        candidate_symbol_count(candidates),
        include_tests,
    );
    append_ingest_rank_frontier(
        &mut rendered,
        owners.len(),
        candidate_symbol_count(candidates),
        include_tests,
    );
    rendered.push_str(&format!("entries={}\n", ingest_entries_for_pipes(pipes)));
    rendered
}

pub(super) fn render_empty_ingest_diagnostic(language_id: &str) -> String {
    format!(
        "[search-ingest] root=. alg=asp-fast-seed-frontier-v1\n\
|note kind=stdin-required message=\"search ingest requires candidate stdin; no provider full report was started\"\n\
|next prime: asp {language_id} search prime --workspace . --view seeds\n"
    )
}

pub(super) fn render_owner_tests_frontier(
    project_root: &Path,
    locator_root: &Path,
    owner: &Path,
) -> String {
    let owner_path = if owner.is_absolute() {
        owner.to_path_buf()
    } else {
        project_root.join(owner)
    };
    let display_owner = display_path(locator_root, &owner_path);
    let mut rendered = String::from(
        "[search-reasoning] q=owner-tests alg=asp-fast-owner-tests-v1\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases: graph:{G=search,T=test,O=owner}\n",
    );
    let _ = writeln!(
        rendered,
        "O=owner:path({display_owner})!owner;T=test:path({display_owner})!tests"
    );
    rendered.push_str("G>{O:selects,T:covers}\n");
    rendered.push_str("rank=O,T frontier=O.owner,T.tests\n");
    rendered.push_str("entries=owner-tests(O=>covering-tests+test-entrypoints+fixtures)\n");
    rendered
}

fn append_ingest_nodes(
    rendered: &mut String,
    owners: &[String],
    candidates: &[Candidate],
    include_tests: bool,
) {
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
}

fn append_ingest_edges(
    rendered: &mut String,
    owner_count: usize,
    symbol_count: usize,
    include_tests: bool,
) {
    let mut edge_targets = numbered_ids("O", owner_count)
        .into_iter()
        .map(|id| format!("{id}:selects"))
        .collect::<Vec<_>>();
    if include_tests {
        edge_targets.extend(
            numbered_ids("T", owner_count)
                .into_iter()
                .map(|id| format!("{id}:covers")),
        );
    }
    edge_targets.extend(
        numbered_ids("S", symbol_count)
            .into_iter()
            .map(|id| format!("{id}:contains")),
    );
    let _ = writeln!(rendered, "G>{{{}}}", edge_targets.join(","));
}

fn append_ingest_rank_frontier(
    rendered: &mut String,
    owner_count: usize,
    symbol_count: usize,
    include_tests: bool,
) {
    let owner_ids = numbered_ids("O", owner_count);
    let test_ids = if include_tests {
        numbered_ids("T", owner_count)
    } else {
        Vec::new()
    };
    let symbol_ids = numbered_ids("S", symbol_count);
    let rank = owner_ids
        .iter()
        .chain(test_ids.iter())
        .chain(symbol_ids.iter())
        .cloned()
        .collect::<Vec<_>>();
    let frontier = owner_ids
        .iter()
        .map(|id| format!("{id}.owner"))
        .chain(test_ids.iter().map(|id| format!("{id}.tests")))
        .chain(symbol_ids.iter().map(|id| format!("{id}.symbol")))
        .collect::<Vec<_>>();
    let _ = writeln!(
        rendered,
        "rank={} frontier={}",
        rank.join(","),
        frontier.join(",")
    );
}

fn unique_candidate_paths(candidates: &[Candidate]) -> Vec<String> {
    candidates.iter().fold(Vec::new(), |mut paths, candidate| {
        if !paths.contains(&candidate.path) {
            paths.push(candidate.path.clone());
        }
        paths
    })
}

fn numbered_ids(prefix: &str, count: usize) -> Vec<String> {
    (0..count).map(|index| numbered_id(prefix, index)).collect()
}

fn numbered_id(prefix: &str, index: usize) -> String {
    if index == 0 {
        prefix.to_string()
    } else {
        format!("{prefix}{}", index + 1)
    }
}

fn candidate_symbol_count(candidates: &[Candidate]) -> usize {
    candidates.iter().take(12).count()
}

fn ingest_entries_for_pipes(pipes: &[String]) -> String {
    if pipes.is_empty() {
        return "owner-items(O=>candidate-items+symbols),owner-tests(O=>covering-tests+test-entrypoints+fixtures)"
            .to_string();
    }

    let mut entries = Vec::new();
    if pipes.iter().any(|pipe| pipe == "items" || pipe == "owner") {
        entries.push("owner-items(O=>candidate-items+symbols)");
    }
    if pipes.iter().any(|pipe| pipe == "tests") {
        entries.push("owner-tests(O=>covering-tests+test-entrypoints+fixtures)");
    }
    if pipes
        .iter()
        .any(|pipe| pipe == "deps" || pipe == "dependencies")
    {
        entries.push("query-deps(Q=>dependency-usage-owners)");
    }
    if entries.is_empty() {
        entries.push("owner-items(O=>candidate-items+symbols)");
    }
    entries.join(",")
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
