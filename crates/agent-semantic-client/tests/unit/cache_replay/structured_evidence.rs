use crate::cache_replay::{load_replay_artifact, structured_evidence_artifact_path};
use crate::test_support::{artifacts_root_from_cache_root, v2_cache_root};
use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheFileHash, ClientMethod, ClientRequest,
    LanguageId, ProviderId, SemanticSchemaId,
};
use agent_semantic_client_db::ClientDbGenerationHit;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn structured_evidence_artifacts_use_schema_owned_json_families() {
    let cache_file = Path::new("/tmp/project/workspaces/workspace/live/client");

    for artifact_id in [
        "relation-plan/owner-flow.json",
        "flow-lite/source-sink.json",
        "codeql-evidence/metadata.json",
    ] {
        let path =
            structured_evidence_artifact_path(cache_file, &CacheArtifactId::from(artifact_id))
                .expect("structured evidence artifact path");
        let expected_suffix = Path::new("artifacts").join(artifact_id);
        assert!(path.ends_with(expected_suffix));
    }
}

#[test]
fn structured_evidence_artifacts_reject_prompt_output_and_unsafe_paths() {
    let cache_file = Path::new("/tmp/project/workspaces/workspace/live/client/client.turso");

    for artifact_id in [
        "prompt-output/relation-plan.txt",
        "relation-plan/not-json.txt",
        "relation-plan/../prompt-output/stale.json",
        "/relation-plan/rooted.json",
    ] {
        assert!(
            structured_evidence_artifact_path(cache_file, &CacheArtifactId::from(artifact_id))
                .is_none(),
            "{artifact_id} should not be accepted as structured evidence"
        );
    }
}

#[test]
fn structured_evidence_artifacts_prevent_prompt_stdout_replay() {
    let root = temp_root("structured-evidence-no-prompt-replay");
    let cache_root = v2_cache_root(&root);
    write_source(&root);
    write_prompt_output_artifact(&root, "safe prompt stdout\n");
    let request = ClientRequest::new(ClientMethod::Search, ".").with_forwarded_args(vec![
        "lexical".to_string(),
        "relation".to_string(),
        "--view".to_string(),
        "seeds".to_string(),
        ".".to_string(),
    ]);

    let prompt_only = generation_hit(
        &root,
        &request,
        vec![CacheArtifactId::from("prompt-output/stale.txt")],
    );
    assert!(
        load_replay_artifact(&cache_root, &prompt_only, &request).is_some(),
        "control generation should replay prompt stdout fallback"
    );

    let structured_evidence = generation_hit(
        &root,
        &request,
        vec![
            CacheArtifactId::from("relation-plan/native-owner-flow.json"),
            CacheArtifactId::from("prompt-output/stale.txt"),
        ],
    );
    assert!(
        load_replay_artifact(&cache_root, &structured_evidence, &request).is_none(),
        "structured evidence artifacts should suppress prompt stdout fallback"
    );

    let _ = std::fs::remove_dir_all(root);
}

fn generation_hit(
    root: &Path,
    request: &ClientRequest,
    artifact_ids: Vec<CacheArtifactId>,
) -> ClientDbGenerationHit {
    ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("search/lexical"),
        schema_ids: vec![
            SemanticSchemaId::from("agent.semantic-protocols.client-prompt-output"),
            SemanticSchemaId::from("agent.semantic-protocols.semantic-relation-plan"),
        ],
        request_fingerprint: Some(prompt_output_request_fingerprint(root, request)),
        file_hashes: vec![client_file_hash(root, "src/lib.rs").expect("source hash")],
        artifact_ids,
    }
}

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-client-{name}-{unique}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_source(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create src dir");
    std::fs::write(root.join("src/lib.rs"), "pub fn relation() {}\n").expect("write source");
}

fn write_prompt_output_artifact(root: &Path, stdout: &str) {
    let prompt_dir = artifacts_root_from_cache_root(&v2_cache_root(root)).join("prompt-output");
    std::fs::create_dir_all(&prompt_dir).expect("create prompt artifact dir");
    std::fs::write(prompt_dir.join("stale.txt"), stdout).expect("write prompt artifact");
}

fn client_file_hash(root: &Path, path: &str) -> Option<ClientCacheFileHash> {
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

fn prompt_output_request_fingerprint(root: &Path, request: &ClientRequest) -> String {
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
        "search/lexical",
        request.forwarded_args.join("\0"),
        "syntax-query-ast-abi:none",
        "prompt-output-render-abi:none"
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
