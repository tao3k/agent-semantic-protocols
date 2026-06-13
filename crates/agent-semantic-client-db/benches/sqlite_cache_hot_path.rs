use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_semantic_client_core::{
    ClientCacheGeneration, ClientCacheManifest, LanguageId, ProviderId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{
    ClientDb, ClientDbStructuralIndexLookup, ClientDbStructuralQueryKey, ClientDbSyntaxQueryLookup,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use serde_json::{Value, json};
use std::hint::black_box;

const SYNTAX_QUERY: &str = "(function_item name: (identifier) @function.name)";
const SYNTAX_ROW_COUNT: usize = 512;
const STRUCTURAL_SYMBOL_COUNT: usize = 512;
const STRUCTURAL_DEPENDENCY_COUNT: usize = 256;

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
    let structural_generation_row = structural_generation(&project_root);
    db.import_manifest(&manifest_from_generation(
        &cache_root,
        structural_generation_row.clone(),
    ))
    .expect("import benchmark structural manifest");
    db.import_semantic_structural_index_packet(
        &structural_generation_row,
        &serde_json::to_vec(&structural_index_packet(
            &project_root,
            STRUCTURAL_SYMBOL_COUNT,
            STRUCTURAL_DEPENDENCY_COUNT,
        ))
        .expect("structural index packet bytes"),
    )
    .expect("import benchmark structural rows");
    let import_cache_root = cache_root.join("import-refresh");
    let import_db_path = ClientDb::default_path(&import_cache_root);
    let import_project_root = import_cache_root.join("project");
    let mut import_db = ClientDb::open_or_create(&import_db_path).expect("open import db");
    let import_generation = structural_generation(&import_project_root);
    let import_packet = serde_json::to_vec(&structural_index_packet(
        &import_project_root,
        STRUCTURAL_SYMBOL_COUNT,
        STRUCTURAL_DEPENDENCY_COUNT,
    ))
    .expect("import structural index packet bytes");
    import_db
        .import_manifest(&manifest_from_generation(
            &import_cache_root,
            import_generation.clone(),
        ))
        .expect("import structural import manifest");
    let read_db = ClientDb::open_read_only_existing(&db_path)
        .expect("open read-only benchmark db")
        .expect("benchmark db exists");
    let syntax_lookup = ClientDbSyntaxQueryLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: project_root.clone(),
        query_ast_fingerprint: syntax_query_ast_abi_fingerprint(SYNTAX_QUERY)
            .expect("syntax query AST fingerprint"),
        selector: Some("src/lib.rs:1:2000".to_string()),
    };
    let structural_symbol_lookup = ClientDbStructuralIndexLookup {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: project_root.clone(),
        query: ClientDbStructuralQueryKey::from("function_511"),
        limit: 8,
    };
    let structural_dependency_lookup = ClientDbStructuralIndexLookup {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root,
        query: ClientDbStructuralQueryKey::from("serde_json::from_str_255"),
        limit: 8,
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
    c.bench_function("sqlite_cache_hot_path/syntax_query_replay_open_512", |b| {
        b.iter(|| {
            let replay = read_db
                .lookup_syntax_query_replay_open(black_box(&syntax_lookup))
                .expect("syntax replay lookup")
                .expect("syntax replay rows");
            black_box(replay.rows.len());
        });
    });
    c.bench_function("sqlite_cache_hot_path/structural_symbol_lookup_512", |b| {
        b.iter(|| {
            let symbols = read_db
                .lookup_structural_symbols(black_box(&structural_symbol_lookup))
                .expect("structural symbol lookup");
            black_box(symbols.len());
        });
    });
    c.bench_function(
        "sqlite_cache_hot_path/structural_dependency_lookup_256",
        |b| {
            b.iter(|| {
                let dependencies = read_db
                    .lookup_structural_dependency_usages(black_box(&structural_dependency_lookup))
                    .expect("structural dependency lookup");
                black_box(dependencies.len());
            });
        },
    );
    let mut import_group = c.benchmark_group("sqlite_cache_hot_path");
    import_group.sample_size(30);
    import_group.warm_up_time(Duration::from_secs(1));
    import_group.measurement_time(Duration::from_secs(5));
    import_group.throughput(Throughput::Elements(
        (STRUCTURAL_SYMBOL_COUNT + STRUCTURAL_DEPENDENCY_COUNT) as u64,
    ));
    import_group.bench_function("structural_import_refresh_512_symbols_256_deps", |b| {
        b.iter(|| {
            let stats = import_db
                .import_semantic_structural_index_packet(
                    black_box(&import_generation),
                    black_box(&import_packet),
                )
                .expect("structural import refresh");
            black_box(stats.symbol_count);
            black_box(stats.dependency_usage_count);
        });
    });
    import_group.finish();
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

fn structural_generation(project_root: &std::path::Path) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": "structural-bench",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": project_root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-structural-index"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": "fnv64:structural-bench",
        "fileHashes": [{"path": "src/lib.rs", "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}],
        "artifactIds": ["structural-index/structural-bench.json"]
    }))
    .expect("structural generation")
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

fn structural_index_packet(
    project_root: &std::path::Path,
    symbol_count: usize,
    dependency_count: usize,
) -> Value {
    let symbols = (0..symbol_count)
        .map(|index| {
            let line = index + 1;
            json!({
                "ownerPath": "src/lib.rs",
                "name": format!("function_{index}"),
                "qualifiedName": format!("crate::function_{index}"),
                "kind": "function",
                "visibility": "public",
                "sourceLocator": format!("src/lib.rs:{line}:{line}"),
                "queryKeys": [format!("function_{index}"), format!("crate::function_{index}")]
            })
        })
        .collect::<Vec<_>>();
    let dependency_usages = (0..dependency_count)
        .map(|index| {
            let line = index + 1;
            json!({
                "ownerPath": "src/lib.rs",
                "packageName": "serde_json",
                "packageVersion": "1.0.0",
                "apiName": format!("from_str_{index}"),
                "importPath": format!("serde_json::from_str_{index}"),
                "manifestPath": "Cargo.toml",
                "lockfileHash": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "source": "manifest+native-parser",
                "sourceLocator": format!("src/lib.rs:{line}:{line}"),
                "queryKeys": [format!("serde_json::from_str_{index}")]
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schemaId": "agent.semantic-protocols.semantic-structural-index",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "generationId": "structural-bench",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "index/structural",
        "projectRoot": project_root.display().to_string(),
        "packageRoot": ".",
        "sourceAuthority": "native-parser",
        "sourceArtifactId": "structural-index/structural-bench.json",
        "rawSourceStored": false,
        "fileHashes": [
            {
                "path": "src/lib.rs",
                "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
                "source": "provider"
            },
            {
                "path": "Cargo.toml",
                "sha256": "1111111111111111111111111111111111111111111111111111111111111111",
                "source": "manifest"
            }
        ],
        "owners": [
            {
                "ownerPath": "src/lib.rs",
                "ownerKind": "source",
                "sourceAuthority": "native-parser",
                "location": {"path": "src/lib.rs", "lineRange": "1:2000"},
                "queryKeys": ["src/lib.rs", "lib"]
            }
        ],
        "symbols": symbols,
        "dependencyUsages": dependency_usages
    })
}

criterion_group!(benches, sqlite_cache_hot_path);
criterion_main!(benches);
