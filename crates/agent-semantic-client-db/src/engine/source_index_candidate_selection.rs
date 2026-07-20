use std::collections::BTreeSet;

use agent_semantic_client_core::{LanguageId, ProviderId};

use crate::{
    ClientDbSourceIndexCandidate, ClientDbSourceIndexSelectorPayloadProof,
    ClientDbSourceIndexSourceKind,
};

use super::source_index_candidate_types::{
    TursoSourceIndexCandidateScope, TursoSourceIndexCanonicalSelectorFact,
    TursoSourceIndexLookupRequestScope, TursoSourceIndexLookupScope,
};
use super::source_index_query_scoring::{
    SOURCE_INDEX_READ_MODEL_MAX_CANDIDATES, SOURCE_INDEX_READ_MODEL_MAX_TERMS,
    source_index_structured_candidate_score,
};
use super::turso_statement::run_turso_operation_with_lock_retry;

pub(super) fn decode_turso_source_index_canonical_selectors(
    selector_facts_json: &str,
) -> Result<
    (
        String,
        Option<String>,
        Option<String>,
        Option<ClientDbSourceIndexSelectorPayloadProof>,
    ),
    String,
> {
    let selector_facts =
        serde_json::from_str::<Vec<TursoSourceIndexCanonicalSelectorFact>>(selector_facts_json)
            .map_err(|error| {
                format!("failed to decode Turso source-index canonical selectors: {error}")
            })?;
    let mut haystack = String::new();
    let mut selector_symbol = None;
    let mut selector_kind = None;
    let mut selector_proof = None;
    for selector in selector_facts {
        if selector_proof.is_none()
            && let Some(payload_kind) = selector
                .payload_kind
                .filter(|value| !value.trim().is_empty())
        {
            selector_symbol = selector.symbol.clone();
            selector_kind = selector.kind.clone();
            selector_proof = Some(ClientDbSourceIndexSelectorPayloadProof {
                structural_selector: selector.selector_id.clone(),
                payload_kind,
                bounded: selector.payload_bounded,
            });
        }
        haystack.push(' ');
        haystack.push_str(&selector.selector_id);
        haystack.push(' ');
        haystack.push_str(selector.symbol.as_deref().unwrap_or_default());
        haystack.push(' ');
        haystack.push_str(selector.kind.as_deref().unwrap_or_default());
        haystack.push(' ');
        haystack.push_str(&selector.source);
        haystack.push(' ');
        haystack.push_str(
            &serde_json::to_string(&selector.query_keys).map_err(|error| {
                format!("failed to encode Turso source-index canonical selector keys: {error}")
            })?,
        );
    }
    Ok((haystack, selector_symbol, selector_kind, selector_proof))
}

pub(super) async fn resolve_turso_source_index_lookup_scope(
    connection: &turso::Connection,
    requested_scope: Option<TursoSourceIndexLookupRequestScope>,
) -> Result<Option<TursoSourceIndexLookupScope>, String> {
    let mut rows = match requested_scope {
        Some(scope) => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(
                            "SELECT scope.project_root, scope.schema_id, scope.schema_version, scope.generation_id, scope.source_snapshot_json
                         FROM asp_source_index_scope_v1 AS scope
                         JOIN asp_source_index_layout_v1 AS layout
                           ON layout.project_root = scope.project_root
                          AND layout.schema_id = scope.schema_id
                          AND layout.schema_version = scope.schema_version
                          AND layout.term_projection_version = ?4
                          AND layout.token_projection_generation_id = scope.generation_id
                         WHERE scope.project_root = ?1
                           AND scope.schema_id = ?2
                           AND scope.schema_version = ?3
                         LIMIT 1",
                            (
                                scope.project_root.as_str(),
                                scope.schema_id.as_str(),
                                scope.schema_version.as_str(),
                                super::turso_source_index::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                            ),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to resolve Turso source-index scope",
            )
            .await?
        }
        None => {
            run_turso_operation_with_lock_retry(
                || async {
                    connection
                        .query(
                            "SELECT scope.project_root, scope.schema_id, scope.schema_version, scope.generation_id, scope.source_snapshot_json
                         FROM asp_source_index_scope_v1 AS scope
                         JOIN asp_source_index_layout_v1 AS layout
                           ON layout.project_root = scope.project_root
                          AND layout.schema_id = scope.schema_id
                          AND layout.schema_version = scope.schema_version
                          AND layout.term_projection_version = ?1
                          AND layout.token_projection_generation_id = scope.generation_id
                         ORDER BY scope.updated_at_ms DESC
                         LIMIT 2",
                            (super::turso_source_index::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,),
                        )
                        .await
                        .map_err(|error| error.to_string())
                },
                "failed to resolve unscoped Turso source-index scope",
            )
            .await?
        }
    };
    let Some(row) = rows
        .next()
        .await
        .map_err(|error| format!("failed to read Turso source-index scope: {error}"))?
    else {
        return Ok(None);
    };
    let scope = TursoSourceIndexLookupScope {
        project_root: row
            .get::<String>(0)
            .map_err(|error| format!("failed to read Turso source-index project root: {error}"))?,
        schema_id: row
            .get::<String>(1)
            .map_err(|error| format!("failed to read Turso source-index schema id: {error}"))?,
        schema_version: row.get::<String>(2).map_err(|error| {
            format!("failed to read Turso source-index schema version: {error}")
        })?,
        generation_id: row
            .get::<String>(3)
            .map_err(|error| format!("failed to read Turso source-index generation id: {error}"))?,
        source_snapshot_json: row.get::<String>(4).map_err(|error| {
            format!("failed to read Turso source-index source snapshot evidence: {error}")
        })?,
    };
    if rows
        .next()
        .await
        .map_err(|error| format!("failed to verify Turso source-index scope: {error}"))?
        .is_some()
    {
        return Err(
            "unscoped Turso source-index lookup is ambiguous; provide the indexed project root"
                .to_string(),
        );
    }
    Ok(Some(scope))
}

pub(super) async fn query_turso_source_index_snapshot_candidates_with_connection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    query_turso_source_index_snapshot_candidates_for_scope_with_connection(
        connection,
        TursoSourceIndexCandidateScope::Resolved(scope),
        query,
        language_id,
        limit,
        terms,
    )
    .await
}

pub(super) async fn query_turso_source_index_snapshot_candidates_for_scope_with_connection(
    connection: &turso::Connection,
    scope: TursoSourceIndexCandidateScope<'_>,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    if limit == 0 || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.min(SOURCE_INDEX_READ_MODEL_MAX_CANDIDATES);
    let terms = &terms[..terms.len().min(SOURCE_INDEX_READ_MODEL_MAX_TERMS)];
    let term_tokens_json = serde_json::to_string(terms)
        .map_err(|error| format!("failed to encode Turso source-index query terms: {error}"))?;
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    // Each requested token probes the `(scope, token, owner_path)` primary key
    // directly. Avoid a Turso group/order aggregate over high-fanout postings;
    // structured scoring below fuses the bounded per-token owner windows.
    let candidate_limit = i64::from(limit);
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some()
        && let TursoSourceIndexCandidateScope::Resolved(scope) = scope
    {
        trace_turso_source_index_posting_projection(
            connection,
            scope,
            term_tokens_json.as_str(),
            terms.len(),
        )
        .await?;
    }
    let posting_lookup_started_at = std::time::Instant::now();
    let mut fetched_owner_count = 0;
    let mut seen_owner_paths = BTreeSet::new();
    let mut candidates = Vec::<(usize, ClientDbSourceIndexCandidate)>::new();
    for term in terms {
        let mut rows = run_turso_operation_with_lock_retry(
            || async {
                match scope {
                    TursoSourceIndexCandidateScope::Resolved(scope) => connection
                        .query(
                        "SELECT owner.owner_path,
                                owner.language_id,
                                owner.provider_id,
                                owner.source_kind,
                                owner.line_count,
                                owner.query_keys_json,
                                owner.selector_facts_json
                         FROM asp_source_index_token_owner_v1 AS indexed
JOIN asp_source_index_owner_v1 AS owner
                           ON owner.project_root = indexed.project_root
                          AND owner.schema_id = indexed.schema_id
                          AND owner.schema_version = indexed.schema_version
                          AND owner.generation_id = indexed.generation_id
                          AND owner.owner_path = indexed.owner_path
                         WHERE indexed.project_root = ?1
                           AND indexed.schema_id = ?2
                           AND indexed.schema_version = ?3
                       AND indexed.generation_id = ?4
                       AND indexed.token = ?5
                       AND (?6 IS NULL OR owner.language_id = ?6)
                         ORDER BY indexed.owner_path
                    LIMIT ?7",
                        (
                            scope.project_root.as_str(),
                            scope.schema_id.as_str(),
                            scope.schema_version.as_str(),
                            scope.generation_id.as_str(),
                            term.as_str(),
                            language_id.map(|value| value.as_str()),
                            candidate_limit,
                        ),
                    )
                    .await
                    .map_err(|error| error.to_string()),
                    TursoSourceIndexCandidateScope::Requested(scope) => connection
                        .query(
                            "SELECT owner.owner_path,
                                    owner.language_id,
                                    owner.provider_id,
                                    owner.source_kind,
                                    owner.line_count,
                                    owner.query_keys_json,
                                    owner.selector_facts_json
                             FROM asp_source_index_token_owner_v1 AS indexed
                             JOIN asp_source_index_owner_v1 AS owner
                               ON owner.project_root = indexed.project_root
                              AND owner.schema_id = indexed.schema_id
                              AND owner.schema_version = indexed.schema_version
                              AND owner.generation_id = indexed.generation_id
                              AND owner.owner_path = indexed.owner_path
                             WHERE indexed.project_root = ?1
                               AND indexed.schema_id = ?2
                               AND indexed.schema_version = ?3
                               AND indexed.generation_id = (
                                   SELECT published.generation_id
                                   FROM asp_source_index_scope_v1 AS published
                                   JOIN asp_source_index_layout_v1 AS layout
                                     ON layout.project_root = published.project_root
                                    AND layout.schema_id = published.schema_id
                                    AND layout.schema_version = published.schema_version
                                    AND layout.term_projection_version = ?7
                                    AND layout.token_projection_generation_id = published.generation_id
                                   WHERE published.project_root = ?1
                                     AND published.schema_id = ?2
                                     AND published.schema_version = ?3
                                   LIMIT 1
                               )
                               AND indexed.token = ?4
                               AND (?5 IS NULL OR owner.language_id = ?5)
                             ORDER BY indexed.owner_path
                             LIMIT ?6",
                            (
                                scope.project_root.as_str(),
                                scope.schema_id.as_str(),
                                scope.schema_version.as_str(),
                                term.as_str(),
                                language_id.map(|value| value.as_str()),
                                candidate_limit,
                                super::turso_source_index::core::TURSO_SOURCE_INDEX_TERM_PROJECTION_VERSION,
                            ),
                        )
                        .await
                        .map_err(|error| error.to_string()),
                }
            },
            "failed to query Turso source-index token postings",
        )
        .await?;
        while let Some(row) = rows
            .next()
            .await
            .map_err(|error| format!("failed to read Turso source-index snapshot owner: {error}"))?
        {
            fetched_owner_count += 1;
            let path = row.get::<String>(0).map_err(|error| {
                format!("failed to read Turso source-index owner path: {error}")
            })?;
            if !seen_owner_paths.insert(path.clone()) {
                continue;
            }
            let row_language_id = row.get::<Option<String>>(1).map_err(|error| {
                format!("failed to read Turso source-index owner language id: {error}")
            })?;
            let provider_id = row.get::<Option<String>>(2).map_err(|error| {
                format!("failed to read Turso source-index owner provider id: {error}")
            })?;
            let source_kind = row.get::<String>(3).map_err(|error| {
                format!("failed to read Turso source-index owner source kind: {error}")
            })?;
            let line_count = row
                .get::<Option<i64>>(4)
                .map_err(|error| format!("failed to read Turso source-index line count: {error}"))?
                .and_then(|value| u32::try_from(value).ok());
            let query_keys_json = row.get::<String>(5).map_err(|error| {
                format!("failed to read Turso source-index query keys: {error}")
            })?;
            let selector_facts_json = row.get::<String>(6).map_err(|error| {
                format!("failed to read Turso source-index canonical selectors: {error}")
            })?;
            let query_keys =
                serde_json::from_str::<Vec<String>>(&query_keys_json).map_err(|error| {
                    format!("failed to decode Turso source-index query keys: {error}")
                })?;
            let (selector_haystack, selector_symbol, selector_kind, selector_proof) =
                decode_turso_source_index_canonical_selectors(&selector_facts_json)?;
            let match_score = source_index_structured_candidate_score(
                &path,
                row_language_id.as_deref(),
                provider_id.as_deref(),
                &source_kind,
                &query_keys,
                &selector_haystack,
                terms,
            );
            if match_score == 0 {
                continue;
            }
            candidates.push((
                match_score,
                ClientDbSourceIndexCandidate {
                    path,
                    language_id: row_language_id.map(LanguageId::from),
                    provider_id: provider_id.map(ProviderId::from),
                    source_kind: ClientDbSourceIndexSourceKind::Other(
                        "turso-source-index".to_string(),
                    ),
                    line_count,
                    query_keys,
                    selector_symbol,
                    selector_kind,
                    selector_proof,
                },
            ));
        }
    }
    candidates.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.path.cmp(&right.path))
    });
    if std::env::var_os("ASP_SOURCE_INDEX_TRACE").is_some() {
        eprintln!(
            "[source-index-read-trace] stage=token-candidates fetchedOwners={fetched_owner_count} rankedOwners={} lookupMs={}",
            candidates.len(),
            posting_lookup_started_at.elapsed().as_millis(),
        );
    }
    Ok(candidates
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(limit as usize)
        .collect())
}

async fn trace_turso_source_index_posting_projection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    term_tokens_json: &str,
    requested_term_count: usize,
) -> Result<(), String> {
    let mut rows = connection
        .query(
            "WITH requested_terms AS (
                SELECT DISTINCT lower(value) AS token
                FROM json_each(?4)
             )
             SELECT COUNT(*),
                    COUNT(*)
             FROM asp_source_index_token_owner_v1 AS indexed
             JOIN asp_source_index_scope_v1 AS active
               ON active.project_root = indexed.project_root
              AND active.schema_id = indexed.schema_id
              AND active.schema_version = indexed.schema_version
              AND active.generation_id = indexed.generation_id
             JOIN requested_terms
               ON requested_terms.token = indexed.token
             WHERE indexed.project_root = ?1
               AND indexed.schema_id = ?2
               AND indexed.schema_version = ?3",
            (
                scope.project_root.as_str(),
                scope.schema_id.as_str(),
                scope.schema_version.as_str(),
                term_tokens_json,
            ),
        )
        .await
        .map_err(|error| format!("failed to trace Turso source-index token projection: {error}"))?;
    let Some(row) = rows.next().await.map_err(|error| {
        format!("failed to read Turso source-index token projection trace: {error}")
    })?
    else {
        return Ok(());
    };
    let token_count = row
        .get::<i64>(0)
        .map_err(|error| format!("failed to decode Turso source-index token trace: {error}"))?;
    let owner_count = row
        .get::<i64>(1)
        .map_err(|error| format!("failed to decode Turso source-index owner trace: {error}"))?;
    eprintln!(
        "[source-index-read-trace] stage=posting-lookup requestedTerms={requested_term_count} matchedTokens={token_count} matchedPostings={owner_count}"
    );
    Ok(())
}

pub(super) async fn query_turso_source_index_candidates_with_connection(
    connection: &turso::Connection,
    scope: &TursoSourceIndexLookupScope,
    query: &str,
    language_id: Option<&LanguageId>,
    limit: u32,
    terms: &[String],
) -> Result<Vec<ClientDbSourceIndexCandidate>, String> {
    query_turso_source_index_snapshot_candidates_with_connection(
        connection,
        scope,
        query,
        language_id,
        limit,
        terms,
    )
    .await
}
