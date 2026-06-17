use serde_json::{Value, json};

use crate::provider_command::support::{
    CACHE_SOURCE_PATH, CACHE_SOURCE_SHA256, cache_root, write_cache_source_fixture,
};

pub(super) fn valid_manifest(root: &std::path::Path) -> Value {
    valid_search_manifest_with_artifact(root, "search/rust-main-1.json")
}

pub(super) fn valid_manifest_with_artifact(root: &std::path::Path, artifact_id: &str) -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": cache_root(root).display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "fileHashes": fresh_file_hashes(root),
                "artifactIds": [artifact_id]
            }
        ]
    })
}

pub(super) fn valid_search_manifest_with_artifact(
    root: &std::path::Path,
    artifact_id: &str,
) -> Value {
    let request_fingerprint =
        request_fingerprint(root, "search/prime", &["prime", "--view", "seeds"]);
    json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": cache_root(root).display().to_string(),
        "generations": [
            {
                "generationId": "rust-main-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "search/prime",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-search-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": request_fingerprint,
                "fileHashes": fresh_file_hashes(root),
                "artifactIds": [artifact_id]
            }
        ]
    })
}

pub(super) fn valid_query_manifest_with_artifact(
    root: &std::path::Path,
    artifact_id: &str,
) -> Value {
    let request_fingerprint = request_fingerprint(
        root,
        "query/owner-items",
        &["src/lib.rs", "--term", "CacheReplay"],
    );
    json!({
        "schemaId": "agent.semantic-protocols.client-cache-manifest",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.client",
        "protocolVersion": "1",
        "cacheRoot": cache_root(root).display().to_string(),
        "generations": [
            {
                "generationId": "rust-query-1",
                "languageId": "rust",
                "providerId": "rs-harness",
                "providerVersion": "0.1.0",
                "exportMethod": "query/owner-items",
                "projectRoot": root.display().to_string(),
                "packageRoot": ".",
                "schemaIds": ["agent.semantic-protocols.semantic-query-packet"],
                "cacheStatus": "miss",
                "rawSourceStored": false,
                "requestFingerprint": request_fingerprint,
                "fileHashes": fresh_file_hashes(root),
                "artifactIds": [artifact_id]
            }
        ]
    })
}

fn fresh_file_hashes(root: &std::path::Path) -> Value {
    write_cache_source_fixture(root);
    let source_path = root.join(CACHE_SOURCE_PATH);
    let metadata = std::fs::metadata(&source_path).expect("cache fixture metadata");
    let mtime_ms = metadata
        .modified()
        .expect("cache fixture mtime")
        .duration_since(std::time::UNIX_EPOCH)
        .expect("cache fixture mtime after epoch")
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    json!([
        {
            "path": CACHE_SOURCE_PATH,
            "sha256": CACHE_SOURCE_SHA256,
            "byteLen": metadata.len(),
            "mtimeMs": mtime_ms
        }
    ])
}

fn request_fingerprint(root: &std::path::Path, export_method: &str, args: &[&str]) -> String {
    let prompt_output_provenance = prompt_output_render_abi_provenance(export_method);
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        "rust",
        "rs-harness",
        normalized_path(root),
        export_method,
        args.join("\0"),
        "syntax-query-ast-abi:none",
        prompt_output_provenance
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
    "capabilities=pipe,fzf,fd-query,rg-query,owner-items,selector-code,treesitter-query;",
    "ladder=pipe>fzf>fd-query|rg-query>owner-items>selector-code;",
    "history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath;",
    "risk=broad-direct-read,manual-window-scan,repeat-prime;",
    "next=search pipe <question-or-feature-term> --view seeds"
);

fn normalized_path(path: &std::path::Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn stable_hash_hex(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(super) fn sample_search_packet() -> Value {
    json!({
        "schemaId": "agent.semantic-protocols.semantic-search-packet",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.semantic-language",
        "protocolVersion": "1",
        "languageId": "rust",
        "providerId": "rs-harness",
        "view": "prime",
        "query": "CacheReplay",
        "querySet": ["CacheReplay"],
        "searchSynthesis": {
            "algorithm": "cache-packet-replay",
            "seeds": [
                {
                    "kind": "owner",
                    "target": "src/lib.rs",
                    "targetRole": "path"
                },
                {
                    "kind": "symbol",
                    "target": "CacheReplay",
                    "targetRole": "symbol",
                    "read": "src/lib.rs:1:5"
                },
                {
                    "kind": "tests",
                    "target": "tests/cache_replay.rs",
                    "targetRole": "path"
                }
            ]
        }
    })
}

pub(super) fn sample_query_packet() -> Value {
    serde_json::from_str(
        r#"{
  "schemaId": "agent.semantic-protocols.semantic-query-packet",
  "schemaVersion": "1",
  "protocolId": "agent.semantic-protocols.semantic-language",
  "protocolVersion": "1",
  "languageId": "rust",
  "providerId": "rs-harness",
  "binary": "rs-harness",
  "namespace": "agent.semantic-protocols.languages.rust.rs-harness",
  "method": "query/owner-items",
  "projectRoot": ".",
  "packageName": "cache-demo",
  "ownerPath": "src/lib.rs",
  "query": "CacheReplay",
  "queryTerms": ["CacheReplay"],
  "matchMode": "exact",
  "outputMode": "outline",
  "queryCoverage": [
    {
      "value": "CacheReplay",
      "status": "hit",
      "match": "exact",
      "matchCount": 1,
      "nextAction": "select-item"
    }
  ],
  "matches": [
    {
      "name": "CacheReplay",
      "kind": "struct",
      "visibility": "private",
      "doc": false,
      "location": {"path": "src/lib.rs", "lineRange": "1:3"},
      "read": "src/lib.rs:1:3",
      "code": "struct CacheReplay\nfield stdout: Vec<u8>",
      "projection": {
        "mode": "compact",
        "syntax": "semantic-outline",
        "sourceAuthority": "native-parser",
        "sourceFingerprint": "src/lib.rs:1:3:44",
        "losslessStructure": true,
        "exactRead": "src/lib.rs:1:3",
        "nodes": [
          {
            "id": "query-cache-node",
            "nativeId": "rust:struct:CacheReplay",
            "kind": "struct",
            "role": "declaration",
            "label": "struct CacheReplay",
            "depth": 0,
            "read": "src/lib.rs:1:3",
            "structuralFingerprint": "struct:declaration:CacheReplay"
          }
        ],
        "renderedNodeIds": ["query-cache-node"]
      },
      "truncated": false
    }
  ],
  "matchCount": 1,
  "matchLimit": 1,
  "matchesTruncated": false,
  "truncated": false
}"#,
    )
    .expect("sample query packet must be valid JSON")
}
