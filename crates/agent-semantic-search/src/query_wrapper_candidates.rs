//! Query-wrapper candidate orchestration.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::dynamic_overlay::DynamicOverlayLane;
use crate::query_wrapper_quality::{QueryWrapperSearchClause, query_wrapper_unique_take};
use crate::query_wrapper_scan::query_candidate_priority;
use crate::{
    QueryCandidateAppend, QueryWrapperCandidate, QueryWrapperCandidateSurface,
    QueryWrapperScanConfig, QueryWrapperSearchCandidateRequest, QueryWrapperSourceIndexLookup,
    QueryWrapperSourceIndexRequest, RankedSearchCandidate, SearchOverlayCollectionRequest,
    SearchOverlayConfig, SearchOverlaySurface, SearchStageReceipt, augment_package_path_candidates,
    collect_query_wrapper_search_candidates, collect_query_wrapper_source_index_candidates,
    collect_search_overlay_candidates, language_neutral_search_file_spec,
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

/// Collect ranked query-wrapper candidates from the configured search adapter.
pub fn query_wrapper_ranked_search_candidates(
    surface: QueryWrapperSearchSurface,
    project_root: &Path,
    terms: &[String],
) -> Result<Vec<RankedSearchCandidate>, String> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let query = terms.join(" ");
    let limit = query_wrapper_ranked_search_limit(surface);
    crate::collect_turso_structural_index_ranked_candidates(
        crate::TursoStructuralIndexCandidateRequest {
            project_root,
            query: query.as_str(),
            limit,
        },
    )
}

fn query_wrapper_ranked_search_limit(surface: QueryWrapperSearchSurface) -> u32 {
    match surface {
        QueryWrapperSearchSurface::Fd => 16,
        QueryWrapperSearchSurface::Rg => crate::QUERY_WRAPPER_CANDIDATE_LIMIT as u32,
    }
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
    pub ranked_search_candidates: &'a [RankedSearchCandidate],
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
    match status {
        "used" => {}
        "busy" => {
            fields.insert(
                "nextCommand".to_string(),
                Value::from("retry source-index lookup or continue query-overlay"),
            );
        }
        _ => {
            fields.insert(
                "nextCommand".to_string(),
                Value::from("asp cache source-index refresh"),
            );
        }
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
        "busy" => "busy",
        "miss" => "miss",
        _ => "unknown",
    }
}

/// Render-neutral search-stage trace projection for query-wrapper callers.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryWrapperSearchStageTraceProjection {
    pub source: String,
    pub status: String,
    pub candidate_count: usize,
    pub skipped_count: usize,
    pub input_count: usize,
    pub fields: BTreeMap<String, Value>,
}

/// Project a search-stage receipt into render-neutral fields.
#[must_use]
pub fn query_wrapper_search_stage_trace_projection(
    receipt: &SearchStageReceipt,
) -> QueryWrapperSearchStageTraceProjection {
    let source = receipt
        .route_sources
        .first()
        .cloned()
        .unwrap_or_else(|| "searchStage".to_string());
    let status = query_wrapper_search_stage_status(receipt);
    let mut fields = BTreeMap::new();
    fields.insert(
        "schemaId".to_string(),
        Value::from("agent.semantic-protocols.semantic-search-stage-receipt"),
    );
    fields.insert("schemaVersion".to_string(), Value::from(1));
    fields.insert("stage".to_string(), Value::from(receipt.stage.clone()));
    fields.insert(
        "routeSources".to_string(),
        Value::Array(
            receipt
                .route_sources
                .iter()
                .cloned()
                .map(Value::from)
                .collect(),
        ),
    );
    fields.insert(
        "candidateCount".to_string(),
        Value::from(receipt.candidate_count),
    );
    fields.insert(
        "returnedCount".to_string(),
        Value::from(receipt.returned_count),
    );
    fields.insert(
        "filteredLineIdentityCount".to_string(),
        Value::from(receipt.filtered_line_identity_count),
    );
    fields.insert(
        "fallbackReason".to_string(),
        Value::from(receipt.fallback_reason.clone()),
    );
    QueryWrapperSearchStageTraceProjection {
        source,
        status,
        candidate_count: receipt.returned_count,
        skipped_count: receipt
            .candidate_count
            .saturating_sub(receipt.returned_count),
        input_count: receipt.candidate_count,
        fields,
    }
}

fn query_wrapper_search_stage_status(receipt: &SearchStageReceipt) -> String {
    if receipt.fallback_reason != "none" {
        receipt.fallback_reason.clone()
    } else if receipt.returned_count == 0 {
        "empty".to_string()
    } else {
        "used".to_string()
    }
}

/// Query-wrapper candidates plus route receipt data.
pub struct QueryWrapperCandidateCollection {
    pub candidates: Vec<QueryWrapperCandidate>,
    pub trace_fields: BTreeMap<String, Value>,
    pub source_index_trace: Option<QueryWrapperSearchSourceIndexTrace>,
    pub search_stage_receipts: Vec<SearchStageReceipt>,
    pub search_stage_trace_projections: Vec<QueryWrapperSearchStageTraceProjection>,
    pub query_overlay_skipped_after_source_index: bool,
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

    if request.native_args.is_empty()
        && let Some(collection) = collect_ranked_search_query_candidates(
            request.project_root,
            &roots,
            request.terms,
            &axis_terms,
            request.ranked_search_candidates,
        )
    {
        return Ok(collection);
    }

    if !fd_query_prefers_internal_scan(request.surface, request.terms, request.native_args)
        && let Some(mut collection) =
            collect_search_overlay_candidates(SearchOverlayCollectionRequest {
                lane: DynamicOverlayLane::Query,
                surface: native_surface,
                language_id: "query-wrapper",
                file_spec_override: Some(language_neutral_search_file_spec()),
                accept_all_files,
                project_root: request.project_root,
                locator_root: &display_root,
                roots: &roots,
                terms: request.terms,
                config: SearchOverlayConfig {
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
                source_index_trace: busy_source_index_trace(request.source_index_lookup.as_ref()),
                search_stage_receipts: Vec::new(),
                search_stage_trace_projections: Vec::new(),
                query_overlay_skipped_after_source_index: false,
                candidate_sources: vec![query_overlay_route_source().to_string()],
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
                source_index_trace: busy_source_index_trace(request.source_index_lookup.as_ref()),
                search_stage_receipts: Vec::new(),
                search_stage_trace_projections: Vec::new(),
                query_overlay_skipped_after_source_index: false,
                candidate_sources: vec![query_overlay_route_source().to_string()],
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
        source_index_trace: busy_source_index_trace(request.source_index_lookup.as_ref()),
        search_stage_receipts: Vec::new(),
        search_stage_trace_projections: Vec::new(),
        query_overlay_skipped_after_source_index: false,
        candidate_sources: vec![query_overlay_route_source().to_string()],
    })
}

fn collect_ranked_search_query_candidates(
    project_root: &Path,
    roots: &[PathBuf],
    terms: &[String],
    axis_terms: &[String],
    ranked_search_candidates: &[RankedSearchCandidate],
) -> Option<QueryWrapperCandidateCollection> {
    let started_at = Instant::now();
    let collection = collect_query_wrapper_search_candidates(QueryWrapperSearchCandidateRequest {
        project_root,
        roots,
        terms,
        axis_terms,
        ranked: ranked_search_candidates,
    })?;
    let mut trace_fields = BTreeMap::new();
    trace_fields.insert(
        "collectMs".to_string(),
        Value::from(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
    );
    trace_fields.insert(
        "candidateCount".to_string(),
        Value::from(collection.candidates.len()),
    );
    let candidate_sources = query_wrapper_unique_take(
        collection
            .candidates
            .iter()
            .map(|candidate| candidate.source.clone()),
        6,
    );
    let search_stage_trace_projections = vec![query_wrapper_search_stage_trace_projection(
        &collection.stage_receipt,
    )];
    Some(QueryWrapperCandidateCollection {
        candidates: collection.candidates,
        trace_fields,
        source_index_trace: None,
        search_stage_receipts: vec![collection.stage_receipt],
        search_stage_trace_projections,
        query_overlay_skipped_after_source_index: false,
        candidate_sources,
    })
}

fn query_overlay_route_source() -> &'static str {
    DynamicOverlayLane::Query.route_source()
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
        search_stage_receipts: Vec::new(),
        search_stage_trace_projections: Vec::new(),
        query_overlay_skipped_after_source_index: true,
        candidate_sources: vec!["source-index".to_string()],
    }))
}

fn busy_source_index_trace(
    source_index_lookup: Option<&QueryWrapperSourceIndexLookup>,
) -> Option<QueryWrapperSearchSourceIndexTrace> {
    let lookup = source_index_lookup?;
    (lookup.state == "busy").then(|| QueryWrapperSearchSourceIndexTrace {
        lookup: lookup.clone(),
        candidate_count: 0,
        elapsed: Duration::ZERO,
    })
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
    candidate: crate::SearchOverlayCandidate,
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

fn native_surface(surface: QueryWrapperSearchSurface) -> SearchOverlaySurface {
    match surface {
        QueryWrapperSearchSurface::Fd => SearchOverlaySurface::Path,
        QueryWrapperSearchSurface::Rg => SearchOverlaySurface::Content,
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
