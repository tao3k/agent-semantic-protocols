use std::time::{SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{ClientDb, ClientDbSyntaxQueryLookup};
use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::{Value, json};
use std::hint::black_box;

const SYNTAX_QUERY: &str = "(function_item name: (identifier) @function.name)";
const SYNTAX_ROW_COUNT: usize = 512;

fn sqlite_cache_hot_path(c: &mut Criterion) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let cache_root = std::env::temp_dir().join(format!("asp-client-db-bench-{unique}"));
    let db_path = ClientDb::default_path(&cache_root);
    let project_root = cache_root.join("project");
    let mut db = ClientDb::open_or_create(&db_path).expect("open benchmark db");
    let generation = syntax_generation(&project_root);
    db.import_manifest(&manifest_from_generation(&cache_root, generation.clone()))
        .expect("import benchmark syntax manifest");
    db.import_semantic_tree_sitter_query_packet(
        &generation,
        &serde_json::to_vec(&syntax_packet(SYNTAX_ROW_COUNT)).expect("syntax packet bytes"),
    )
    .expect("import benchmark syntax rows");
    let read_db = ClientDb::open_read_only_existing(&db_path)
        .expect("open read-only benchmark db")
        .expect("benchmark db exists");
    let syntax_lookup = ClientDbSyntaxQueryLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root,
        query_ast_fingerprint: syntax_query_ast_abi_fingerprint(SYNTAX_QUERY)
            .expect("syntax query AST fingerprint"),
        selector: Some("src/lib.rs:1:2000".to_string()),
    };
    c.bench_function("sqlite_cache_hot_path/path_inspect", |b| {
        b.iter(|| {
            let report = ClientDb::inspect(black_box(&db_path));
            black_box(report.generation_count);
            black_box(&mut db);
        });
    });
    c.bench_function("sqlite_cache_hot_path/open_report", |b| {
        b.iter(|| {
            let report = read_db.inspect_open().expect("inspect open db");
            black_box(report.generation_count);
            black_box(&read_db);
        });
    });
    c.bench_function("sqlite_cache_hot_path/syntax_query_replay_512", |b| {
        b.iter(|| {
            let replay = ClientDb::lookup_syntax_query_replay(black_box(&syntax_lookup))
                .expect("syntax replay lookup")
                .expect("syntax replay rows");
            black_box(replay.rows.len());
        });
    });
    let _ = std::fs::remove_dir_all(cache_root);
}

fn syntax_generation(project_root: &std::path::Path) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": "syntax-bench",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": project_root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": "fnv64:syntax-bench",
        "fileHashes": [],
        "artifactIds": ["semantic-tree-sitter-query/syntax-bench.json"]
    }))
    .expect("syntax generation")
}

fn manifest_from_generation(
    cache_root: &std::path::Path,
    generation: ClientCacheGeneration,
) -> ClientCacheManifest {
    ClientCacheManifest {
        schema_id: "agent.semantic-protocols.client-cache-manifest".into(),
        schema_version: "1".into(),
        protocol_id: "agent.semantic-protocols.client".into(),
        protocol_version: "1".into(),
        cache_root: cache_root.display().to_string().into(),
        generations: vec![generation],
    }
}

fn syntax_packet(row_count: usize) -> Value {
    let matches = (0..row_count)
        .map(|index| {
            let line = index + 1;
            let symbol = format!("function_{index}");
            json!({
                "id": format!("m{index}"),
                "range": {"path": "src/lib.rs", "lineRange": {"start": line, "end": line}},
                "fields": {"nodeType": "function_item"},
                "captures": [
                    {
                        "id": format!("c{index}"),
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": {"start": line, "end": line}},
                        "fields": {"symbol": symbol},
                        "field": "name"
                    }
                ]
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": SYNTAX_QUERY,
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "compiledSource": SYNTAX_QUERY,
            "fields": {
                "selector": "src/lib.rs:1:2000",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": matches,
        "truncated": false,
        "cache": {
            "artifactKind": "semantic-tree-sitter-query",
            "rawSourceStored": false
        }
    })
}

criterion_group!(benches, sqlite_cache_hot_path);
criterion_main!(benches);
