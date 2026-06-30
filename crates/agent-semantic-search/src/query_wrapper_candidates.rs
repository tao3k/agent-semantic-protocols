//! Query-wrapper candidate orchestration.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{
    NativeFinderCollectionRequest, NativeFinderConfig, NativeFinderSurface, QueryCandidateAppend,
    QueryWrapperCandidate, QueryWrapperCandidateSurface, QueryWrapperScanConfig,
    QueryWrapperSourceIndexLookup, QueryWrapperSourceIndexRequest, augment_package_path_candidates,
    collect_native_finder_candidates, collect_query_wrapper_source_index_candidates,
    language_neutral_search_file_spec, query_candidate_priority,
};

/// Search surface for query-wrapper commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryWrapperSearchSurface {
    Fd,
    Rg,
}

impl QueryWrapperSearchSurface {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Fd => "fd",
            Self::Rg => "rg",
        }
    }
}

/// Search-owned representation of one query clause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperSearchClause {
    pub id: usize,
    pub terms: Vec<String>,
    pub axis_terms: Vec<String>,
}

/// Request for query-wrapper candidate collection.
pub struct QueryWrapperSearchRequest<'a> {
    pub surface: QueryWrapperSearchSurface,
    pub project_root: &'a Path,
    pub locator_root: &'a Path,
    pub scopes: &'a [PathBuf],
    pub clauses: &'a [QueryWrapperSearchClause],
    pub terms: &'a [String],
    pub ignore_dirs: &'a [String],
    pub include_hidden_dirs: &'a [String],
    pub native_args: &'a [String],
    pub source_index_lookup: Option<QueryWrapperSourceIndexLookup>,
}

/// Source-index receipt data returned to the protocol renderer.
pub struct QueryWrapperSearchSourceIndexTrace {
    pub lookup: QueryWrapperSourceIndexLookup,
    pub candidate_count: usize,
    pub elapsed: Duration,
}

/// Query-wrapper candidates plus route receipt data.
pub struct QueryWrapperCandidateCollection {
    pub candidates: Vec<QueryWrapperCandidate>,
    pub trace_fields: BTreeMap<String, Value>,
    pub source_index_trace: Option<QueryWrapperSearchSourceIndexTrace>,
    pub finder_skipped_after_source_index: bool,
    pub candidate_sources: Vec<String>,
}

/// Collect candidates for fd/rg query wrappers.
pub fn collect_query_wrapper_candidate_collection(
    request: QueryWrapperSearchRequest<'_>,
) -> Result<QueryWrapperCandidateCollection, String> {
    if request.terms.is_empty() {
        return Err(format!(
            "asp {} -query requires non-empty terms",
            request.surface.label()
        ));
    }
    let roots = resolved_scope_roots(request.locator_root, request.scopes);
    let display_root = display_root(request.locator_root, request.scopes, &roots);
    let native_surface = native_surface(request.surface);
    let accept_all_files = !request.scopes.is_empty();
    let axis_terms = query_axis_terms(request.clauses);
    let config = QueryWrapperScanConfig {
        ignore_dirs: request.ignore_dirs,
        include_hidden_dirs: request.include_hidden_dirs,
    };

    if request.native_args.is_empty()
        && let Some(collection) = collect_source_index_query_candidates(
            request.surface,
            request.project_root,
            &roots,
            request.terms,
            &axis_terms,
            request.source_index_lookup.as_ref(),
        )?
    {
        return Ok(collection);
    }

    if !fd_query_prefers_internal_scan(request.surface, request.terms, request.native_args)
        && let Some(mut collection) =
            collect_native_finder_candidates(NativeFinderCollectionRequest {
                surface: native_surface,
                language_id: "query-wrapper",
                file_spec_override: Some(language_neutral_search_file_spec()),
                accept_all_files,
                project_root: request.project_root,
                locator_root: &display_root,
                roots: &roots,
                terms: request.terms,
                config: NativeFinderConfig {
                    ignore_dirs: request.ignore_dirs,
                    include_hidden_dirs: request.include_hidden_dirs,
                },
                native_args: request.native_args,
            })?
    {
        if collection.candidates.is_empty()
            && request.surface == QueryWrapperSearchSurface::Fd
            && collection.provenance.input_candidate_count() == 0
        {
            return Ok(QueryWrapperCandidateCollection {
                candidates: Vec::new(),
                trace_fields: collection.provenance.trace_fields(0),
                source_index_trace: None,
                finder_skipped_after_source_index: false,
                candidate_sources: vec!["finder".to_string()],
            });
        }
        if !collection.candidates.is_empty() {
            collection.candidates.sort_by_key(|candidate| {
                query_candidate_priority(&candidate.path, request.terms, &axis_terms)
            });
            let mut candidates = cohesive_query_candidates(
                collection
                    .candidates
                    .into_iter()
                    .map(query_wrapper_candidate_from_native)
                    .collect(),
                request.clauses,
            );
            let package_path_augmented_count = augment_package_path_candidates(
                &display_root,
                &roots,
                request.terms,
                config,
                &mut candidates,
            )?;
            candidates.sort_by_key(|candidate| {
                query_candidate_priority(&candidate.path, request.terms, &axis_terms)
            });
            let mut trace_fields = collection.provenance.trace_fields(candidates.len());
            if package_path_augmented_count > 0 {
                trace_fields.insert(
                    "packagePathAugmented".to_string(),
                    Value::from(package_path_augmented_count),
                );
            }
            return Ok(QueryWrapperCandidateCollection {
                candidates,
                trace_fields,
                source_index_trace: None,
                finder_skipped_after_source_index: false,
                candidate_sources: vec!["finder".to_string()],
            });
        }
    }

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for root in &roots {
        if candidates.len() >= crate::QUERY_WRAPPER_CANDIDATE_LIMIT {
            break;
        }
        crate::append_query_candidates(QueryCandidateAppend {
            surface: candidate_surface(request.surface),
            locator_root: &display_root,
            path: root,
            terms: request.terms,
            axis_terms: &axis_terms,
            config,
            accept_all_files,
            seen: &mut seen,
            candidates: &mut candidates,
        })?;
    }
    candidates.sort_by_key(|candidate| {
        query_candidate_priority(&candidate.path, request.terms, &axis_terms)
    });
    let mut candidates = cohesive_query_candidates(candidates, request.clauses);
    let package_path_augmented_count = augment_package_path_candidates(
        &display_root,
        &roots,
        request.terms,
        config,
        &mut candidates,
    )?;
    candidates.sort_by_key(|candidate| {
        query_candidate_priority(&candidate.path, request.terms, &axis_terms)
    });
    let mut trace_fields = BTreeMap::new();
    if package_path_augmented_count > 0 {
        trace_fields.insert(
            "packagePathAugmented".to_string(),
            Value::from(package_path_augmented_count),
        );
    }
    Ok(QueryWrapperCandidateCollection {
        candidates,
        trace_fields,
        source_index_trace: None,
        finder_skipped_after_source_index: false,
        candidate_sources: vec!["finder".to_string()],
    })
}

fn collect_source_index_query_candidates(
    surface: QueryWrapperSearchSurface,
    project_root: &Path,
    roots: &[PathBuf],
    terms: &[String],
    axis_terms: &[String],
    source_index_lookup: Option<&QueryWrapperSourceIndexLookup>,
) -> Result<Option<QueryWrapperCandidateCollection>, String> {
    let Some(source_index_lookup) = source_index_lookup else {
        return Ok(None);
    };
    let started_at = Instant::now();
    let Some(collection) =
        collect_query_wrapper_source_index_candidates(QueryWrapperSourceIndexRequest {
            surface: candidate_surface(surface),
            project_root,
            roots,
            terms,
            axis_terms,
            lookup: source_index_lookup,
        })?
    else {
        return Ok(None);
    };
    let candidate_count = collection.candidates.len();
    Ok(Some(QueryWrapperCandidateCollection {
        candidates: collection.candidates,
        trace_fields: BTreeMap::new(),
        source_index_trace: Some(QueryWrapperSearchSourceIndexTrace {
            lookup: source_index_lookup.clone(),
            candidate_count,
            elapsed: started_at.elapsed(),
        }),
        finder_skipped_after_source_index: true,
        candidate_sources: vec!["source-index".to_string()],
    }))
}

fn cohesive_query_candidates(
    candidates: Vec<QueryWrapperCandidate>,
    clauses: &[QueryWrapperSearchClause],
) -> Vec<QueryWrapperCandidate> {
    if candidates.is_empty() || clauses.len() <= 1 {
        return candidates;
    }
    let expected = clauses
        .iter()
        .map(|clause| clause.id)
        .collect::<BTreeSet<_>>();
    let mut package_coverage = BTreeMap::<String, BTreeSet<usize>>::new();
    let mut path_coverage = BTreeMap::<String, BTreeSet<usize>>::new();
    for candidate in &candidates {
        let clause_ids = candidate_clause_ids(candidate, clauses);
        if clause_ids.is_empty() {
            continue;
        }
        package_coverage
            .entry(package_key(&candidate.path))
            .or_default()
            .extend(clause_ids.iter().copied());
        path_coverage
            .entry(candidate.path.clone())
            .or_default()
            .extend(clause_ids);
    }
    let cohesive_packages = package_coverage
        .iter()
        .filter(|(_, coverage)| coverage == &&expected)
        .map(|(package, _)| package.clone())
        .collect::<BTreeSet<_>>();
    if !cohesive_packages.is_empty() {
        return candidates
            .into_iter()
            .filter(|candidate| cohesive_packages.contains(&package_key(&candidate.path)))
            .collect();
    }
    let cohesive_paths = path_coverage
        .iter()
        .filter(|(_, coverage)| coverage == &&expected)
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if !cohesive_paths.is_empty() {
        return candidates
            .into_iter()
            .filter(|candidate| cohesive_paths.contains(&candidate.path))
            .collect();
    }
    candidates
}

fn candidate_clause_ids(
    candidate: &QueryWrapperCandidate,
    clauses: &[QueryWrapperSearchClause],
) -> BTreeSet<usize> {
    clauses
        .iter()
        .filter(|clause| {
            clause
                .terms
                .iter()
                .any(|term| candidate_matches_term(candidate, term))
        })
        .map(|clause| clause.id)
        .collect()
}

fn candidate_matches_term(candidate: &QueryWrapperCandidate, term: &str) -> bool {
    let lower =
        format!("{} {} {}", candidate.path, candidate.symbol, candidate.text).to_ascii_lowercase();
    lower.contains(term)
}

fn query_wrapper_candidate_from_native(
    candidate: crate::NativeFinderCandidate,
) -> QueryWrapperCandidate {
    QueryWrapperCandidate {
        path: candidate.path,
        line: candidate.line,
        end_line: candidate.end_line,
        symbol: candidate.symbol,
        text: candidate.text,
        source: candidate.source,
        confidence: candidate.confidence,
    }
}

fn query_axis_terms(clauses: &[QueryWrapperSearchClause]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    clauses
        .iter()
        .flat_map(|clause| clause.axis_terms.iter().cloned())
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

fn resolved_scope_roots(locator_root: &Path, scopes: &[PathBuf]) -> Vec<PathBuf> {
    if scopes.is_empty() {
        vec![locator_root.to_path_buf()]
    } else {
        scopes
            .iter()
            .map(|scope| absolute_scope(locator_root, scope))
            .collect()
    }
}

fn display_root(locator_root: &Path, scopes: &[PathBuf], roots: &[PathBuf]) -> PathBuf {
    if scopes.len() == 1 {
        let root = roots
            .first()
            .cloned()
            .unwrap_or_else(|| locator_root.to_path_buf());
        if root.is_file() {
            locator_root.to_path_buf()
        } else {
            root
        }
    } else {
        locator_root.to_path_buf()
    }
}

fn native_surface(surface: QueryWrapperSearchSurface) -> NativeFinderSurface {
    match surface {
        QueryWrapperSearchSurface::Fd => NativeFinderSurface::Path,
        QueryWrapperSearchSurface::Rg => NativeFinderSurface::Content,
    }
}

fn candidate_surface(surface: QueryWrapperSearchSurface) -> QueryWrapperCandidateSurface {
    match surface {
        QueryWrapperSearchSurface::Fd => QueryWrapperCandidateSurface::Fd,
        QueryWrapperSearchSurface::Rg => QueryWrapperCandidateSurface::Rg,
    }
}

fn fd_query_prefers_internal_scan(
    surface: QueryWrapperSearchSurface,
    terms: &[String],
    native_args: &[String],
) -> bool {
    surface == QueryWrapperSearchSurface::Fd
        && native_args.is_empty()
        && terms
            .iter()
            .any(|term| term.contains('/') || term.contains('.') || term.contains('_'))
}

fn absolute_scope(root: &Path, scope: &Path) -> PathBuf {
    if scope.is_absolute() {
        scope.to_path_buf()
    } else {
        root.join(scope)
    }
}

fn package_key(path: &str) -> String {
    path.split('/')
        .take_while(|segment| *segment != "src" && *segment != "tests")
        .collect::<Vec<_>>()
        .join("/")
}
