// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

package AspSearchEngine {
    private import ScalarValues::*;

    doc /* OpenSysML v2 engineering design for the converged ASP Search Engine
           aligned with Marlin Agent CORE's `dev/sysml` standard. */

    enum def ProviderSyncProfile {
        enum core;
        enum analyzers;
        enum docs;
        enum 'all';
    }

    enum def SearchAxisKind {
        enum lexicalRg;
        enum tantivy;
        enum openEdbFacet;
        enum topology;
        enum graph;
    }

    attribute def ProviderPackageSpec {
        attribute packageId : String;
        attribute path : String;
        attribute repoUrl : String;
        attribute lockedRev : String;
        attribute profile : ProviderSyncProfile;
        doc /* Declarative package entry owned by root `providers.toml`
               replacing `.gitmodules` for external language providers. */
    }

    attribute def GenerationPointerSwap {
        attribute projectId : String;
        attribute workspaceId : String;
        attribute pointerKey : String;
        attribute expectedArtifactHash : String [0..1];
        attribute newGenerationDigest : String;
        attribute newArtifactHash : String;
        doc /* Typed compare-and-swap request across `SearchEngineStoragePort`. */
    }

    attribute def SearchHotPathProfile {
        attribute benchmarkName : String;
        attribute candidateScale : Integer;
        attribute deterministicOrdering : Boolean;
        doc /* Criterion throughput and latency qualification contract. */
    }

    part <'ASP-SRCH-A01'> actual {
        part searchCore {
            doc /* `agent-semantic-search`, `agent-semantic-search-playbook`,
                   `agent-semantic-search-projection`, `agent-semantic-topology`,
                   and `agent-semantic-content-identity` (zero `agent-semantic-tree-sitter`,
                   zero `agent-semantic-mrr`, zero Rust `ascent`). */
        }
        part providerBoundary {
            doc /* `agent-semantic-provider-protocol`, `agent-semantic-http-json`,
                   and `agent-semantic-provider-transport` owning external
                   non-Rust language provider HTTP-JSON transport and checked-in
                   resolved register snapshots. */
        }
        part packageManager {
            doc /* `providers.toml`, `agent-semantic-config::providers_manifest`,
                   and `agent-semantic-schema-manager::provider_workspace`
                   exposing pure-Rust `gix`-backed library APIs with zero
                   standalone CLI binaries and zero `.gitmodules`. */
        }
        dependency coreUsesIdentity from searchCore to providerBoundary;
        dependency managerResolvesProviders from packageManager to providerBoundary;
    }

    part <'ASP-SRCH-A02'> target {
        part convergedFacade {
            doc /* `MarlinConvergedSearchEngine`: single entry point wired
                   strictly through `OrgDocumentProjectionPort` and
                   `SearchEngineStoragePort` with zero embedded Orgize FFI
                   and zero embedded Turso runtime. */
        }
        part playbookAdmission {
            doc /* `agent-semantic-search-playbook`: pure typed Rust plan
                   admission accepting structures projected natively from
                   Marlin's Gerbil Scheme `RootedSchemeObject` ABI. */
        }
        part rankingAndIndexing {
            doc /* Deterministic `rank_source_index_report`, `WorkspaceSearchIndex`
                   (compact 6B `loc_mask`/`next_mask` trigram postings,
                   `regex-syntax` HIR boolean query planner, zero-copy `mmap`
                   snapshots, incremental `apply_file_delta` + disk sync,
                   resident structural symbol extraction & 1-hop import/reference
                   graph queries, plus zero-regex protocol-native
                   `ingest_open_edb_facts` / `project_prefiltered_edb` conforming
                   to `schemas/semantic-open-edb.v1.schema.json` streaming open
                   `WorkspaceEdbEntityFact`/`WorkspaceEdbEdgeFact` slices to
                   Marlin's `:gerbil-ascent` + `:gerbil-parser` + POO DSL),
                   `FileLocatorIndex`, `execute_admitted_playbook_with_index`,
                   and in-RAM `Tantivy` BM25/Fuzzy lexical acceleration. */
        }
        part topologyAndOpenEdb {
            doc /* `OrgDocumentProjectionPort`-backed topology manifest/overlay
                   admission and zero-regex generation-scoped Open EDB fact
                   store (`semantic-open-edb.v1`) for native language providers
                   and Marlin's Scheme `:gerbil-ascent` + `:gerbil-parser`
                   reasoning plane (zero regex extractors in Rust, zero
                   pseudo-Tree-sitter `.scm` query layer, and zero Rust `ascent`). */
        }
        part providerTransport {
            doc /* Process and HTTP-JSON boundary reserved solely for external
                   language providers (TypeScript, Python, Julia, Gerbil Scheme). */
        }
        part providerPackageLibrary {
            doc /* Embeddable pure-Rust `providers.toml` workspace library
                   using `gix` for fast local Git inspection (zero standalone
                   Rust CLI binary in ASP). */
        }
        part qualificationSuite {
            doc /* OpenSysML `dev/sysml`, Quint `dev/quint/search-engine.qnt`
                   with negative mutation controls, Rust stateful reference-model
                   tests, and multi-scale Criterion benchmarks. */
        }

        dependency facadeAdmitsPlans from convergedFacade to playbookAdmission;
        dependency facadeRunsRanking from convergedFacade to rankingAndIndexing;
        dependency facadeAdmitsTopology from convergedFacade to topologyAndOpenEdb;
        dependency rankingConsumesProviders from rankingAndIndexing to providerTransport;
        dependency transportSyncedByManifest from providerTransport to providerPackageLibrary;
        dependency suiteQualifiesFacade from qualificationSuite to convergedFacade;
        dependency suiteQualifiesRanking from qualificationSuite to rankingAndIndexing;
    }

    state def SearchGenerationLifecycle {
        doc /* State machine governing content-addressed search generation
               artifacts and CAS workspace pointers. */
        entry; then empty;

        state empty {
            doc /* No active search generation pointer exists for the workspace. */
        }
        state blobStored {
            doc /* Immutable BLAKE3-verified artifact blob stored in host CAS. */
        }
        state active {
            doc /* Workspace pointer atomically points at the verified blob
                   with a monotonic revision number. */
        }
        state casConflict {
            doc /* Expected artifact hash did not match active pointer; active
                   state remains untouched. */
        }

        transition storeBlob
            first empty
            then blobStored;
        transition activateGeneration
            first blobStored
            then active;
        transition rejectStaleSwap
            first blobStored
            then casConflict;
        transition stageSuccessor
            first active
            then blobStored;
        transition retryAfterConflict
            first casConflict
            then active;
    }

    action def ExecuteSearchPipeline {
        in swap : GenerationPointerSwap;
        out profile : SearchHotPathProfile;

        action admitTypedPlan {
            doc /* Admit typed `SearchPlaybookPlanInput` with fail-closed
                   scope and predicate validation. */
        }
        action evaluateRankers {
            doc /* Execute deterministic lexical source-index and graph-owner
                   ranking over resident corpus and admitted graph evidence. */
        }
        action persistGeneration {
            doc /* Write BLAKE3 content-addressed blob and atomically advance
                   the workspace pointer via `SearchEngineStoragePort`. */
        }
        action persistTopologyEdges {
            doc /* Append deduplicated topology edges isolated by
                   `(project_id, generation_digest)`. */
        }

        first admitTypedPlan;
        then evaluateRankers;
        then persistGeneration;
        then persistTopologyEdges;
    }

    requirement <'ASP-SRCH-R01'> zeroLegacyPureRustEngine {
        doc /* ASP search crates contain zero legacy server/daemon shims,
               zero pseudo S-expression text parser, zero `orgize` FFI, and
               zero embedded `turso_core` database engine. */
    }
    requirement <'ASP-SRCH-R02'> hostSuppliedPorts {
        doc /* Host persistence and Org AST parsing are injected exclusively
               through `SearchEngineStoragePort` and `OrgDocumentProjectionPort`. */
    }
    requirement <'ASP-SRCH-R03'> atomicGenerationCas {
        doc /* Generation publication writes immutable verified blobs before
               swapping pointers, guarantees monotonic revisions, rejects stale
               expected hashes without side effects, and keeps topology edges
               idempotent and generation-isolated. */
    }
    requirement <'ASP-SRCH-R04'> declarativeProviderPackaging {
        doc /* External language repositories are managed by `providers.toml`
               with zero `.gitmodules`; clean Cargo git checkouts build
               self-contained without `CARGO_NET_GIT_FETCH_WITH_CLI`. */
    }
    requirement <'ASP-SRCH-R05'> externalProviderTransportBoundary {
        doc /* gRPC streaming transport is scoped strictly to external
               non-Rust language providers and never placed between Marlin
               and ASP. */
    }
    requirement <'ASP-SRCH-R06'> formalAndStatefulVerification {
        doc /* Quint formal models with negative mutation controls and Rust
               stateful reference-model tests verify generation publication,
               playbook admission, and edge isolation invariants. */
    }
    requirement <'ASP-SRCH-R07'> highPerformanceBenchmarks {
        doc /* Multi-scale Criterion benchmarks continuously quantify throughput
               and latency for source-index ranking, graph-owner ranking,
               playbook admission, sorted record lookup, and CAS storage. */
    }

    allocation ownPureRustEngine allocate zeroLegacyPureRustEngine to target.convergedFacade;
    allocation ownHostPorts allocate hostSuppliedPorts to target.convergedFacade;
    allocation ownAtomicCas allocate atomicGenerationCas to target.convergedFacade;
    allocation ownProviderPackaging allocate declarativeProviderPackaging to target.providerPackageLibrary;
    allocation ownProviderTransport allocate externalProviderTransportBoundary to target.providerTransport;
    allocation ownFormalTests allocate formalAndStatefulVerification to target.qualificationSuite;
    allocation ownPerfBenches allocate highPerformanceBenchmarks to target.qualificationSuite;
}
