use crate::cache_replay::load_replay_artifact;
use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheGeneration, ClientCacheManifest, ClientMethod,
    ClientRequest, LanguageId, ProviderId, SemanticSchemaId,
};
use agent_semantic_client_db::{ClientDb, ClientDbGenerationHit};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn semantic_tree_sitter_query_replay_falls_back_to_rows_when_artifact_is_missing() {
    let root = temp_root("syntax-row-replay");
    let cache_root = root.join("client");
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
        String::from_utf8(replay.stdout).expect("utf8"),
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
    let cache_root = root.join("client");
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
    let cache_root = root.join("client");
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
fn prompt_output_replay_rejects_legacy_compact_graph_grammar() {
    let root = temp_root("prompt-output-legacy-graph");
    let cache_root = root.join("client");
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "fzf".to_string(),
        "GraphAlias".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);
    write_prompt_output_artifact(
        &root,
        "[search-fzf] q=GraphAlias alg=seed-frontier\n\
legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next\n\
alias: graph:{G=search,Q=query}\n\
Q=query:term(GraphAlias)!fzf\n\
G>{Q:matches}\n\
rank=Q frontier=Q.fzf\n",
    );
    let hit = prompt_generation_hit(&root, &request, "search/fzf");

    assert!(load_replay_artifact(&cache_root, &hit, &request).is_none());
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
        file_hashes: Vec::new(),
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
        file_hashes: Vec::new(),
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
    ["src/lib.rs", "src/main.rs"]
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(root.join(path)).expect("read syntax replay source");
            let digest = Sha256::digest(&bytes);
            json!({"path": path, "sha256": format!("{digest:x}")})
        })
        .collect()
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
    let prompt_dir = root.join("artifacts/prompt-output");
    std::fs::create_dir_all(&prompt_dir).expect("create prompt artifact dir");
    std::fs::write(prompt_dir.join("stale.txt"), stdout).expect("write prompt artifact");
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
        "{}\0{}\0{}\0{}\0{}",
        "rust",
        "rs-harness",
        project_root,
        export_method,
        request.forwarded_args.join("\0")
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
