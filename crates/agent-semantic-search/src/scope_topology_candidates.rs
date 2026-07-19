use crate::{
    SearchPipeCandidate, SearchPipeSourceAcquisition, SearchPipeSourceAcquisitionTrace,
    SemanticWorkspaceScope,
};
use ignore::{DirEntry, WalkBuilder};
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

pub const SEARCH_PIPE_SCOPE_TOPOLOGY_ENTRY_VISIT_LIMIT: usize = 256;
pub const SEARCH_PIPE_SCOPE_TOPOLOGY_CANDIDATE_LIMIT: usize = 12;
pub const SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE: &str = "workspace-scope-topology";

pub struct SearchPipeScopeTopologyAcquisitionRequest<'a> {
    pub workspace_scope: &'a SemanticWorkspaceScope,
    pub locator_root: &'a Path,
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub entry_visit_limit: usize,
    pub candidate_limit: usize,
}

pub fn collect_search_pipe_scope_topology_acquisition(
    request: SearchPipeScopeTopologyAcquisitionRequest<'_>,
) -> Result<SearchPipeSourceAcquisition, String> {
    let started = Instant::now();
    let extensions = normalized_source_extensions(&request);
    let mut package_roots = request
        .workspace_scope
        .packages
        .iter()
        .map(|package| package.root.clone())
        .collect::<Vec<_>>();
    package_roots.sort();
    package_roots.dedup();

    let mut candidates = Vec::with_capacity(request.candidate_limit);
    let mut seen_paths = BTreeSet::new();
    let mut visited = 0usize;
    let mut truncated = false;

    'roots: for package_root in package_roots {
        let mut builder = WalkBuilder::new(&package_root);
        builder.hidden(false);
        builder.sort_by_file_path(|left, right| left.cmp(right));
        builder.filter_entry(scope_entry_filter(
            request.ignore_dirs.to_vec(),
            request.include_hidden_dirs.to_vec(),
        ));
        for entry in builder.build() {
            let entry = entry.map_err(|error| {
                format!(
                    "workspace scope topology walk failed under {}: {error}",
                    package_root.display()
                )
            })?;
            if entry.depth() == 0 {
                continue;
            }
            if visited >= request.entry_visit_limit || candidates.len() >= request.candidate_limit {
                truncated = true;
                break 'roots;
            }
            visited += 1;
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
                || !matches_scope_source_extension(entry.path(), &extensions)
            {
                continue;
            }
            let path = candidate_path(entry.path(), request.locator_root);
            if !seen_paths.insert(path.clone()) {
                continue;
            }
            let symbol = entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("source")
                .to_string();
            candidates.push(SearchPipeCandidate {
                path: path.clone(),
                line: 1,
                end_line: 1,
                symbol,
                text: path,
                source: SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE.to_string(),
                confidence: "scope-exact".to_string(),
            });
        }
    }

    let status = if truncated {
        "truncated"
    } else if candidates.is_empty() {
        "empty"
    } else {
        "used"
    };
    let matched = candidates.len();
    Ok(SearchPipeSourceAcquisition {
        candidates,
        candidate_sources: vec![SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE.to_string()],
        source_trace: vec![SearchPipeSourceAcquisitionTrace {
            source: SEARCH_PIPE_SCOPE_TOPOLOGY_SOURCE.to_string(),
            status: status.to_string(),
            matched,
            missing: visited.saturating_sub(matched),
            normalized: visited,
            elapsed: Some(started.elapsed()),
        }],
    })
}

pub fn merge_search_pipe_source_acquisitions(
    mut primary: SearchPipeSourceAcquisition,
    secondary: SearchPipeSourceAcquisition,
) -> SearchPipeSourceAcquisition {
    let mut seen = primary
        .candidates
        .iter()
        .map(candidate_identity)
        .collect::<BTreeSet<_>>();
    primary.candidates.extend(
        secondary
            .candidates
            .into_iter()
            .filter(|candidate| seen.insert(candidate_identity(candidate))),
    );
    for source in secondary.candidate_sources {
        if !primary.candidate_sources.contains(&source) {
            primary.candidate_sources.push(source);
        }
    }
    primary.source_trace.extend(secondary.source_trace);
    primary
}

fn candidate_identity(candidate: &SearchPipeCandidate) -> (String, usize, usize, String) {
    (
        candidate.path.clone(),
        candidate.line,
        candidate.end_line,
        candidate.symbol.clone(),
    )
}

fn normalized_source_extensions(
    request: &SearchPipeScopeTopologyAcquisitionRequest<'_>,
) -> BTreeSet<String> {
    request
        .workspace_scope
        .source_extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .filter(|extension| !extension.is_empty())
        .collect()
}

fn matches_scope_source_extension(path: &Path, extensions: &BTreeSet<String>) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extensions.contains(&extension.to_ascii_lowercase()))
}

fn candidate_path(path: &Path, locator_root: &Path) -> String {
    path.strip_prefix(locator_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn scope_entry_filter(
    ignore_dirs: Vec<String>,
    include_hidden_dirs: Vec<String>,
) -> impl Fn(&DirEntry) -> bool + Send + Sync + 'static {
    move |entry| {
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            return true;
        }
        !should_skip_scope_dir(entry, &ignore_dirs, &include_hidden_dirs)
    }
}

fn should_skip_scope_dir(
    entry: &DirEntry,
    ignore_dirs: &[String],
    include_hidden_dirs: &[String],
) -> bool {
    if entry.depth() == 0
        || !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
    {
        return false;
    }
    let Some(name) = entry.path().file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name.starts_with('.') && !include_hidden_dirs.iter().any(|dir| dir == name) {
        return true;
    }
    ignore_dirs.iter().any(|dir| dir == name)
}
