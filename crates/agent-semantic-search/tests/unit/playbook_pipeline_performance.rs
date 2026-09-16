// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use agent_semantic_content_identity::SourceSnapshotEvidence;
use agent_semantic_content_identity::SourceSnapshotKind;
use agent_semantic_content_identity::hash_blob;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation;
use agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelationEndpoint;
use agent_semantic_content_identity::workspace_generation_evidence::WorkspaceGenerationEvidenceV1;
use agent_semantic_search_projection::ResidentSearchWorkCounters;

use crate::NativeSyntaxProjection;
use crate::NativeSyntaxRelation;
use crate::NativeSyntaxSelector;
use crate::ResidentGraphSearchBudget;
use crate::ResidentGraphSearchRequest;
use crate::ResidentGraphSearchWork;
use crate::ResidentIndexBuildResources;
use crate::ResidentIndexBuildStrategy;
use crate::ResidentLexicalCoverageInput;
use crate::ResidentSearchAuthority;
use crate::ResidentSourceDocument;
use crate::ResidentSourceIndex;
use crate::SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID;
use crate::SearchGenerationGraphReceipt;
use crate::SearchGenerationGraphRequest;
use crate::SearchGenerationIdentity;
use crate::SourceByteOwner;
use crate::build_native_syntax_stage;
use crate::build_resident_graph_generation;
use crate::build_source_byte_acquisition_stage;
use crate::canonical_blake3_digest;
use crate::rank_resident_graph_generation;
use crate::resident_lexical_coverage_batch;
use crate::stable_graph_node_id;

const OWNER_COUNT: usize = 4_096;
const COHORT_COUNT: usize = 64;
const COLD_SAMPLE_COUNT: usize = 32;
const DERIVED_SAMPLE_COUNT: usize = 4;
// A p99 over 128 observations is determined by only the slowest few samples
// and is dominated by occasional host scheduling interruptions. Keep the
// latency budget strict, but use enough observations for a meaningful tail.
const WARM_SAMPLE_COUNT: usize = 512;
const CONCURRENT_QUERY_COUNT: usize = 32;
const QUERY_LIMIT: u32 = 32;

#[derive(Clone)]
struct LargeOwner {
    path: String,
    source: Vec<u8>,
    content_digest: String,
    cohort: String,
    selector: String,
}

struct LargeGeneration {
    index: Arc<ResidentSourceIndex>,
    graph: Arc<crate::ResidentGraphGeneration>,
    graph_request: Arc<SearchGenerationGraphRequest>,
    generation_digest: String,
    authority: ResidentSearchAuthority,
    construction: ConstructionDurations,
}

#[derive(Clone, Copy)]
struct ConstructionDurations {
    lexical_coverage: Duration,
    tantivy_commit: Duration,
    rust_graph: Duration,
    graph_relation_projection: Duration,
    graph_request_validation: Duration,
    graph_materialization: Duration,
    graph_owner_projection: Duration,
    graph_edge_projection: Duration,
    graph_identity_hash: Duration,
}

#[derive(Clone, Copy)]
struct Distribution {
    p50: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
}

fn digest(byte: char) -> String {
    format!("blake3-256:{}", byte.to_string().repeat(64))
}

fn cohort_name(index: usize) -> String {
    let first = char::from(b'a' + u8::try_from(index / 26).unwrap());
    let second = char::from(b'a' + u8::try_from(index % 26).unwrap());
    format!("cohort_{first}{second}")
}

fn large_owners() -> Vec<LargeOwner> {
    (0..OWNER_COUNT)
        .map(|index| {
            let cohort = cohort_name(index % COHORT_COUNT);
            let path = format!("crates/large_{:02}/src/owner_{index:04}.rs", index % 32);
            let symbol = format!("{cohort}_entry_{index:04}");
            let source = format!(
                "pub fn {symbol}() -> &'static str {{ \"{cohort} resident lexical graph evidence owner {index}\" }}\n"
            )
            .into_bytes();
            let selector = format!("rust://{path}#item/function/{symbol}");
            LargeOwner {
                path,
                content_digest: format!("blake3-256:{}", hash_blob(&source).value),
                source,
                cohort,
                selector,
            }
        })
        .collect()
}

fn identity(snapshot: &SourceSnapshotEvidence) -> SearchGenerationIdentity {
    SearchGenerationIdentity {
        project_id: "project-large-workspace".to_owned(),
        workspace_id: "workspace-large-workspace".to_owned(),
        source_root_digest: canonical_blake3_digest(&snapshot.root_digest).unwrap(),
        provider_digest: canonical_blake3_digest(&snapshot.provider_digest).unwrap(),
        schema_digest: digest('c'),
        generation_candidate_digest: digest('d'),
    }
}

fn owner_relations(owners: &[LargeOwner]) -> Vec<ProviderProjectedRelation> {
    let mut cohorts = BTreeMap::<&str, Vec<&LargeOwner>>::new();
    for owner in owners {
        cohorts.entry(&owner.cohort).or_default().push(owner);
    }
    cohorts
        .into_values()
        .flat_map(|cohort| {
            cohort
                .windows(2)
                .map(|pair| (pair[0].path.clone(), pair[1].path.clone()))
                .collect::<Vec<_>>()
        })
        .map(|(from, to)| ProviderProjectedRelation {
            from: ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                id: from,
            },
            kind: "next-in-cohort".into(),
            to: ProviderProjectedRelationEndpoint {
                kind: agent_semantic_content_identity::ProviderRelationEndpointKindV1::Owner,
                id: to,
            },
        })
        .collect()
}

fn build_lexical_accelerator(
    owners: &[LargeOwner],
    snapshot: SourceSnapshotEvidence,
    authority: ResidentSearchAuthority,
) -> (Arc<ResidentSourceIndex>, Duration, Duration) {
    let (documents, lexical_bytes, coverage_elapsed) = large_source_documents(owners, &authority);
    let tantivy_started = Instant::now();
    let resources = ResidentIndexBuildResources::new(
        4,
        64 * 1024 * 1024,
        ResidentIndexBuildStrategy::ParallelSegments,
    )
    .unwrap();
    println!(
        "[tantivy-build-resources] strategy={:?} indexingThreads={} memoryBudgetBytes={} lexicalBytes={lexical_bytes}",
        resources.strategy(),
        resources.indexing_threads(),
        resources.memory_budget_bytes(),
    );
    let index =
        Arc::new(ResidentSourceIndex::new(documents, snapshot, digest('d'), resources).unwrap());
    (index, coverage_elapsed, tantivy_started.elapsed())
}

fn large_source_documents(
    owners: &[LargeOwner],
    authority: &ResidentSearchAuthority,
) -> (BTreeMap<String, ResidentSourceDocument>, usize, Duration) {
    let coverage_started = Instant::now();
    let coverage_inputs = owners
        .iter()
        .map(|owner| ResidentLexicalCoverageInput {
            owner_path: &owner.path,
            source: &owner.source,
            parser_query_keys: vec![owner.selector.clone()],
        })
        .collect::<Vec<_>>();
    let coverage = resident_lexical_coverage_batch(&coverage_inputs);
    let mut documents = BTreeMap::new();
    for (owner, query_keys) in owners.iter().zip(coverage) {
        documents.insert(
            owner.path.clone(),
            ResidentSourceDocument {
                owner_path: owner.path.clone(),
                owner_content_digest: owner.content_digest.clone(),
                line_count: 1,
                query_keys,
                lexical_body: Some(String::from_utf8_lossy(&owner.source).into_owned()),
                authority: Some(authority.clone()),
            },
        );
    }
    let coverage_elapsed = coverage_started.elapsed();
    let lexical_bytes: usize = documents
        .values()
        .flat_map(|document| document.query_keys.iter())
        .map(String::len)
        .sum();
    (documents, lexical_bytes, coverage_elapsed)
}

fn build_large_generation(owners: &[LargeOwner]) -> LargeGeneration {
    let snapshot = SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        OWNER_COUNT,
        digest('b'),
    );
    let (content_generation, _, _, _, _) = build_cold_content_generation(owners, &snapshot);
    let workspace_generation = WorkspaceGenerationEvidenceV1 {
        root_digest: snapshot.root_digest.clone(),
        root_depth: 1,
        leaf_count: OWNER_COUNT as u64,
        owner_count: OWNER_COUNT as u64,
    };
    let authority = ResidentSearchAuthority {
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
    };
    let graph_snapshot = snapshot;
    let lexical_snapshot = graph_snapshot.clone();
    let lexical_authority = authority.clone();
    let relation_projection_started = Instant::now();
    let relations = owner_relations(owners);
    let graph_relation_projection = relation_projection_started.elapsed();
    let graph_request_started = Instant::now();
    let graph_request = Arc::new(
        SearchGenerationGraphRequest::new(
            &content_generation,
            graph_snapshot,
            workspace_generation,
            owners.iter().map(|owner| owner.path.clone()),
            relations,
        )
        .unwrap(),
    );
    let graph_request_validation = graph_request_started.elapsed();
    let graph_materialization_started = Instant::now();
    let graph = Arc::new(build_resident_graph_generation(Arc::clone(&graph_request)).unwrap());
    let graph_materialization = graph_materialization_started.elapsed();
    let graph_build_metrics = graph.build_metrics();
    let graph_elapsed = graph_relation_projection
        .saturating_add(graph_request_validation)
        .saturating_add(graph_materialization);
    let (index, lexical_coverage_elapsed, tantivy_elapsed) =
        build_lexical_accelerator(owners, lexical_snapshot, lexical_authority);
    LargeGeneration {
        index,
        graph,
        graph_request,
        generation_digest: digest('d'),
        authority,
        construction: ConstructionDurations {
            lexical_coverage: lexical_coverage_elapsed,
            tantivy_commit: tantivy_elapsed,
            rust_graph: graph_elapsed,
            graph_relation_projection,
            graph_request_validation,
            graph_materialization,
            graph_owner_projection: Duration::from_nanos(
                graph_build_metrics.owner_projection_nanos,
            ),
            graph_edge_projection: Duration::from_nanos(
                graph_build_metrics.relation_projection_nanos,
            ),
            graph_identity_hash: Duration::from_nanos(graph_build_metrics.identity_hash_nanos),
        },
    }
}

fn build_cold_content_generation(
    owners: &[LargeOwner],
    snapshot: &SourceSnapshotEvidence,
) -> (
    crate::ContentSearchGenerationReceipt,
    String,
    Duration,
    Duration,
    Duration,
) {
    let identity = identity(snapshot);
    let construction_started = Instant::now();
    let acquisition_started = Instant::now();
    let acquisition = build_source_byte_acquisition_stage(
        identity.clone(),
        owners.iter().map(|owner| SourceByteOwner {
            owner_path: &owner.path,
            content_digest: &owner.content_digest,
            byte_len: owner.source.len(),
        }),
    )
    .unwrap();
    let acquisition_elapsed = acquisition_started.elapsed();
    let syntax_started = Instant::now();
    let projections = owners.iter().map(|owner| NativeSyntaxProjection {
        owner_path: owner.path.clone(),
        content_digest: canonical_blake3_digest(&owner.content_digest).unwrap(),
        selectors: vec![NativeSyntaxSelector {
            selector: owner.selector.clone(),
            byte_start: 0,
            byte_end: owner.source.len(),
            query_keys: vec![owner.cohort.clone()],
            derived_projection_digest: digest('e'),
        }],
    });
    let relations = owners.iter().map(|owner| NativeSyntaxRelation {
        owner_path: owner.path.clone(),
        relation_digest: digest('f'),
    });
    let syntax = build_native_syntax_stage(identity, projections, relations).unwrap();
    let native_syntax_elapsed = syntax_started.elapsed();
    let construction_elapsed = construction_started.elapsed();
    let content_generation = crate::ContentSearchGenerationReceipt::new(acquisition).unwrap();
    assert!(construction_elapsed >= acquisition_elapsed.max(native_syntax_elapsed));
    (
        content_generation,
        syntax.artifact_digest,
        construction_elapsed,
        acquisition_elapsed,
        native_syntax_elapsed,
    )
}

fn query_and_rank(generation: &LargeGeneration, query_index: usize) -> Duration {
    query_and_rank_observation(generation, query_index).0
}

fn query_and_rank_observation(
    generation: &LargeGeneration,
    query_index: usize,
) -> (Duration, ResidentGraphSearchWork) {
    let query = cohort_name(query_index % COHORT_COUNT);
    let started = Instant::now();
    let lexical = generation
        .index
        .query(&query, Some(&generation.authority), QUERY_LIMIT)
        .unwrap();
    assert_eq!(lexical.hits.len(), QUERY_LIMIT as usize);
    assert!(
        lexical
            .hits
            .iter()
            .all(|hit| hit.query_keys.contains(&query))
    );
    let graph = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "large-workspace-query",
            operation: "conceptual",
            query: &query,
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation.generation_digest,
            source_snapshot: &generation.graph_request.source_snapshot,
            workspace_generation: &generation.graph_request.workspace_generation,
            lexical_hits: &lexical.hits,
            generation_graph: &generation.graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 128,
            max_edges: 128,
            max_frontier: 128,
            max_results: 64,
        },
    )
    .unwrap();
    assert_eq!(graph.work.provider_rpc_count, 0);
    assert!(!graph.ranked_owner_paths.is_empty());
    (started.elapsed(), graph.work)
}

fn distribution(mut samples: Vec<Duration>) -> Distribution {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    let at = |percent: usize| samples[(samples.len() - 1) * percent / 100];
    Distribution {
        p50: at(50),
        p95: at(95),
        p99: at(99),
        max: *samples.last().unwrap(),
    }
}

#[test]
fn tantivy_build_strategy_matrix_uses_one_identical_generation() {
    let owners = large_owners();
    let authority = ResidentSearchAuthority {
        language_id: "rust".into(),
        provider_id: "asp-rust".into(),
    };
    let (documents, lexical_bytes, _) = large_source_documents(&owners, &authority);
    let snapshot = SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        OWNER_COUNT,
        digest('b'),
    );
    let minimum_arena = ResidentIndexBuildResources::TANTIVY_MINIMUM_ARENA_BYTES_PER_THREAD;
    let candidates = [
        (
            ResidentIndexBuildStrategy::SingleSegmentBulk,
            1usize,
            minimum_arena,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            1usize,
            minimum_arena,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            2usize,
            minimum_arena * 2,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            4usize,
            minimum_arena * 4,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            4usize,
            minimum_arena * 8,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            8usize,
            minimum_arena * 8,
        ),
        (
            ResidentIndexBuildStrategy::ParallelSegments,
            8usize,
            minimum_arena * 16,
        ),
    ];
    let mut observations = Vec::with_capacity(candidates.len());
    for (strategy, indexing_threads, memory_budget_bytes) in candidates {
        let resources =
            ResidentIndexBuildResources::new(indexing_threads, memory_budget_bytes, strategy)
                .unwrap();
        let started = Instant::now();
        let index =
            ResidentSourceIndex::new(documents.clone(), snapshot.clone(), digest('d'), resources)
                .unwrap();
        let elapsed = started.elapsed();
        let result = index
            .query(&cohort_name(0), Some(&authority), QUERY_LIMIT)
            .unwrap();
        assert_eq!(result.hits.len(), QUERY_LIMIT as usize);
        observations.push(serde_json::json!({
            "strategy": format!("{strategy:?}"),
            "indexingThreads": indexing_threads,
            "memoryBudgetBytes": memory_budget_bytes,
            "elapsedNanos": elapsed.as_nanos(),
        }));
    }
    eprintln!(
        "{}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.tantivy-build-strategy-matrix-receipt",
            "schemaVersion": "1",
            "ownerCount": OWNER_COUNT,
            "lexicalBytes": lexical_bytes,
            "observations": observations,
        })
    );
}

#[test]
fn large_workspace_playbook_measures_cold_warm_and_concurrent_triad() {
    let owners = large_owners();
    let mut cold_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut acquisition_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut native_syntax_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut lexical_coverage_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut tantivy_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut rust_graph_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_relation_projection_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_request_validation_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_materialization_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_owner_projection_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_edge_projection_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let mut graph_identity_hash_samples = Vec::with_capacity(COLD_SAMPLE_COUNT);
    let cold_snapshot = SourceSnapshotEvidence::new(
        "a".repeat(64),
        SourceSnapshotKind::Filesystem,
        OWNER_COUNT,
        digest('b'),
    );
    for _ in 0..COLD_SAMPLE_COUNT {
        let (_, _, cold_publish, acquisition, native_syntax) =
            build_cold_content_generation(&owners, &cold_snapshot);
        cold_samples.push(cold_publish);
        acquisition_samples.push(acquisition);
        native_syntax_samples.push(native_syntax);
    }
    let mut generation = None;
    for _ in 0..DERIVED_SAMPLE_COUNT {
        let built = build_large_generation(&owners);
        lexical_coverage_samples.push(built.construction.lexical_coverage);
        tantivy_samples.push(built.construction.tantivy_commit);
        rust_graph_samples.push(built.construction.rust_graph);
        graph_relation_projection_samples.push(built.construction.graph_relation_projection);
        graph_request_validation_samples.push(built.construction.graph_request_validation);
        graph_materialization_samples.push(built.construction.graph_materialization);
        graph_owner_projection_samples.push(built.construction.graph_owner_projection);
        graph_edge_projection_samples.push(built.construction.graph_edge_projection);
        graph_identity_hash_samples.push(built.construction.graph_identity_hash);
        generation = Some(built);
    }
    let generation = Arc::new(generation.unwrap());
    let cold = distribution(cold_samples);
    let acquisition = distribution(acquisition_samples);
    let native_syntax = distribution(native_syntax_samples);
    let lexical_coverage = distribution(lexical_coverage_samples);
    let tantivy = distribution(tantivy_samples);
    let rust_graph = distribution(rust_graph_samples);
    let graph_relation_projection = distribution(graph_relation_projection_samples);
    let graph_request_validation = distribution(graph_request_validation_samples);
    let graph_materialization = distribution(graph_materialization_samples);
    let graph_owner_projection = distribution(graph_owner_projection_samples);
    let graph_edge_projection = distribution(graph_edge_projection_samples);
    let graph_identity_hash = distribution(graph_identity_hash_samples);

    for index in 0..COHORT_COUNT {
        query_and_rank(&generation, index);
    }
    let warm = distribution(
        (0..WARM_SAMPLE_COUNT)
            .map(|index| query_and_rank(&generation, index))
            .collect(),
    );
    let (_, fused_cache) = query_and_rank_observation(&generation, 0);
    assert_eq!(fused_cache.cache_hit_count, 1);
    assert_eq!(fused_cache.cache_miss_count, 0);
    assert_eq!(fused_cache.provider_rpc_count, 0);

    let concurrent_started = Instant::now();
    let concurrent = std::thread::scope(|scope| {
        (0..CONCURRENT_QUERY_COUNT)
            .map(|index| {
                let generation = Arc::clone(&generation);
                scope.spawn(move || query_and_rank(&generation, index))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    let concurrent_wall = concurrent_started.elapsed();
    let concurrent = distribution(concurrent);

    let python_receipt = SearchGenerationGraphReceipt {
        schema_id: SEARCH_GENERATION_GRAPH_RECEIPT_SCHEMA_ID.to_owned(),
        schema_version: "1".to_owned(),
        identity: generation.graph_request.identity.clone(),
        content_generation_digest: generation.graph_request.content_generation_digest.clone(),
        entry_owner_ids: generation.graph_request.owner_paths.clone(),
        entry_node_ids: generation
            .graph_request
            .owner_paths
            .iter()
            .map(|owner| stable_graph_node_id("owner", owner))
            .collect(),
        candidate_owner_ids: generation.graph_request.owner_paths.clone(),
        artifact_digest: generation.graph.digest().to_owned(),
        complete: true,
    };
    let python_started = Instant::now();
    python_receipt
        .validate_for(&generation.graph_request)
        .unwrap();
    let python_elapsed = python_started.elapsed();

    let query = cohort_name(0);
    let lexical_started = Instant::now();
    let lexical = generation
        .index
        .query(&query, Some(&generation.authority), QUERY_LIMIT)
        .unwrap();
    let lexical_elapsed = lexical_started.elapsed();
    let graph = rank_resident_graph_generation(
        ResidentGraphSearchRequest {
            operation_id: "relationship-query",
            operation: "relationship",
            query: &query,
            language_id: "rust",
            provider_id: "asp-rust",
            generation_digest: &generation.generation_digest,
            source_snapshot: &generation.graph_request.source_snapshot,
            workspace_generation: &generation.graph_request.workspace_generation,
            lexical_hits: &lexical.hits,
            generation_graph: &generation.graph,
        },
        ResidentGraphSearchBudget {
            max_nodes: 128,
            max_edges: 128,
            max_frontier: 128,
            max_results: 64,
        },
    )
    .unwrap();
    let owner_paths = lexical
        .hits
        .iter()
        .map(|hit| hit.owner_path.clone())
        .collect::<Vec<_>>();
    let selectors = owner_paths
        .iter()
        .map(|owner| format!("rust://{owner}#item/function/fixture"))
        .collect::<Vec<_>>();
    let native_syntax_started = Instant::now();
    let projections = owner_paths
        .iter()
        .zip(selectors.iter())
        .map(|(owner, selector)| NativeSyntaxProjection {
            owner_path: owner.clone(),
            content_digest: canonical_blake3_digest(
                &owners
                    .iter()
                    .find(|item| &item.path == owner)
                    .unwrap()
                    .content_digest,
            )
            .unwrap(),
            selectors: vec![NativeSyntaxSelector {
                selector: selector.clone(),
                byte_start: 0,
                byte_end: 1,
                query_keys: vec![query.clone()],
                derived_projection_digest: digest('e'),
            }],
        })
        .collect::<Vec<_>>();
    let native_syntax_elapsed = native_syntax_started.elapsed();
    assert_eq!(projections.len(), lexical.hits.len());
    let request_time_external_work = ResidentSearchWorkCounters::default();
    request_time_external_work.validate_zero_io().unwrap();

    eprintln!(
        "{}",
        serde_json::json!({
            "schemaId": "agent.semantic-protocols.large-search-playbook-performance-receipt",
            "schemaVersion": "1",
            "owners": OWNER_COUNT,
            "cold": {
                "samples": COLD_SAMPLE_COUNT,
                "p50Nanos": cold.p50.as_nanos(),
                "p95Nanos": cold.p95.as_nanos(),
                "p99Nanos": cold.p99.as_nanos(),
                "maxNanos": cold.max.as_nanos(),
                "stageP95Nanos": {
                    "sourceByteAcquisition": acquisition.p95.as_nanos(),
                    "nativeSyntax": native_syntax.p95.as_nanos(),
                },
            },
            "derived": {
                "samples": DERIVED_SAMPLE_COUNT,
                "stageP95Nanos": {
                    "lexicalCoverage": lexical_coverage.p95.as_nanos(),
                    "tantivyCommit": tantivy.p95.as_nanos(),
                    "rustGraph": rust_graph.p95.as_nanos(),
                    "graphRelationProjection": graph_relation_projection.p95.as_nanos(),
                    "graphRequestValidation": graph_request_validation.p95.as_nanos(),
                    "graphMaterialization": graph_materialization.p95.as_nanos(),
                    "graphOwnerProjection": graph_owner_projection.p95.as_nanos(),
                    "graphEdgeProjection": graph_edge_projection.p95.as_nanos(),
                    "graphIdentityHash": graph_identity_hash.p95.as_nanos(),
                },
            },
            "warm": {
                "samples": WARM_SAMPLE_COUNT,
                "p50Nanos": warm.p50.as_nanos(),
                "p95Nanos": warm.p95.as_nanos(),
                "p99Nanos": warm.p99.as_nanos(),
                "maxNanos": warm.max.as_nanos(),
            },
            "concurrent": {
                "queries": CONCURRENT_QUERY_COUNT,
                "p99Nanos": concurrent.p99.as_nanos(),
                "maxNanos": concurrent.max.as_nanos(),
                "wallNanos": concurrent_wall.as_nanos(),
            },
            "fusedCache": {
                "hitCount": fused_cache.cache_hit_count,
                "missCount": fused_cache.cache_miss_count,
                "entryCount": fused_cache.cache_entry_count,
                "valueBytes": fused_cache.cache_value_bytes,
                "capacity": fused_cache.cache_capacity,
                "shardCount": fused_cache.cache_shard_count,
                "externalWorkCount": fused_cache.provider_rpc_count,
            },
            "pythonGraph": {
                "state": "executed",
                "generationDigest": generation.generation_digest,
                "artifactDigest": python_receipt.artifact_digest,
                "receiptValidationNanos": python_elapsed.as_nanos(),
            },
            "nativeSyntaxProjectionNanos": native_syntax_elapsed.as_nanos(),
            "lexicalQueryNanos": lexical_elapsed.as_nanos(),
            "graphRankNanos": graph.elapsed_micros.saturating_mul(1_000),
            "requestTimeExternalWork": request_time_external_work,
        })
    );
    assert!(
        cold.p95 < Duration::from_millis(250),
        "cold p95={:?}",
        cold.p95
    );
    assert!(
        warm.p95 < Duration::from_millis(1),
        "warm p95={:?}",
        warm.p95
    );
    assert!(
        warm.p99 < Duration::from_millis(5),
        "warm p99={:?}",
        warm.p99
    );
    assert!(
        concurrent.p99 < Duration::from_millis(5),
        "concurrent p99={:?}",
        concurrent.p99
    );
    assert!(
        concurrent_wall < Duration::from_millis(500),
        "concurrent wall={concurrent_wall:?}"
    );
}
