// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Unit tests for cached zero-copy `mmap` open, incremental disk sync, resident
//! playbook execution, and native symbol + 1-hop import/reference graph axes.

use std::collections::BTreeSet;
use std::path::PathBuf;

use agent_semantic_search::{
    SEMANTIC_OPEN_EDB_SCHEMA_ID, SEMANTIC_OPEN_EDB_SCHEMA_VERSION, SearchPlaybookPlanInput,
    WorkspaceEdbAttr, WorkspaceEdbEdgeFact, WorkspaceEdbEntityFact, WorkspaceEdbPrefilter,
    WorkspaceFileDelta, WorkspaceIndexConfig, WorkspaceOpenEdbIngestBatch, WorkspaceSearchIndex,
    WorkspaceSearchQuery, WorkspaceSymbolKind, admit_search_playbook_plan,
    execute_admitted_playbook, execute_admitted_playbook_with_index,
};
use agent_semantic_search_playbook::{SearchPlaybookComposition, SearchPlaybookLeaf};

#[test]
fn cached_mmap_open_and_disk_sync_plus_resident_playbook_execution() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path().join("workspace");
    let snap = tmp.path().join("cache/workspace.asp-index.bin");
    std::fs::create_dir_all(ws.join("src")).expect("create src");
    std::fs::write(
        ws.join("src/alpha.rs"),
        "pub fn alpha_search_kernel() -> usize { 1 }\n",
    )
    .expect("write alpha");
    std::fs::write(
        ws.join("src/beta.rs"),
        "pub fn beta_other_helper() -> usize { 2 }\n",
    )
    .expect("write beta");

    let cfg = WorkspaceIndexConfig::default();
    let cold = WorkspaceSearchIndex::open_or_build_cached(&ws, &snap, cfg.clone());
    assert!(snap.is_file());
    assert_eq!(cold.file_count(), 2);

    let mut warm = WorkspaceSearchIndex::open_or_build_cached(&ws, &snap, cfg.clone());
    assert!(warm.is_mmap_backed());
    assert_eq!(warm.generation_digest(), cold.generation_digest());

    std::fs::write(
        ws.join("src/alpha.rs"),
        "pub fn alpha_search_kernel_v2() -> usize { 42 }\n",
    )
    .expect("update alpha");
    std::fs::write(
        ws.join("src/gamma.rs"),
        "pub fn gamma_resident_engine() -> usize { 7 }\n",
    )
    .expect("write gamma");
    std::fs::remove_file(ws.join("src/beta.rs")).expect("remove beta");

    let receipt = warm.sync_workspace_from_disk();
    assert_eq!(receipt.upserted_files, 2);
    assert_eq!(receipt.deleted_files, 1);
    assert_eq!(receipt.live_files, 2);

    let fresh = WorkspaceSearchIndex::build_with_config(&ws, cfg);
    assert_eq!(warm.generation_digest(), fresh.generation_digest());

    let plan = SearchPlaybookPlanInput {
        workspace: Some("virtual-ws".to_owned()),
        languages: vec!["rust".to_owned()],
        documents: Vec::new(),
        composition: SearchPlaybookComposition::Leaf(SearchPlaybookLeaf::Rg(vec![
            "-n".to_owned(),
            "(alpha_search_kernel_v2|gamma_resident_engine)".to_owned(),
        ])),
    };
    let admitted = admit_search_playbook_plan(plan).expect("admit plan");
    let resident_report =
        execute_admitted_playbook_with_index(&warm, &admitted).expect("resident playbook");
    let disk_report = execute_admitted_playbook(&ws, &admitted).expect("disk playbook");

    assert_eq!(resident_report.execution_mode, "pure-rust-resident-index");
    let res_paths: BTreeSet<String> = resident_report.hits.into_iter().map(|h| h.path).collect();
    let disk_paths: BTreeSet<String> = disk_report.hits.into_iter().map(|h| h.path).collect();
    assert_eq!(res_paths, disk_paths);
    assert_eq!(res_paths.len(), 2);
}

#[test]
fn resident_symbol_and_graph_axes_locate_definitions_importers_and_callers() {
    let mut index = WorkspaceSearchIndex::build_from_memory(
        PathBuf::from("/virtual/sym-graph"),
        vec![
            (
                "src/engine.rs",
                b"use crate::storage::StoragePort;\n\npub struct SearchKernel {}\n\npub fn run_kernel_query(port: &StoragePort) -> usize { 1 }\n".to_vec(),
            ),
            (
                "src/storage.rs",
                b"pub trait StoragePort {\n    fn load_blob(&self) -> usize;\n}\n".to_vec(),
            ),
            (
                "src/caller.rs",
                b"use crate::engine::run_kernel_query;\n\npub fn invoke_search() -> usize {\n    run_kernel_query(todo!())\n}\n".to_vec(),
            ),
            (
                "scheme/helm.ss",
                b"(import :marlin/engine)\n(def (helm-open-session q) q)\n".to_vec(),
            ),
        ],
        WorkspaceIndexConfig::default(),
    );

    let sym_report = index.query(&WorkspaceSearchQuery {
        symbol: Some("run_kernel_query"),
        max_hits: 16,
        context_radius: 2,
        ..Default::default()
    });
    assert_eq!(sym_report.hits.len(), 1);
    assert_eq!(sym_report.hits[0].path, "src/engine.rs");
    assert_eq!(sym_report.hits[0].first_line, Some(5));
    assert!(
        sym_report.hits[0]
            .matched_axes
            .iter()
            .any(|a| a == "symbol")
    );
    assert_eq!(sym_report.hits[0].symbols[0].name, "run_kernel_query");
    assert_eq!(
        sym_report.hits[0].symbols[0].kind,
        WorkspaceSymbolKind::Function
    );

    let scheme_sym = index.query(&WorkspaceSearchQuery {
        symbol: Some("helm-open-session"),
        max_hits: 16,
        context_radius: 2,
        ..Default::default()
    });
    assert_eq!(scheme_sym.hits.len(), 1);
    assert_eq!(scheme_sym.hits[0].path, "scheme/helm.ss");
    assert_eq!(scheme_sym.hits[0].first_line, Some(2));

    let graph_report = index.query(&WorkspaceSearchQuery {
        graph_target: Some("run_kernel_query"),
        max_hits: 16,
        context_radius: 2,
        ..Default::default()
    });
    let graph_paths: BTreeSet<&str> = graph_report.hits.iter().map(|h| h.path.as_str()).collect();
    assert!(
        graph_paths.contains("src/engine.rs"),
        "missing definer: {graph_paths:?}"
    );
    assert!(
        graph_paths.contains("src/caller.rs"),
        "missing caller/importer: {graph_paths:?}"
    );
    assert!(
        graph_paths.contains("src/storage.rs"),
        "missing outgoing dependency: {graph_paths:?}"
    );
    assert!(
        graph_report
            .hits
            .iter()
            .find(|h| h.path == "src/caller.rs")
            .is_some_and(|h| h
                .graph_relations
                .iter()
                .any(|r| r.contains("run_kernel_query")))
    );

    index.apply_file_delta(WorkspaceFileDelta {
        upserts: vec![(
            "src/engine.rs".to_owned(),
            b"pub fn run_kernel_query_v3() -> usize { 3 }\n".to_vec(),
        )],
        deletes: Vec::new(),
    });
    let updated_sym = index.query(&WorkspaceSearchQuery {
        symbol: Some("run_kernel_query_v3"),
        max_hits: 16,
        context_radius: 2,
        ..Default::default()
    });
    assert_eq!(updated_sym.hits.len(), 1);
    assert_eq!(updated_sym.hits[0].path, "src/engine.rs");
    assert_eq!(updated_sym.hits[0].first_line, Some(1));
}

#[test]
fn open_edb_ingests_native_parser_protocol_facts_falls_back_to_resident_index_and_invalidates_on_delta()
 {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("crates/tooling/src")).expect("mkdir tooling");
    std::fs::create_dir_all(ws.join("crates/unrelated/src")).expect("mkdir unrelated");
    std::fs::create_dir_all(ws.join("scheme/search-dsl")).expect("mkdir scheme");
    std::fs::create_dir_all(ws.join("dev/sysml")).expect("mkdir sysml");

    std::fs::write(
        ws.join("crates/tooling/src/search.rs"),
        "use crate::runtime::ToolRuntime;\npub struct ResidentSearchHandler;\nimpl ToolRuntime for ResidentSearchHandler {\n    pub async fn execute_search(&self) -> usize { 1 }\n}\nmacro_rules! register_search_tool { () => {}; }\n",
    )
    .expect("write rust");
    std::fs::write(
        ws.join("scheme/search-dsl/objects.ss"),
        "(import (only-in :clan/poo/object .def .mix))\n(.def RustSearchFragment.\n  language: 'rust)\n(def RustFragmentValue (.mix RustSearchFragment. BaseSearchFragment.))\n",
    )
    .expect("write scheme");
    std::fs::write(
        ws.join("dev/sysml/search.sysml"),
        "package MarlinSearch {\n    requirement <'MAR-SRCH-R01'> openEdbExtraction {}\n}\n",
    )
    .expect("write sysml");
    std::fs::write(
        ws.join("crates/unrelated/src/noise.rs"),
        "pub fn completely_unrelated_helper() {}\n",
    )
    .expect("write noise");

    let mut index = WorkspaceSearchIndex::build_with_config(ws, WorkspaceIndexConfig::default());

    // 1. Before provider ingestion, project_prefiltered_edb streams zero-regex resident symbols.
    let baseline = index.project_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["rust".to_owned()],
        required_tokens: &["ToolRuntime".to_owned()],
        ..Default::default()
    });
    assert_eq!(baseline.candidate_files, 1);
    assert_eq!(baseline.files[0].path, "crates/tooling/src/search.rs");
    assert!(baseline.entities.iter().any(|e| {
        e.kind == "function" && e.name == "execute_search" && e.source == "resident-index"
    }));
    assert!(baseline.entities.iter().any(|e| {
        e.kind == "macro" && e.name == "register_search_tool" && e.source == "resident-index"
    }));

    // 2. Deserialize & ingest a semantic-open-edb.v1 JSON payload as emitted by asp-rust.
    let rust_wire_json = serde_json::json!({
        "schemaId": SEMANTIC_OPEN_EDB_SCHEMA_ID,
        "schemaVersion": SEMANTIC_OPEN_EDB_SCHEMA_VERSION,
        "languageId": "rust",
        "providerId": "asp-rust",
        "entities": [
            {
                "path": "crates/tooling/src/search.rs",
                "line": 2,
                "name": "ResidentSearchHandler",
                "kind": "struct",
                "languageKind": "struct_item",
                "source": "native-parser"
            },
            {
                "path": "crates/tooling/src/search.rs",
                "line": 3,
                "name": "ResidentSearchHandler",
                "kind": "item",
                "languageKind": "impl_item",
                "source": "native-parser",
                "attrs": [
                    { "key": "trait", "value": "ToolRuntime" }
                ]
            },
            {
                "path": "crates/tooling/src/search.rs",
                "line": 4,
                "name": "execute_search",
                "kind": "method",
                "languageKind": "fn_item",
                "source": "native-parser"
            },
            {
                "path": "crates/tooling/src/search.rs",
                "line": 6,
                "name": "register_search_tool",
                "kind": "macro",
                "languageKind": "macro_rules",
                "source": "native-parser"
            }
        ],
        "edges": [
            {
                "src": "ResidentSearchHandler",
                "relation": "implements",
                "dst": "ToolRuntime",
                "path": "crates/tooling/src/search.rs",
                "line": 3
            }
        ]
    });
    let rust_batch: WorkspaceOpenEdbIngestBatch =
        serde_json::from_value(rust_wire_json).expect("decode semantic-open-edb.v1 batch");
    let rust_receipt = index.ingest_open_edb_facts(rust_batch);
    assert_eq!(rust_receipt.updated_files, 1);
    assert_eq!(rust_receipt.ingested_entities, 4);
    assert_eq!(rust_receipt.ingested_edges, 1);
    assert_eq!(rust_receipt.skipped_facts, 0);

    // 3. Ingest Scheme (:gerbil-parser) and SysML native facts directly.
    let scheme_receipt = index.ingest_open_edb_facts(WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some("scheme".to_owned()),
        provider_id: Some("marlin-gerbil-parser".to_owned()),
        entities: vec![WorkspaceEdbEntityFact {
            entity_id: String::new(),
            file_id: 0,
            path: "scheme/search-dsl/objects.ss".to_owned(),
            line: 2,
            name: "RustSearchFragment.".to_owned(),
            language: "scheme".to_owned(),
            kind: "item".to_owned(),
            language_kind: "poo_def".to_owned(),
            source: "gerbil-parser".to_owned(),
            attrs: vec![WorkspaceEdbAttr {
                key: "form".to_owned(),
                value: ".def".to_owned(),
            }],
        }],
        edges: vec![WorkspaceEdbEdgeFact {
            src: "RustSearchFragment.".to_owned(),
            relation: "mixes".to_owned(),
            dst: "BaseSearchFragment.".to_owned(),
            path: "scheme/search-dsl/objects.ss".to_owned(),
            line: 4,
        }],
    });
    assert_eq!(scheme_receipt.updated_files, 1);

    let sysml_receipt = index.ingest_open_edb_facts(WorkspaceOpenEdbIngestBatch {
        schema_id: Some(SEMANTIC_OPEN_EDB_SCHEMA_ID.to_owned()),
        schema_version: Some(SEMANTIC_OPEN_EDB_SCHEMA_VERSION.to_owned()),
        language_id: Some("sysml".to_owned()),
        provider_id: Some("marlin-gerbil-parser".to_owned()),
        entities: vec![WorkspaceEdbEntityFact {
            entity_id: String::new(),
            file_id: 0,
            path: "dev/sysml/search.sysml".to_owned(),
            line: 2,
            name: "openEdbExtraction".to_owned(),
            language: "sysml".to_owned(),
            kind: "item".to_owned(),
            language_kind: "requirement_def".to_owned(),
            source: "gerbil-parser".to_owned(),
            attrs: vec![WorkspaceEdbAttr {
                key: "reqId".to_owned(),
                value: "MAR-SRCH-R01".to_owned(),
            }],
        }],
        edges: Vec::new(),
    });
    assert_eq!(sysml_receipt.updated_files, 1);

    // 4. Verify trigram-prefiltered projection preserves native kind + language_kind + edges.
    let rust_edb = index.project_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["rust".to_owned()],
        required_tokens: &["ToolRuntime".to_owned()],
        ..Default::default()
    });
    assert_eq!(rust_edb.candidate_files, 1);
    assert!(rust_edb.entities.iter().any(|e| {
        e.kind == "item"
            && e.language_kind == "impl_item"
            && e.name == "ResidentSearchHandler"
            && e.source == "native-parser"
            && e.attrs
                .iter()
                .any(|a| a.key == "trait" && a.value == "ToolRuntime")
    }));
    assert!(rust_edb.entities.iter().any(|e| {
        e.kind == "macro" && e.language_kind == "macro_rules" && e.name == "register_search_tool"
    }));
    assert!(rust_edb.edges.iter().any(|edge| {
        edge.relation == "implements"
            && edge.src == "ResidentSearchHandler"
            && edge.dst == "ToolRuntime"
    }));

    let scheme_edb = index.project_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["scheme".to_owned()],
        ..Default::default()
    });
    assert_eq!(scheme_edb.candidate_files, 1);
    assert!(scheme_edb.entities.iter().any(|e| {
        e.kind == "item"
            && e.language_kind == "poo_def"
            && e.name == "RustSearchFragment."
            && e.source == "gerbil-parser"
    }));
    assert!(scheme_edb.edges.iter().any(|edge| {
        edge.relation == "mixes"
            && edge.src == "RustSearchFragment."
            && edge.dst == "BaseSearchFragment."
    }));

    let sysml_edb = index.project_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["sysml".to_owned()],
        ..Default::default()
    });
    assert_eq!(sysml_edb.entities.len(), 1);
    assert_eq!(sysml_edb.entities[0].language_kind, "requirement_def");
    assert_eq!(sysml_edb.entities[0].name, "openEdbExtraction");

    // 5. Updating a file via apply_file_delta automatically invalidates stale provider facts.
    index.apply_file_delta(WorkspaceFileDelta {
        upserts: vec![(
            "crates/tooling/src/search.rs".to_owned(),
            b"pub fn execute_search_v2() -> usize { 2 }\n".to_vec(),
        )],
        deletes: Vec::new(),
    });
    let after_delta = index.project_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["rust".to_owned()],
        required_tokens: &["execute_search_v2".to_owned()],
        ..Default::default()
    });
    assert_eq!(after_delta.candidate_files, 1);
    assert!(
        !after_delta
            .entities
            .iter()
            .any(|e| e.language_kind == "impl_item")
    );
    assert!(after_delta.entities.iter().any(|e| {
        e.kind == "function" && e.name == "execute_search_v2" && e.source == "resident-index"
    }));
}
