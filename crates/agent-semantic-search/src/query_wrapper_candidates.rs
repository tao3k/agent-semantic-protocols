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
    pub raw: String,
    pub terms: Vec<String>,
    pub axis_terms: Vec<String>,
}

/// Search-owned candidate view used by query-wrapper quality analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperQualityCandidate {
    pub path: String,
    pub symbol: String,
    pub text: String,
}

/// Query-wrapper clause coverage result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperClauseCoverage {
    pub id: usize,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
}

/// Query-wrapper quality analysis result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryWrapperQuality {
    pub query_pack_quality: String,
    pub scope_quality: String,
    pub package_cohesion: String,
    pub packages: Vec<String>,
    pub risks: Vec<String>,
    pub noise: Vec<String>,
    pub allow_query_selector: bool,
    pub clause_coverages: Vec<QueryWrapperClauseCoverage>,
}

/// Split one query-wrapper raw query into stable lowercase terms.
#[must_use]
pub fn query_wrapper_terms(query: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    query
        .split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(str::to_ascii_lowercase)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

/// Expand one query-wrapper raw query into search axes, including identifier
/// components such as camelCase and PascalCase segments.
#[must_use]
pub fn query_wrapper_axis_terms(raw: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    raw.split(|character: char| character == '|' || character == ',' || character.is_whitespace())
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .flat_map(expanded_query_terms)
        .filter(|term| seen.insert(term.clone()))
        .collect()
}

/// Build query-wrapper clauses from raw query strings.
#[must_use]
pub fn query_wrapper_clauses(queries: &[String]) -> Vec<QueryWrapperSearchClause> {
    queries
        .iter()
        .enumerate()
        .filter_map(|(index, raw)| {
            let terms = query_wrapper_terms(raw);
            (!terms.is_empty()).then_some(QueryWrapperSearchClause {
                id: index + 1,
                raw: raw.clone(),
                terms,
                axis_terms: query_wrapper_axis_terms(raw),
            })
        })
        .collect()
}

/// Return deduplicated clause terms in first-seen order.
#[must_use]
pub fn query_wrapper_unique_clause_terms(clauses: &[QueryWrapperSearchClause]) -> Vec<String> {
    clauses
        .iter()
        .flat_map(|clause| clause.terms.iter())
        .fold(Vec::new(), |mut terms, term| {
            if !terms.iter().any(|seen| seen == term) {
                terms.push(term.clone());
            }
            terms
        })
}

fn expanded_query_terms(raw: &str) -> Vec<String> {
    let normalized = raw.to_ascii_lowercase();
    let normalized_filter = normalized.clone();
    std::iter::once(normalized)
        .chain(
            identifier_components(raw)
                .into_iter()
                .filter(move |component| component.len() >= 2 && component != &normalized_filter),
        )
        .collect()
}

fn identifier_components(raw: &str) -> Vec<String> {
    let chars = raw.chars().collect::<Vec<_>>();
    let mut components = Vec::new();
    let mut current = String::new();
    for (index, character) in chars.iter().enumerate() {
        if !character.is_alphanumeric() {
            push_component(&mut components, &mut current);
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|previous| chars.get(previous));
        let next = chars.get(index + 1);
        let uppercase_boundary = character.is_uppercase()
            && previous.is_some_and(|previous| {
                previous.is_lowercase()
                    || previous.is_ascii_digit()
                    || (previous.is_uppercase() && next.is_some_and(|next| next.is_lowercase()))
            });
        if uppercase_boundary {
            push_component(&mut components, &mut current);
        }
        current.push(character.to_ascii_lowercase());
    }
    push_component(&mut components, &mut current);
    components
}

fn push_component(components: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        components.push(std::mem::take(current));
    }
}

/// Analyze query-wrapper quality gates over search-owned candidate facts.
#[must_use]
pub fn analyze_query_wrapper_quality(
    scopes: &[PathBuf],
    clauses: &[QueryWrapperSearchClause],
    terms: &[String],
    candidates: &[QueryWrapperQualityCandidate],
) -> QueryWrapperQuality {
    let scope_quality = query_wrapper_scope_quality(scopes);
    let packages = query_wrapper_package_clusters(candidates);
    let package_count = candidates
        .iter()
        .map(|candidate| query_wrapper_package_key(&candidate.path))
        .filter(|package| !package.is_empty())
        .collect::<BTreeSet<_>>()
        .len();
    let package_cohesion = if candidates.is_empty() || package_count == 0 {
        "low"
    } else if package_count <= 1 {
        "high"
    } else if package_count <= 3 {
        "medium"
    } else {
        "low"
    }
    .to_string();
    let clause_coverages = query_wrapper_clause_coverages(clauses, candidates);
    let noise = query_wrapper_noise_paths(candidates);
    let single_or_clause = clauses.len() == 1 && terms.len() > 1;
    let generic_count = terms
        .iter()
        .filter(|term| is_query_wrapper_generic_term(term))
        .count();
    let generic_ratio_high = !terms.is_empty() && generic_count * 5 >= terms.len() * 2;
    let all_clauses_covered = clause_coverages
        .iter()
        .all(|coverage| !coverage.matched.is_empty());
    let flat_recall_risk = single_or_clause
        && (scope_quality == "low"
            || terms.len() >= 4
            || package_cohesion == "low"
            || generic_ratio_high
            || !noise.is_empty());
    let mut risks = Vec::new();
    if candidates.is_empty() {
        risks.push("no-candidates".to_string());
    }
    if single_or_clause {
        risks.push("single-flat-or-recall".to_string());
    }
    if scope_quality == "low" {
        risks.push("broad-scope".to_string());
    }
    if package_cohesion == "low" {
        risks.push("low-package-cohesion".to_string());
    }
    if !all_clauses_covered {
        risks.push("clause-missing".to_string());
    }
    if generic_ratio_high {
        risks.push("generic-terms".to_string());
    }
    if !noise.is_empty() {
        risks.push("noisy-candidates".to_string());
    }
    let query_pack_quality = if candidates.is_empty() || !all_clauses_covered || flat_recall_risk {
        "low"
    } else if clauses.len() >= 2 && scope_quality == "high" && package_cohesion == "high" {
        "high"
    } else {
        "medium"
    }
    .to_string();
    let allow_query_selector =
        clauses.len() >= 2 && query_pack_quality != "low" && package_cohesion != "low";
    QueryWrapperQuality {
        query_pack_quality,
        scope_quality,
        package_cohesion,
        packages,
        risks,
        noise,
        allow_query_selector,
        clause_coverages,
    }
}

/// Return the package/area key used by query-wrapper quality and next-action
/// projection.
#[must_use]
pub fn query_wrapper_package_key(path: &str) -> String {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "packages") {
        let end = (index + 3).min(parts.len());
        return parts[index..end].join("/");
    }
    parts
        .into_iter()
        .filter(|part| !part.is_empty() && *part != ".")
        .take(2)
        .collect::<Vec<_>>()
        .join("/")
}

/// Return stable package clusters for query-wrapper candidates.
#[must_use]
pub fn query_wrapper_package_clusters(candidates: &[QueryWrapperQualityCandidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .map(|candidate| query_wrapper_package_key(&candidate.path)),
        6,
    )
}

/// Return compact owner path candidates for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_owner_candidates(paths: impl Iterator<Item = String>) -> Vec<String> {
    unique_take(paths, 8)
}

/// Return compact package clusters for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_package_clusters_from_paths(
    paths: impl Iterator<Item = String>,
) -> Vec<String> {
    unique_take(paths.map(|path| query_wrapper_package_key(&path)), 6)
}

/// Return the next rg scope candidates for query-wrapper render hints.
#[must_use]
pub fn query_wrapper_rg_scope_next(paths: impl Iterator<Item = String>) -> Vec<String> {
    unique_take(
        paths
            .map(|path| query_wrapper_package_key(&path))
            .filter(|package| !package.is_empty()),
        3,
    )
}

#[must_use]
pub fn query_wrapper_candidate_matches_term(
    candidate: &QueryWrapperQualityCandidate,
    term: &str,
) -> bool {
    format!("{} {} {}", candidate.path, candidate.symbol, candidate.text)
        .to_ascii_lowercase()
        .contains(term)
}

fn query_wrapper_scope_quality(scopes: &[PathBuf]) -> String {
    if scopes.is_empty()
        || scopes
            .iter()
            .any(|scope| scope.as_os_str().is_empty() || scope.as_path() == Path::new("."))
    {
        "low"
    } else if scopes.len() == 1 {
        "high"
    } else {
        "medium"
    }
    .to_string()
}

fn query_wrapper_clause_coverages(
    clauses: &[QueryWrapperSearchClause],
    candidates: &[QueryWrapperQualityCandidate],
) -> Vec<QueryWrapperClauseCoverage> {
    clauses
        .iter()
        .map(|clause| {
            let matched = clause
                .terms
                .iter()
                .filter(|term| {
                    candidates
                        .iter()
                        .any(|candidate| query_wrapper_candidate_matches_term(candidate, term))
                })
                .cloned()
                .collect::<Vec<_>>();
            let missing = clause
                .terms
                .iter()
                .filter(|term| !matched.iter().any(|matched| matched == *term))
                .cloned()
                .collect::<Vec<_>>();
            QueryWrapperClauseCoverage {
                id: clause.id,
                matched,
                missing,
            }
        })
        .collect()
}

fn query_wrapper_noise_paths(candidates: &[QueryWrapperQualityCandidate]) -> Vec<String> {
    unique_take(
        candidates
            .iter()
            .filter(|candidate| is_query_wrapper_noise_path(&candidate.path))
            .map(|candidate| {
                let package = query_wrapper_package_key(&candidate.path);
                if package.is_empty() {
                    candidate.path.clone()
                } else {
                    package
                }
            }),
        6,
    )
}

fn unique_take(values: impl Iterator<Item = String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .take(limit)
        .collect()
}

fn is_query_wrapper_noise_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("/analyzers/")
        || lower.starts_with("analyzers/")
        || lower.contains("/notebooks/")
        || lower.starts_with("notebooks/")
        || lower.contains("/experiments/")
        || lower.starts_with("experiments/")
}

fn is_query_wrapper_generic_term(term: &str) -> bool {
    matches!(
        term,
        "asp"
            | "rg"
            | "fd"
            | "query"
            | "search"
            | "command"
            | "config"
            | "cache"
            | "provider"
            | "prefix"
            | "scope"
            | "owner"
            | "package"
            | "frontier"
            | "noise"
            | "policy"
            | "stage"
            | "activation"
    )
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

/// Render-neutral source-index trace projection for query-wrapper callers.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryWrapperSourceIndexTraceProjection {
    pub source: String,
    pub status: String,
    pub candidate_count: usize,
    pub skipped_count: usize,
    pub input_count: usize,
    pub fields: BTreeMap<String, Value>,
}

/// Project a query-wrapper source-index trace into render-neutral fields.
#[must_use]
pub fn query_wrapper_source_index_trace_projection(
    trace: &QueryWrapperSearchSourceIndexTrace,
) -> QueryWrapperSourceIndexTraceProjection {
    let status = query_wrapper_source_index_status(trace.lookup.state.as_str());
    let mut fields = BTreeMap::new();
    fields.insert(
        "collectMs".to_string(),
        Value::from(trace.elapsed.as_millis().min(u128::from(u64::MAX)) as u64),
    );
    fields.insert("state".to_string(), Value::from(trace.lookup.state.clone()));
    fields.insert(
        "dbPath".to_string(),
        Value::from(trace.lookup.db_path.display().to_string()),
    );
    if status != "used" {
        fields.insert(
            "nextCommand".to_string(),
            Value::from("asp cache source-index refresh"),
        );
    }
    QueryWrapperSourceIndexTraceProjection {
        source: "sourceIndex".to_string(),
        status: status.to_string(),
        candidate_count: trace.candidate_count,
        skipped_count: usize::from(trace.candidate_count == 0),
        input_count: trace.candidate_count,
        fields,
    }
}

fn query_wrapper_source_index_status(state: &str) -> &'static str {
    match state {
        "hit" => "used",
        "missing-db" => "missing-db",
        "empty-index" => "empty-index",
        "miss" => "miss",
        _ => "unknown",
    }
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
        selector: None,
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
