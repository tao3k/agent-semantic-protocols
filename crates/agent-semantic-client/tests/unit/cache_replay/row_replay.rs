use crate::cache_replay::load_replay_artifact;
use crate::test_support::{artifacts_root_from_cache_root, v2_cache_root};
use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheFileHash, ClientCacheGeneration,
    ClientCacheManifest, ClientMethod, ClientRequest, LanguageId, ProviderId, SemanticSchemaId,
    syntax_query_ast_abi_fingerprint,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationHit, ClientDbSyntaxQueryLookup};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn semantic_tree_sitter_query_replay_falls_back_to_rows_when_artifact_is_missing() {
    let root = temp_root("syntax-row-replay");
    let cache_root = v2_cache_root(&root);
    let db_path = ClientDb::default_path(&cache_root);
    write_syntax_replay_sources(&root);
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&cache_root, generation.clone());
    let packet = syntax_packet_with_matches();
    let packet_bytes = serde_json::to_vec(&packet).expect("packet bytes");
    let packet_len = packet_bytes.len();
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    let direct_replay = ClientDb::lookup_syntax_query_replay(&ClientDbSyntaxQueryLookup {
        db_path: db_path.clone(),
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.clone(),
        query_ast_fingerprint: syntax_query_ast_abi_fingerprint(
            "(function_item\n  name: (identifier) @function.name)",
        )
        .expect("query AST fingerprint"),
        selector: Some("src/lib.rs:1:80".to_string()),
    })
    .expect("direct row lookup");
    assert!(direct_replay.is_some(), "syntax query rows should replay");

    let replay = load_replay_artifact(
        &cache_root,
        &syntax_generation_hit(&root),
        &syntax_request(
            "(function_item\n  name: (identifier) @function.name)",
            "src/lib.rs:1:80",
            false,
        ),
    )
    .expect("row replay");

    assert_eq!(
        std::str::from_utf8(replay.stdout.as_ref()).expect("utf8"),
        expected_stdout()
    );
    assert_eq!(
        replay.syntax_artifact_id.as_ref().map(|id| id.as_str()),
        Some("semantic-tree-sitter-query/missing.json")
    );
    assert_eq!(
        replay.packet_bytes.map(|bytes| bytes.as_u64()),
        Some(packet_len.min(u64::MAX as usize) as u64)
    );
    assert_eq!(replay.sqlite_read_count, 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_tree_sitter_query_row_replay_rejects_stale_source_hashes() {
    let root = temp_root("syntax-row-replay-stale");
    let cache_root = v2_cache_root(&root);
    let db_path = ClientDb::default_path(&cache_root);
    write_syntax_replay_sources(&root);
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&cache_root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet_with_matches()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    std::fs::write(root.join("src/lib.rs"), "pub fn changed() {}\n").expect("mutate source");

    assert!(
        load_replay_artifact(
            &cache_root,
            &syntax_generation_hit(&root),
            &syntax_request(
                "(function_item name: (identifier) @function.name)",
                "src/lib.rs:1:80",
                false,
            ),
        )
        .is_none()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn semantic_tree_sitter_query_row_replay_does_not_fallback_to_prompt_stdout() {
    let root = temp_root("syntax-row-replay-no-prompt-fallback");
    let cache_root = v2_cache_root(&root);
    let db_path = ClientDb::default_path(&cache_root);
    write_syntax_replay_sources(&root);
    let generation = syntax_generation(&root);
    let manifest = manifest_from_generation(&cache_root, generation.clone());
    let packet_bytes = serde_json::to_vec(&syntax_packet_with_matches()).expect("packet bytes");
    let mut db = ClientDb::open_or_create(&db_path).expect("open db");

    db.import_manifest(&manifest).expect("import manifest");
    db.import_semantic_tree_sitter_query_packet(&generation, &packet_bytes)
        .expect("import syntax rows");
    std::fs::write(root.join("src/lib.rs"), "pub fn changed() {}\n").expect("mutate source");

    let request = syntax_request(
        "(function_item name: (identifier) @function.name)",
        "src/lib.rs:1:80",
        false,
    );
    write_prompt_output_artifact(&root, "stale prompt stdout\n");
    let mut hit = syntax_generation_hit(&root);
    hit.request_fingerprint = Some(prompt_output_request_fingerprint(
        &root,
        &request,
        "query/tree-sitter",
    ));
    hit.artifact_ids
        .push(CacheArtifactId::from("prompt-output/stale.txt"));

    assert!(load_replay_artifact(&cache_root, &hit, &request).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn prompt_output_replay_rejects_obsolete_compact_graph_grammar() {
    let root = temp_root("prompt-output-obsolete-graph");
    let cache_root = v2_cache_root(&root);
    write_syntax_replay_sources(&root);
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "GraphAlias".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);
    write_prompt_output_artifact(
        &root,
        "[search-obsolete] q=GraphAlias alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
alias: graph:{G=search,Q=query}\n\
Q=query:term(GraphAlias)!legacy\n\
G>{Q:matches}\n\
rank=Q frontier=Q.legacy\n",
    );
    let hit = prompt_generation_hit(&root, &request, "search/lexical");

    assert!(load_replay_artifact(&cache_root, &hit, &request).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn prompt_output_replay_rejects_stale_generation_file_hashes() {
    let root = temp_root("prompt-output-stale-generation-hash");
    let cache_root = v2_cache_root(&root);
    write_syntax_replay_sources(&root);
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "parse_query".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);
    write_prompt_output_artifact(&root, "owner:src/lib.rs read=src/lib.rs:1:3\n");
    let hit = prompt_generation_hit(&root, &request, "search/lexical");

    std::fs::write(root.join("src/lib.rs"), "pub fn changed() {}\n").expect("mutate source");

    assert!(load_replay_artifact(&cache_root, &hit, &request).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hook_direct_source_read_prompt_output_artifact_does_not_replay() {
    let root = temp_root("hook-direct-source-read-prompt-replay");
    let cache_root = v2_cache_root(&root);
    write_syntax_replay_sources(&root);
    let request = ClientRequest::new(ClientMethod::Query, &root).with_forwarded_args(vec![
        "--from-hook".to_string(),
        "direct-source-read".to_string(),
        "--selector".to_string(),
        "src/lib.rs:1:3".to_string(),
        "--code".to_string(),
        ".".to_string(),
    ]);
    write_prompt_output_artifact(&root, "pub fn parse_query() -> usize {\n    1\n}\n");
    let hit = prompt_generation_hit(&root, &request, "query/direct-source-read");

    assert!(load_replay_artifact(&cache_root, &hit, &request).is_none());
    let selector_request =
        ClientRequest::new(ClientMethod::Query, &root).with_forwarded_args(vec![
            "--selector".to_string(),
            "src/lib.rs:1:3".to_string(),
            "--code".to_string(),
            ".".to_string(),
        ]);
    let selector_hit = prompt_generation_hit(&root, &selector_request, "query/code");

    assert!(load_replay_artifact(&cache_root, &selector_hit, &selector_request).is_none());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn search_packet_replay_prefers_cached_search_stdout_artifact() {
    let root = temp_root("search-output-replay");
    let cache_root = v2_cache_root(&root);
    write_syntax_replay_sources(&root);
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "cache".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);
    let stdout = "[graph-frontier] profile=owner-query alg=typed-ppr-diverse seed=Q budget=10\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
aliases=G:graph,Q:query\n\
Q=query:term(cache)!lexical\n\
G>{Q:matches}\n\
rank=Q frontier=Q.lexical\n";
    write_search_output_artifact(&root, stdout);
    assert!(crate::cache_replay::search_output_artifact_replay_safe(
        stdout.as_bytes()
    ));
    assert!(
        crate::cache_replay::replay_artifact_path(
            &cache_root,
            &CacheArtifactId::from("search-output/cached.txt"),
            "search-output/",
            ".txt",
        )
        .expect("search output artifact path")
        .is_file()
    );

    let hit = ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("search/lexical"),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-search-packet",
        )],
        request_fingerprint: None,
        file_hashes: syntax_replay_client_file_hashes(&root),
        artifact_ids: vec![
            CacheArtifactId::from("search/missing.json"),
            CacheArtifactId::from("search-output/cached.txt"),
        ],
    };

    let replay = load_replay_artifact(&cache_root, &hit, &request).expect("search stdout replay");

    assert_eq!(
        std::str::from_utf8(replay.stdout.as_ref()).expect("utf8"),
        stdout
    );
    assert_eq!(replay.sqlite_read_count, 0);
    let _ = std::fs::remove_dir_all(root);
}

fn syntax_request(source: &str, selector: &str, code: bool) -> ClientRequest {
    let mut args = vec![
        "--treesitter-query".to_string(),
        source.to_string(),
        "--selector".to_string(),
        selector.to_string(),
        ".".to_string(),
    ];
    if code {
        args.push("--code".to_string());
    }
    ClientRequest::new(ClientMethod::Query, ".").with_forwarded_args(args)
}

fn syntax_generation_hit(root: &std::path::Path) -> ClientDbGenerationHit {
    ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("query/tree-sitter"),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-tree-sitter-query",
        )],
        request_fingerprint: Some("fnv64:syntax-row".to_string()),
        file_hashes: syntax_replay_client_file_hashes(root),
        artifact_ids: vec![CacheArtifactId::from(
            "semantic-tree-sitter-query/missing.json",
        )],
    }
}

fn prompt_generation_hit(
    root: &std::path::Path,
    request: &ClientRequest,
    export_method: &str,
) -> ClientDbGenerationHit {
    ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from(export_method),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.client-prompt-output",
        )],
        request_fingerprint: Some(prompt_output_request_fingerprint(
            root,
            request,
            export_method,
        )),
        file_hashes: syntax_replay_client_file_hashes(root),
        artifact_ids: vec![CacheArtifactId::from("prompt-output/stale.txt")],
    }
}

fn syntax_generation(root: &std::path::Path) -> ClientCacheGeneration {
    serde_json::from_value(json!({
        "generationId": "syntax-row",
        "languageId": "rust",
        "providerId": "rs-harness",
        "providerVersion": "0.1.0",
        "exportMethod": "query/tree-sitter",
        "projectRoot": root.display().to_string(),
        "packageRoot": ".",
        "schemaIds": ["agent.semantic-protocols.semantic-tree-sitter-query"],
        "cacheStatus": "hit",
        "rawSourceStored": false,
        "requestFingerprint": "fnv64:syntax-row",
        "fileHashes": syntax_replay_file_hashes(root),
        "artifactIds": ["semantic-tree-sitter-query/missing.json"]
    }))
    .expect("syntax generation")
}

fn write_syntax_replay_sources(root: &std::path::Path) {
    std::fs::create_dir_all(root.join("src")).expect("create syntax replay src dir");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn parse_query() -> usize {\n    1\n}\n",
    )
    .expect("write src/lib.rs");
    std::fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write src/main.rs");
}

fn syntax_replay_file_hashes(root: &std::path::Path) -> Vec<Value> {
    syntax_replay_client_file_hashes(root)
        .into_iter()
        .map(|file_hash| {
            json!({
                "path": file_hash.path,
                "sha256": file_hash.sha256,
                "byteLen": file_hash.byte_len,
                "mtimeMs": file_hash.mtime_ms
            })
        })
        .collect()
}

fn syntax_replay_client_file_hashes(root: &std::path::Path) -> Vec<ClientCacheFileHash> {
    ["src/lib.rs", "src/main.rs"]
        .into_iter()
        .filter_map(|path| client_file_hash(root, path))
        .collect()
}

fn client_file_hash(root: &std::path::Path, path: &str) -> Option<ClientCacheFileHash> {
    let source_path = root.join(path);
    let metadata = std::fs::metadata(&source_path).ok()?;
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)?;
    let bytes = std::fs::read(source_path).ok()?;
    let digest = Sha256::digest(&bytes);
    Some(ClientCacheFileHash {
        path: path.to_string(),
        sha256: format!("{digest:x}"),
        byte_len: metadata.len(),
        mtime_ms,
    })
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

fn syntax_packet_with_matches() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-tree-sitter-query",
        "method": "query",
        "languageId": "rust",
        "providerId": "rs-harness",
        "grammarId": "tree-sitter-rust",
        "grammarProfileVersion": "2026-06-04.v1",
        "query": {
            "input": "(function_item name: (identifier) @function.name)",
            "inputForm": "s-expression",
            "dialect": "tree-sitter-query",
            "compiledSource": "(function_item name: (identifier) @function.name)",
            "fields": {
                "selector": "src/lib.rs:1:80",
                "codeOutput": false,
                "captures": ["function.name"]
            }
        },
        "matches": [
            {
                "id": "m1",
                "range": {"path": "src/lib.rs", "lineRange": "10:12"},
                "captures": [
                    {
                        "id": "c1",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/lib.rs", "lineRange": "10:10"},
                        "fields": {"symbol": "parse_query"}
                    }
                ]
            },
            {
                "id": "m2",
                "captures": [
                    {
                        "id": "c2",
                        "name": "function.name",
                        "nodeType": "identifier",
                        "range": {"path": "src/main.rs", "lineRange": {"start": 20, "end": 20}},
                        "fields": {"name": "main"}
                    }
                ]
            }
        ],
        "truncated": false,
        "cache": {
            "artifactKind": "semantic-tree-sitter-query",
            "rawSourceStored": false
        }
    })
}

fn expected_stdout() -> &'static str {
    "[query-treesitter] root=. lang=rust pattern=function_item/name capture=function.name alg=syntax-capture-frontier\n\
legend: aliases ID:kind; node ID=kind:role(value)!next; ts=node/field; frontier ID.next\n\
aliases=G:query,Q:tsquery,C:capture,I:item,O:owner\n\n\
Q=tsquery:pattern(function_item/name)!query\n\
C=capture:function.name(parse_query)@src/lib.rs:10!code ts=identifier/name\n\
I=item:fn(parse_query)@src/lib.rs:10:12!code ts=function_item\n\
C2=capture:function.name(main)@src/main.rs:20!code ts=identifier/name\n\
I2=item:fn(main)@src/main.rs:20!code ts=function_item\n\n\
G>{Q:selects}\n\
Q>{C:captures,C2:captures}\n\
C>{I:enclosing-item}\n\
C2>{I2:enclosing-item}\n\n\
omit=code,full-node-list,capture-text\n\
rank=I,I2\n\
frontier=I.code,I2.code\n\
avoid=broad-code-output,raw-read\n"
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_prompt_output_artifact(root: &std::path::Path, stdout: &str) {
    let prompt_dir = artifacts_root_from_cache_root(&v2_cache_root(root)).join("prompt-output");
    std::fs::create_dir_all(&prompt_dir).expect("create prompt artifact dir");
    std::fs::write(prompt_dir.join("stale.txt"), stdout).expect("write prompt artifact");
}

fn write_search_output_artifact(root: &std::path::Path, stdout: &str) {
    let search_output_dir =
        artifacts_root_from_cache_root(&v2_cache_root(root)).join("search-output");
    std::fs::create_dir_all(&search_output_dir).expect("create search output artifact dir");
    std::fs::write(search_output_dir.join("cached.txt"), stdout)
        .expect("write search output artifact");
}

fn prompt_output_request_fingerprint(
    root: &std::path::Path,
    request: &ClientRequest,
    export_method: &str,
) -> String {
    let project_root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string();
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        "rust",
        "rs-harness",
        project_root,
        export_method,
        request.forwarded_args.join("\0"),
        "syntax-query-ast-abi:none",
        prompt_output_render_abi_provenance(export_method)
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

fn prompt_output_render_abi_provenance(export_method: &str) -> String {
    if matches!(export_method, "search/prime" | "search/package") {
        return format!(
            "prompt-output-render-abi:fnv64:{}",
            stable_hash_hex(PRIME_DECISION_PRIMER_RENDER_ABI)
        );
    }
    "prompt-output-render-abi:none".to_string()
}

const PRIME_DECISION_PRIMER_RENDER_ABI: &str = concat!(
    "semantic-search-prime;",
    "purpose=decision-primer;",
    "answer=false;",
    "code=false;",
    "capabilities=pipe,lexical,fd-query,rg-query,owner-items,selector-code,treesitter-query;",
    "ladder=pipe>lexical>fd-query|rg-query>owner-items>selector-code;",
    "history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath;",
    "risk=broad-direct-read,manual-window-scan,repeat-prime;",
    "next=search pipe <question-or-feature-term> --view seeds"
);

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
