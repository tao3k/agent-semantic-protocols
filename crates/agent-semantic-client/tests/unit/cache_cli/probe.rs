use crate::cache_cli::generation_file_hashes_match;
use agent_semantic_client_core::{
    CacheArtifactId, CacheExportMethod, ClientCacheFileHash, LanguageId, ProviderId,
    SemanticSchemaId,
};
use agent_semantic_client_db::ClientDbGenerationHit;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn generation_file_hashes_detect_changed_source() {
    let root = temp_root("changed-source");
    let source_path = root.join("src/lib.rs");
    std::fs::create_dir_all(source_path.parent().expect("source parent")).expect("mkdir");
    std::fs::write(&source_path, b"fn cached() {}\n").expect("write source");
    let hit = generation_hit(&root, vec![file_hash("src/lib.rs", b"fn cached() {}\n")]);

    assert!(generation_file_hashes_match(&root, &hit));

    std::fs::write(&source_path, b"fn changed() {}\n").expect("rewrite source");

    assert!(!generation_file_hashes_match(&root, &hit));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn generation_without_file_hash_evidence_is_stale() {
    let root = temp_root("missing-evidence");
    let hit = generation_hit(&root, Vec::new());

    assert!(!generation_file_hashes_match(&root, &hit));
    let _ = std::fs::remove_dir_all(root);
}

fn generation_hit(
    root: &std::path::Path,
    file_hashes: Vec<ClientCacheFileHash>,
) -> ClientDbGenerationHit {
    ClientDbGenerationHit {
        language_id: LanguageId::from("rust"),
        provider_id: ProviderId::from("rs-harness"),
        project_root: root.to_path_buf(),
        export_method: CacheExportMethod::from("query/tree-sitter"),
        schema_ids: vec![SemanticSchemaId::from(
            "agent.semantic-protocols.semantic-tree-sitter-query",
        )],
        request_fingerprint: Some("fnv64:0123456789abcdef".to_string()),
        file_hashes,
        artifact_ids: vec![CacheArtifactId::from(
            "semantic-tree-sitter-query/rust-query.json",
        )],
    }
}

fn file_hash(path: &str, bytes: &[u8]) -> ClientCacheFileHash {
    let digest = Sha256::digest(bytes);
    ClientCacheFileHash {
        path: path.to_string(),
        sha256: format!("{digest:x}"),
    }
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("agent-client-probe-{label}-{nanos}"))
}
