use agent_semantic_runtime::LiveCorpusLanguageExtensionEvidenceV1;
use std::path::Path;

use super::{
    LiveCorpusExtensionAdmissionV1, LiveCorpusGitLockV1, LiveCorpusInputsV1, LiveCorpusLockEntryV1,
    load_lock, parse_materialize_request, parse_resource_request, publish_immutable_json,
    sync_usage, validate_extension_admission,
};

fn corpus(admission: Option<LiveCorpusExtensionAdmissionV1>) -> LiveCorpusLockEntryV1 {
    LiveCorpusLockEntryV1 {
        resource_id: "org.worg".to_string(),
        scenario_id: "org.worg-intent-matrix".to_string(),
        provider_id: "orgize".to_string(),
        language: "org".to_string(),
        repository: "bzg/worg".to_string(),
        git: LiveCorpusGitLockV1 {
            remote: "https://git.sr.ht/~bzg/worg".to_string(),
            revision: "1".repeat(40),
        },
        directory: "org-worg".to_string(),
        environment: "SANDTABLE_ORG_WORG_ROOT".to_string(),
        admission,
        inputs: LiveCorpusInputsV1 {
            owner: "index.org".to_string(),
            query: "agenda".to_string(),
            dependency: "Org".to_string(),
        },
    }
}

#[test]
fn extension_admission_rejects_processor_shaped_corpus() {
    let evidence = LiveCorpusLanguageExtensionEvidenceV1 {
        authority: "provider-workspace-scope".to_string(),
        candidate_set_authority: "provider-registry-extension-index".to_string(),
        source_extensions: vec![".org".to_string()],
        matching_file_count: 4,
        candidate_language_file_count: 200,
    };
    let error = validate_extension_admission(
        &corpus(Some(LiveCorpusExtensionAdmissionV1 {
            extension_authority: "provider-workspace-scope".to_string(),
            minimum_matching_files: 100,
            minimum_matching_file_ratio: 0.8,
        })),
        &evidence,
    )
    .expect_err("small document surface must fail");

    assert!(error.contains("language-extension admission failed"));
}

#[test]
fn materialize_args_require_resource_and_source() {
    let error = parse_materialize_request(&["--resource".to_string(), "org.worg".to_string()])
        .expect_err("source is required");
    assert!(error.contains("usage: asp live-corpus materialize"));
}

#[test]
fn sync_args_require_a_locked_resource() {
    let error = parse_resource_request(&[], sync_usage).expect_err("sync resource is required");
    assert!(error.contains("usage: asp live-corpus sync"));
}

#[test]
fn repository_corpus_lock_is_the_materializer_contract() {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/large-library-runtime-corpora.v1.json");
    let lock = load_lock(&lock_path).expect("repository corpus lock must decode");

    assert_eq!(lock.corpora.len(), 17);
    assert!(
        lock.corpora
            .iter()
            .any(|corpus| corpus.resource_id == "org.worg")
    );
}

#[test]
fn immutable_receipt_publication_is_idempotent_and_collision_safe() {
    let root = std::env::temp_dir().join(format!(
        "asp-live-corpus-publication-{}",
        std::process::id()
    ));
    let path = root.join("manifest.json");
    let first = serde_json::json!({"schemaVersion": "1", "digest": "first"});
    let collision = serde_json::json!({"schemaVersion": "1", "digest": "second"});

    publish_immutable_json(&path, &first).expect("first publication");
    publish_immutable_json(&path, &first).expect("idempotent publication");
    let error =
        publish_immutable_json(&path, &collision).expect_err("identity collision must fail");

    assert!(error.contains("immutable live-corpus artifact collision"));
    std::fs::remove_dir_all(root).expect("remove owned test directory");
}
