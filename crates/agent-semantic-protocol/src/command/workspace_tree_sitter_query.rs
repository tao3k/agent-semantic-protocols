use std::path::Path;

use agent_semantic_hook::ActivatedProvider;
use serde_json::json;

const MAX_RETAINED_CAPTURES: usize = 256;

#[derive(Debug)]
struct WorkspaceTreeSitterRequest {
    query_source: String,
    json: bool,
}

struct WorkspaceTreeSitterCapture {
    owner_path: String,
    capture_name: String,
    node_kind: String,
    start_line: usize,
    end_line: usize,
}

pub(super) fn try_run_workspace_tree_sitter_query(
    language_id: &str,
    args: &[String],
    project_root: &Path,
    provider: &ActivatedProvider,
) -> Result<bool, String> {
    let Some(request) = WorkspaceTreeSitterRequest::parse(args)? else {
        return Ok(false);
    };
    let language = agent_semantic_tree_sitter::registered_language_grammar(language_id.into())?;
    let query =
        agent_semantic_tree_sitter::compile_native_query_source(&language, &request.query_source)?;
    if !query.unsupported_predicates().is_empty() {
        return Err(format!(
            "tree-sitter query uses unsupported predicates: {}",
            query.unsupported_predicates().join(",")
        ));
    }
    let snapshot =
        agent_semantic_client::source_index::current_workspace_search_source_index_snapshot(
            project_root,
        )?;
    let (captures, total_captures) = collect_workspace_captures(
        &language,
        &query,
        &snapshot.source_blobs,
        &provider.source_extensions,
    )?;
    render_workspace_query(
        language_id,
        &request,
        project_root,
        captures,
        total_captures,
    )?;
    Ok(true)
}

impl WorkspaceTreeSitterRequest {
    fn parse(args: &[String]) -> Result<Option<Self>, String> {
        if has_exact_selector(args) {
            return Ok(None);
        }
        let Some(query_source) = option_value(args, "--treesitter-query")? else {
            return Ok(None);
        };
        match args.first().map(String::as_str) {
            Some("search") => {}
            Some("query") => {
                return Err(
                    "workspace Tree-sitter discovery is search-owned; use `asp <language> search --treesitter-query <QUERY> --workspace <ROOT>`, or add an exact `--selector` for deterministic query projection"
                        .to_string(),
                );
            }
            _ => return Ok(None),
        }
        if args.iter().any(|argument| argument == "--code") {
            return Err(
                "tree-sitter search returns a capture frontier and does not accept --code; use `query --selector <exact-selector> --code` for deterministic source projection"
                    .to_string(),
            );
        }
        Ok(Some(Self {
            query_source,
            json: args.iter().any(|argument| argument == "--json"),
        }))
    }
}

fn has_exact_selector(args: &[String]) -> bool {
    args.iter()
        .any(|argument| argument == "--selector" || argument.starts_with("--selector="))
}

fn option_value(args: &[String], option: &str) -> Result<Option<String>, String> {
    let mut values = args.iter();
    while let Some(argument) = values.next() {
        if argument == option {
            return values
                .next()
                .cloned()
                .map(Some)
                .ok_or_else(|| format!("missing value after {option}"));
        }
        if let Some(value) = argument.strip_prefix(&format!("{option}=")) {
            return Ok(Some(value.to_string()));
        }
    }
    Ok(None)
}

pub(super) fn infer_workspace_tree_sitter_search_language(
    args: &[String],
    project_root: &Path,
    providers: &[ActivatedProvider],
) -> Result<Option<String>, String> {
    let Some(query_source) = option_value(args, "--treesitter-query")? else {
        return Ok(None);
    };
    let snapshot =
        agent_semantic_client::source_index::current_source_index_snapshot(project_root)?;
    let mut compatible_languages = std::collections::BTreeSet::new();
    let mut matching_languages = std::collections::BTreeSet::new();

    for provider in providers {
        let language_id = provider.language_id.as_str();
        let Ok(language) =
            agent_semantic_tree_sitter::registered_language_grammar(language_id.into())
        else {
            continue;
        };
        let Ok(query) =
            agent_semantic_tree_sitter::compile_native_query_source(&language, &query_source)
        else {
            continue;
        };
        if !query.unsupported_predicates().is_empty() {
            continue;
        }
        compatible_languages.insert(language_id.to_string());
        let (_, total_captures) = collect_workspace_captures(
            &language,
            &query,
            &snapshot.source_blobs,
            &provider.source_extensions,
        )?;
        if total_captures > 0 {
            matching_languages.insert(language_id.to_string());
        }
    }

    match matching_languages.len() {
        1 => Ok(matching_languages.into_iter().next()),
        2.. => Err(format!(
            "tree-sitter search matched multiple active languages: {}; add `--language <language>`",
            matching_languages.into_iter().collect::<Vec<_>>().join("|"),
        )),
        _ if compatible_languages.len() == 1 => Ok(compatible_languages.into_iter().next()),
        _ if compatible_languages.is_empty() => Err(
            "tree-sitter search pattern did not compile for any active language; add `--language <language>` to select the intended grammar"
                .to_string(),
        ),
        _ => Err(format!(
            "tree-sitter search language is ambiguous across active grammars: {}; add `--language <language>`",
            compatible_languages
                .into_iter()
                .collect::<Vec<_>>()
                .join("|"),
        )),
    }
}

fn collect_workspace_captures(
    language: &tree_sitter::Language,
    query: &agent_semantic_tree_sitter::CompiledNativeSyntaxQuery,
    source_blobs: &agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
    source_extensions: &[String],
) -> Result<(Vec<WorkspaceTreeSitterCapture>, usize), String> {
    source_blobs
        .iter()
        .filter(|(owner_path, _)| registered_source_path(owner_path, source_extensions))
        .try_fold(
            (Vec::new(), 0usize),
            |(retained, total), (owner_path, source)| {
                let source = std::str::from_utf8(source).map_err(|error| {
                    format!("tree-sitter source is not UTF-8: {owner_path}: {error}")
                })?;
                let execution =
                    agent_semantic_tree_sitter::execute_native_query(language, query, source)?;
                Ok(execution
                    .matches
                    .into_iter()
                    .flat_map(|item| item.captures)
                    .fold((retained, total), |(mut retained, total), capture| {
                        if retained.len() < MAX_RETAINED_CAPTURES {
                            retained.push(WorkspaceTreeSitterCapture {
                                owner_path: owner_path.to_string(),
                                capture_name: capture.capture_name,
                                node_kind: capture.node.node_kind.as_str().to_string(),
                                start_line: capture.node.start_line,
                                end_line: capture.node.end_line,
                            });
                        }
                        (retained, total + 1)
                    }))
            },
        )
}

fn registered_source_path(owner_path: &str, source_extensions: &[String]) -> bool {
    let Some(extension) = Path::new(owner_path)
        .extension()
        .and_then(|value| value.to_str())
    else {
        return false;
    };
    source_extensions
        .iter()
        .any(|registered| registered.trim_start_matches('.') == extension)
}

fn render_workspace_query(
    language_id: &str,
    request: &WorkspaceTreeSitterRequest,
    project_root: &Path,
    captures: Vec<WorkspaceTreeSitterCapture>,
    total_captures: usize,
) -> Result<(), String> {
    let native_fact_refs = captures
        .iter()
        .map(|capture| capture.native_fact_ref(language_id))
        .collect::<Vec<_>>();
    if request.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
                "schemaVersion": "1",
                "operation": "search",
                "adapterMode": "native-projection",
                "compatibilityLevel": "native-only",
                "selector": format!("workspace:{}", project_root.display()),
                "nativeFactRefs": native_fact_refs,
                "matchCount": total_captures,
                "retainedMatchCount": captures.len(),
                "truncated": total_captures > captures.len(),
                "cache": { "rawSourceStored": false }
            }))
            .map_err(|error| format!("failed to render tree-sitter query JSON: {error}"))?
        );
        return Ok(());
    }
    println!(
        "{}",
        super::tree_sitter_query_diagnostics::render_search_summary(
            language_id,
            total_captures,
            captures.len(),
        )
    );
    if total_captures == 0 {
        super::tree_sitter_query_diagnostics::render_search_miss_guidance(language_id)
            .iter()
            .for_each(|line| println!("{line}"));
        return Ok(());
    }
    println!(
        "{}",
        super::tree_sitter_query_diagnostics::render_search_match_guidance()
    );
    captures
        .iter()
        .for_each(|capture| println!("{}", capture.compact_line(language_id)));
    Ok(())
}

impl WorkspaceTreeSitterCapture {
    fn native_fact_ref(&self, language_id: &str) -> String {
        format!(
            "{}:syntax:{}:{}:{}:{}:{}",
            language_id,
            self.owner_path,
            self.start_line,
            self.end_line,
            self.node_kind,
            self.capture_name
        )
    }

    fn compact_line(&self, language_id: &str) -> String {
        format!(
            "I=syntax:{}/{}@{}:{}:{}!code nativeFactRef={}",
            self.node_kind,
            self.capture_name,
            self.owner_path,
            self.start_line,
            self.end_line,
            self.native_fact_ref(language_id)
        )
    }
}

#[cfg(test)]
#[path = "../../tests/unit/command/workspace_tree_sitter_query.rs"]
mod tests;
