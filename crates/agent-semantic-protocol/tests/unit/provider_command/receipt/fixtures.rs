use serde_json::{Value, json};

use crate::provider_command::support::{
    CACHE_SOURCE_PATH, CACHE_SOURCE_SHA256, cache_root, write_cache_source_fixture,
};

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
    "capabilities=pipe,lexical,fd-query,rg-query,owner-items,selector-code,treesitter-query;",
    "ladder=pipe>lexical>fd-query|rg-query>owner-items>selector-code;",
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
