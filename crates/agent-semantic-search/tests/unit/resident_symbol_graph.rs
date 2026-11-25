// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Unit tests for cached zero-copy `mmap` open, incremental disk sync, resident
//! playbook execution, and native symbol + 1-hop import/reference graph axes.

use std::collections::BTreeSet;
use std::path::PathBuf;

use agent_semantic_search::{
    SearchPlaybookPlanInput, WorkspaceEdbEdgeRule, WorkspaceEdbEntityRule, WorkspaceEdbPrefilter,
    WorkspaceFileDelta, WorkspaceIndexConfig, WorkspaceSearchIndex, WorkspaceSearchQuery,
    WorkspaceSymbolKind, admit_search_playbook_plan, execute_admitted_playbook,
    execute_admitted_playbook_with_index,
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
fn open_edb_extraction_emits_rust_scheme_poo_and_sysml_facts_with_trigram_prefilter() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("crates/tooling/src")).expect("mkdir tooling");
    std::fs::create_dir_all(ws.join("crates/unrelated/src")).expect("mkdir unrelated");
    std::fs::create_dir_all(ws.join("scheme/search-dsl")).expect("mkdir scheme");
    std::fs::create_dir_all(ws.join("dev/sysml")).expect("mkdir sysml");

    std::fs::write(
        ws.join("crates/tooling/src/search.rs"),
        "use crate::runtime::ToolRuntime;\n#[derive(Clone, Debug)]\npub struct ResidentSearchHandler;\nimpl ToolRuntime for ResidentSearchHandler {\n    pub async fn execute_search(&self) -> usize { 1 }\n}\n",
    )
    .expect("write rust");
    std::fs::write(
        ws.join("scheme/search-dsl/objects.ss"),
        "(import (only-in :clan/poo/object .def .mix))\n(.def RustSearchFragment.\n  language: 'rust)\n(def RustFragmentValue (.mix RustSearchFragment. BaseSearchFragment.))\n(c-define (search-compile root) (int64) int64 \"marlin_search_dsl_compile\" \"extern\")\n",
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

    let index = WorkspaceSearchIndex::build_with_config(ws, WorkspaceIndexConfig::default());

    let rust_entity_rules = vec![
        WorkspaceEdbEntityRule {
            language: Some("rust".to_owned()),
            facet_tag: "trait-impl".to_owned(),
            pattern: r"^impl\s+(?P<trait>[A-Za-z0-9_:]+)\s+for\s+(?P<name>[A-Za-z0-9_]+)"
                .to_owned(),
            static_attrs: Vec::new(),
        },
        WorkspaceEdbEntityRule {
            language: Some("rust".to_owned()),
            facet_tag: "derive".to_owned(),
            pattern: r"^#\[derive\([^)]*(?P<name>Debug)[^)]*\)\]".to_owned(),
            static_attrs: Vec::new(),
        },
    ];
    let rust_edge_rules = vec![WorkspaceEdbEdgeRule {
        language: Some("rust".to_owned()),
        relation: "implements".to_owned(),
        pattern: r"^impl\s+(?P<dst>[A-Za-z0-9_:]+)\s+for\s+(?P<src>[A-Za-z0-9_]+)".to_owned(),
    }];

    let rust_edb = index.extract_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["rust".to_owned()],
        required_tokens: &["ToolRuntime".to_owned()],
        entity_rules: &rust_entity_rules,
        edge_rules: &rust_edge_rules,
        ..Default::default()
    });
    assert_eq!(rust_edb.candidate_files, 1);
    assert_eq!(rust_edb.files[0].path, "crates/tooling/src/search.rs");
    assert!(rust_edb.entities.iter().any(|e| {
        e.facet_tag == "trait-impl"
            && e.name == "ResidentSearchHandler"
            && e.attrs
                .iter()
                .any(|a| a.key == "trait" && a.value == "ToolRuntime")
    }));
    assert!(
        rust_edb
            .entities
            .iter()
            .any(|e| e.facet_tag == "derive" && e.name == "Debug")
    );
    assert!(rust_edb.edges.iter().any(|edge| {
        edge.relation == "implements"
            && edge.src == "ResidentSearchHandler"
            && edge.dst == "ToolRuntime"
    }));

    let scheme_entity_rules = vec![
        WorkspaceEdbEntityRule {
            language: Some("scheme".to_owned()),
            facet_tag: "poo-proto".to_owned(),
            pattern: r"^\(\.def\s+(?P<name>[^\s()]+)".to_owned(),
            static_attrs: Vec::new(),
        },
        WorkspaceEdbEntityRule {
            language: Some("scheme".to_owned()),
            facet_tag: "c-define".to_owned(),
            pattern: r#"^\(c-define\s+\((?P<scheme_fn>[^\s()]+)[^"]*"(?P<name>[^"]+)""#.to_owned(),
            static_attrs: Vec::new(),
        },
    ];
    let scheme_edge_rules = vec![
        WorkspaceEdbEdgeRule {
            language: Some("scheme".to_owned()),
            relation: "poo-mix".to_owned(),
            pattern: r"\(\.mix\s+(?P<src>[^\s()]+)\s+(?P<dst>[^\s()]+)".to_owned(),
        },
        WorkspaceEdbEdgeRule {
            language: Some("scheme".to_owned()),
            relation: "ffi-binds".to_owned(),
            pattern: r#"^\(c-define\s+\((?P<src>[^\s()]+)[^"]*"(?P<dst>[^"]+)""#.to_owned(),
        },
    ];

    let scheme_edb = index.extract_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["scheme".to_owned()],
        entity_rules: &scheme_entity_rules,
        edge_rules: &scheme_edge_rules,
        ..Default::default()
    });
    assert_eq!(scheme_edb.candidate_files, 1);
    assert!(
        scheme_edb
            .entities
            .iter()
            .any(|e| e.facet_tag == "poo-proto" && e.name == "RustSearchFragment.")
    );
    assert!(
        scheme_edb
            .entities
            .iter()
            .any(|e| e.facet_tag == "c-define" && e.name == "marlin_search_dsl_compile")
    );
    assert!(scheme_edb.edges.iter().any(|edge| {
        edge.relation == "poo-mix"
            && edge.src == "RustSearchFragment."
            && edge.dst == "BaseSearchFragment."
    }));
    assert!(
        scheme_edb
            .edges
            .iter()
            .any(|edge| edge.relation == "ffi-binds" && edge.dst == "marlin_search_dsl_compile")
    );

    let sysml_rules = vec![WorkspaceEdbEntityRule {
        language: Some("sysml".to_owned()),
        facet_tag: "requirement".to_owned(),
        pattern: r"^requirement\s+<[^>]+>\s+(?P<name>[A-Za-z0-9_]+)".to_owned(),
        static_attrs: Vec::new(),
    }];
    let sysml_edb = index.extract_prefiltered_edb(&WorkspaceEdbPrefilter {
        languages: &["sysml".to_owned()],
        entity_rules: &sysml_rules,
        ..Default::default()
    });
    assert_eq!(sysml_edb.entities.len(), 1);
    assert_eq!(sysml_edb.entities[0].name, "openEdbExtraction");
}
