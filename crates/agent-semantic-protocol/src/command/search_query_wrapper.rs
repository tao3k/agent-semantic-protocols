//! ASP-owned `fd -query` and `rg -query` query-set wrappers.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::graph::render_graph_turbo_packet;
use super::search_config::AspConfig;
use super::search_pipe_graph_turbo::{GraphTurboSearchPipeRequest, render_graph_turbo_request};
use super::search_pipe_plan::render_primary_frontier_actions_only;
use super::search_pipe_provider_facts::ProviderGraphFacts;
use super::search_pipe_render::{
    Candidate, SearchPipeSourceTrace, default_search_surfaces, render_ingest_frontier,
};

const QUERY_CANDIDATE_LIMIT: usize = 256;
const SUPPORTED_EXTENSIONS: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "py", "jl"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueryWrapperSurface {
    Fd,
    Rg,
}

impl QueryWrapperSurface {
    fn from_command(command: &str) -> Option<Self> {
        match command {
            "fd" => Some(Self::Fd),
            "rg" => Some(Self::Rg),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Fd => "fd",
            Self::Rg => "rg",
        }
    }

    fn graph_surface(self) -> &'static str {
        match self {
            Self::Fd => "search-fd",
            Self::Rg => "search-rg",
        }
    }

    fn source_name(self) -> &'static str {
        "finder"
    }

    fn next_classes(self) -> &'static str {
        match self {
            Self::Fd => "owner-items,rg-query,query-selector",
            Self::Rg => "query-selector,owner-items,fd-query",
        }
    }

    fn avoid(self) -> &'static str {
        match self {
            Self::Fd => "repeat-fd,raw-read",
            Self::Rg => "repeat-rg,manual-window-scan,raw-read",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct QueryWrapperArgs {
    query: String,
    scopes: Vec<PathBuf>,
    view: String,
    native_args: Vec<String>,
}

pub(crate) fn is_query_wrapper(command: &str) -> bool {
    QueryWrapperSurface::from_command(command).is_some()
}

pub(crate) fn run_query_wrapper_command(command: &str, args: &[String]) -> Result<(), String> {
    let surface = QueryWrapperSurface::from_command(command)
        .ok_or_else(|| format!("unsupported query wrapper `{command}`"))?;
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{}", query_wrapper_usage(surface));
        return Ok(());
    }
    let wrapper_args = parse_query_wrapper_args(surface, args)?;
    let invocation_root =
        std::env::current_dir().map_err(|error| format!("failed to read cwd: {error}"))?;
    let project_root = wrapper_args
        .scopes
        .first()
        .map(|scope| absolute_scope(&invocation_root, scope))
        .unwrap_or_else(|| invocation_root.clone());
    let config = AspConfig::load(&invocation_root, &project_root);
    let terms = query_terms(&wrapper_args.query);
    let candidates = collect_query_candidates(
        surface,
        &project_root,
        &invocation_root,
        &wrapper_args.scopes,
        &terms,
        &config,
    )?;
    print_query_wrapper_view(
        surface,
        &project_root,
        &wrapper_args.query,
        &terms,
        &candidates,
        &wrapper_args.view,
        &wrapper_args.native_args,
    )
}

fn query_wrapper_usage(surface: QueryWrapperSurface) -> String {
    match surface {
        QueryWrapperSurface::Fd => {
            "usage: asp fd -query <owner-or-path-term-a|term-b|term-c> [scope...] [-- native-fd-argv...]\n\nFinds owner/path/module candidates from an LLM-generated grouped query-set.".to_string()
        }
        QueryWrapperSurface::Rg => {
            "usage: asp rg -query <content-or-error-term-a|term-b|term-c> [scope...] [-- native-rg-argv...]\n\nFinds content/hot-block candidates from an LLM-generated grouped query-set.".to_string()
        }
    }
}

fn parse_query_wrapper_args(
    surface: QueryWrapperSurface,
    args: &[String],
) -> Result<QueryWrapperArgs, String> {
    let mut query = None;
    let mut scopes = Vec::new();
    let mut view = "seeds".to_string();
    let mut native_args = Vec::new();
    let mut index = 0;
    let mut native = false;
    while index < args.len() {
        let arg = &args[index];
        if native {
            native_args.push(arg.clone());
            index += 1;
            continue;
        }
        match arg.as_str() {
            "--" => {
                native = true;
                index += 1;
            }
            "-query" | "--query" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("asp {} -query requires a value", surface.label()))?;
                query = Some(value.clone());
                index += 2;
            }
            "--view" => {
                view = args
                    .get(index + 1)
                    .ok_or_else(|| "--view requires a value".to_string())?
                    .clone();
                index += 2;
            }
            value if value.starts_with("-query=") => {
                query = Some(value.trim_start_matches("-query=").to_string());
                index += 1;
            }
            value if value.starts_with("--query=") => {
                query = Some(value.trim_start_matches("--query=").to_string());
                index += 1;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "unknown asp {} option: {value} (native flags must follow --)",
                    surface.label()
                ));
            }
            value => {
                scopes.push(PathBuf::from(value));
                index += 1;
            }
        }
    }
    if !matches!(view.as_str(), "seeds" | "graph-turbo-request") {
        return Err(format!(
            "asp {} -query supports --view seeds or --view graph-turbo-request",
            surface.label()
        ));
    }
    Ok(QueryWrapperArgs {
        query: query
            .ok_or_else(|| format!("asp {} requires -query <query-set>", surface.label()))?,
        scopes,
        view,
        native_args,
    })
}

fn print_query_wrapper_view(
    surface: QueryWrapperSurface,
    project_root: &Path,
    query: &str,
    terms: &[String],
    candidates: &[Candidate],
    view: &str,
    native_args: &[String],
) -> Result<(), String> {
    let language_id = infer_language_id(project_root);
    let pipes = default_search_surfaces();
    let source_trace = vec![SearchPipeSourceTrace::new(
        surface.source_name(),
        if candidates.is_empty() {
            "empty"
        } else {
            "used"
        },
        candidates.len(),
        usize::from(candidates.is_empty()),
        candidates.len(),
    )];
    let request = render_graph_turbo_request(GraphTurboSearchPipeRequest {
        surface: surface.graph_surface(),
        language_id,
        query: Some(query),
        candidates,
        pipes: &pipes,
        source: "finder",
        candidate_sources: &["finder".to_string()],
        source_trace: &source_trace,
        provider_facts: &ProviderGraphFacts::default(),
        read_memory_selectors: &[],
    })?;
    if view == "graph-turbo-request" {
        print!("{request}");
        return Ok(());
    }
    println!(
        "[search-{}] view=seeds querySet={} source=finder ranker=graph-turbo:owner-query",
        surface.label(),
        terms.len(),
    );
    println!("query={query}");
    println!("terms={}", display_terms(terms));
    if !native_args.is_empty() {
        println!("nativeArgs=pass-through count={}", native_args.len());
    }
    if let Some(output) = render_graph_turbo_packet(request.as_bytes())? {
        if let Ok(compact) = std::str::from_utf8(output.as_ref()) {
            print!("{}", render_primary_frontier_actions_only(compact));
        } else {
            io::stdout()
                .write_all(output.as_ref())
                .map_err(|error| format!("failed to write asp-graph-turbo stdout: {error}"))?;
        }
    } else {
        print!("{}", render_ingest_frontier(candidates, &pipes));
    }
    println!("nextClasses={}", surface.next_classes());
    println!("avoid={}", surface.avoid());
    Ok(())
}

fn collect_query_candidates(
    surface: QueryWrapperSurface,
    project_root: &Path,
    locator_root: &Path,
    scopes: &[PathBuf],
    terms: &[String],
    config: &AspConfig,
) -> Result<Vec<Candidate>, String> {
    if terms.is_empty() {
        return Err(format!(
            "asp {} -query requires non-empty terms",
            surface.label()
        ));
    }
    let roots = if scopes.is_empty() {
        vec![project_root.to_path_buf()]
    } else {
        scopes
            .iter()
            .map(|scope| absolute_scope(locator_root, scope))
            .collect()
    };
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
            break;
        }
        append_query_candidates(
            surface,
            locator_root,
            &root,
            terms,
            config,
            &mut seen,
            &mut candidates,
        )?;
    }
    Ok(candidates)
}

fn append_query_candidates(
    surface: QueryWrapperSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    config: &AspConfig,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) -> Result<(), String> {
    if candidates.len() >= QUERY_CANDIDATE_LIMIT || !path.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "failed to inspect query wrapper path {}: {error}",
            path.display()
        )
    })?;
    if metadata.is_file() {
        append_file_query_candidates(surface, locator_root, path, terms, seen, candidates);
        return Ok(());
    }
    let mut entries = fs::read_dir(path)
        .map_err(|error| {
            format!(
                "failed to read query wrapper dir {}: {error}",
                path.display()
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read query wrapper entry under {}: {error}",
                path.display()
            )
        })?;
    entries.sort_by_key(|entry| path_priority(&entry.path()));
    for entry in entries {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
            break;
        }
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect query wrapper path {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_dir() {
            if should_skip_dir(&path, config) {
                continue;
            }
            append_query_candidates(
                surface,
                locator_root,
                &path,
                terms,
                config,
                seen,
                candidates,
            )?;
        } else if file_type.is_file() {
            append_file_query_candidates(surface, locator_root, &path, terms, seen, candidates);
        }
    }
    Ok(())
}

fn append_file_query_candidates(
    surface: QueryWrapperSurface,
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return;
    };
    if !SUPPORTED_EXTENSIONS.contains(&extension) {
        return;
    }
    match surface {
        QueryWrapperSurface::Fd => {
            append_path_candidate(locator_root, path, terms, seen, candidates)
        }
        QueryWrapperSurface::Rg => {
            append_content_candidates(locator_root, path, terms, seen, candidates)
        }
    }
}

fn append_path_candidate(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let display = display_path(locator_root, path);
    let lower = display.to_ascii_lowercase();
    let Some(term) = terms.iter().find(|term| lower.contains(term.as_str())) else {
        return;
    };
    let key = format!("{display}:1:{term}");
    if !seen.insert(key) {
        return;
    }
    candidates.push(Candidate {
        path: display.clone(),
        line: 1,
        symbol: term.clone(),
        text: display,
        source: "fd-query".to_string(),
        confidence: "path".to_string(),
    });
}

fn append_content_candidates(
    locator_root: &Path,
    path: &Path,
    terms: &[String],
    seen: &mut HashSet<String>,
    candidates: &mut Vec<Candidate>,
) {
    let Ok(bytes) = fs::read(path) else {
        return;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return;
    };
    for (line_index, line) in text.lines().enumerate() {
        if candidates.len() >= QUERY_CANDIDATE_LIMIT {
            break;
        }
        let lower = line.to_ascii_lowercase();
        let Some(term) = terms.iter().find(|term| lower.contains(term.as_str())) else {
            continue;
        };
        let display = display_path(locator_root, path);
        let line_number = line_index + 1;
        let key = format!("{display}:{line_number}:{term}");
        if !seen.insert(key) {
            continue;
        }
        candidates.push(Candidate {
            path: display,
            line: line_number,
            symbol: term.clone(),
            text: line.to_string(),
            source: "rg-query".to_string(),
            confidence: "content".to_string(),
        });
    }
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term);
            }
            terms
        })
}

fn display_terms(terms: &[String]) -> String {
    if terms.is_empty() {
        "-".to_string()
    } else {
        terms.join(",")
    }
}

fn infer_language_id(root: &Path) -> &'static str {
    if root.join("Cargo.toml").exists() {
        "rust"
    } else if root.join("tsconfig.json").exists() || root.join("package.json").exists() {
        "typescript"
    } else if root.join("pyproject.toml").exists() {
        "python"
    } else if root.join("Project.toml").exists() {
        "julia"
    } else {
        "unknown"
    }
}

fn absolute_scope(root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    }
}

fn path_priority(path: &Path) -> (u8, String) {
    let display = path.to_string_lossy().replace('\\', "/");
    let priority = if display.ends_with("/src") || display.contains("/src/") {
        0
    } else if display.contains("/test") || display.contains("/examples/") {
        2
    } else {
        1
    };
    (priority, display)
}

fn should_skip_dir(path: &Path, config: &AspConfig) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.')
        && !config
            .search
            .include_hidden_dirs
            .iter()
            .any(|dir| dir == name)
    {
        return true;
    }
    config.search.ignore_dirs.iter().any(|dir| dir == name)
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
