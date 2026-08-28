use agent_semantic_runtime::{
    LiveCorpusGitCheckoutQualification, LiveCorpusLanguageExtensionEvidence,
};
use std::path::Path;

use super::{
    LiveCorpusExtensionAdmission, LiveCorpusGitLock, LiveCorpusInputs, LiveCorpusLockEntry,
    load_lock, materialized_source_identity, parse_materialize_request, parse_resource_request,
    publish_immutable_json, sync_usage, validate_extension_admission,
};

#[test]
fn materialized_source_identity_uses_the_runtime_content_root() {
    let checkout = LiveCorpusGitCheckoutQualification {
        canonical_remote_identity: "https://example.invalid/corpus".to_owned(),
        head_revision: "revision-1".to_owned(),
        git_tree: "tree-1".to_owned(),
        checkout_identity_digest: "blake3-256:checkout-identity".to_owned(),
    };

    let identity = materialized_source_identity(
        checkout,
        "blake3-256:runtime-canonical-content-root".to_owned(),
    )
    .expect("Runtime canonical root must define the materialized source identity");

    assert_eq!(identity.head_revision, "revision-1");
    assert_eq!(identity.git_tree, "tree-1");
    assert_eq!(
        identity.source_merkle_root,
        "blake3-256:runtime-canonical-content-root"
    );
}

#[test]
fn materialized_source_identity_rejects_an_empty_runtime_root() {
    let checkout = LiveCorpusGitCheckoutQualification {
        canonical_remote_identity: "https://example.invalid/corpus".to_owned(),
        head_revision: "revision-1".to_owned(),
        git_tree: "tree-1".to_owned(),
        checkout_identity_digest: "blake3-256:checkout-identity".to_owned(),
    };

    let error = materialized_source_identity(checkout, "  ".to_owned())
        .expect_err("empty Runtime canonical roots must fail closed");
    assert!(error.contains("empty canonical source root digest"));
}

fn corpus(admission: Option<LiveCorpusExtensionAdmission>) -> LiveCorpusLockEntry {
    LiveCorpusLockEntry {
        resource_id: "org.worg".to_string(),
        scenario_id: "org.worg-intent-matrix".to_string(),
        provider_id: "orgize".to_string(),
        language: "org".to_string(),
        repository: "bzg/worg".to_string(),
        git: LiveCorpusGitLock {
            remote: "https://git.sr.ht/~bzg/worg".to_string(),
            revision: "1".repeat(40),
        },
        directory: "org-worg".to_string(),
        environment: "SANDTABLE_ORG_WORG_ROOT".to_string(),
        admission,
        inputs: LiveCorpusInputs {
            owner: "index.org".to_string(),
            query: "agenda".to_string(),
            dependency: "Org".to_string(),
        },
    }
}

#[test]
fn extension_admission_rejects_processor_shaped_corpus() {
    let evidence = LiveCorpusLanguageExtensionEvidence {
        authority: "provider-project-resolution".to_string(),
        candidate_set_authority: "provider-registry-extension-index".to_string(),
        source_extensions: vec![".org".to_string()],
        matching_file_count: 4,
        candidate_language_file_count: 200,
    };
    let error = validate_extension_admission(
        &corpus(Some(LiveCorpusExtensionAdmission {
            extension_authority: "provider-project-resolution".to_string(),
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

#[tokio::test]
async fn qualify_args_require_a_plan_path_after_the_flag() {
    let error = crate::command::live_corpus::run_live_corpus_command(&[
        "qualify".to_owned(),
        "--plan".to_owned(),
    ])
    .await
    .expect_err("qualification plan path is required");
    assert!(error.contains("requires a path after --plan"));
}

#[tokio::test]
async fn removed_prepare_command_is_not_a_public_surface() {
    let error = crate::command::live_corpus::run_live_corpus_command(&["prepare".to_owned()])
        .await
        .expect_err("removed preparation command must fail closed");
    assert!(error.contains("Usage: asp live-corpus"), "{error}");
    assert!(!error.contains("live-corpus prepare"), "{error}");
    assert!(!error.contains("live-corpus prepare"));
}

#[test]
fn live_corpus_help_exposes_only_current_typed_subcommands() {
    let command = crate::command::cli_help::selected_command(&["live-corpus".to_owned()]);
    let subcommands = command
        .get_subcommands()
        .map(clap::Command::get_name)
        .collect::<Vec<_>>();

    assert!(subcommands.contains(&"qualify"), "{subcommands:?}");
    assert!(!subcommands.contains(&"prepare"), "{subcommands:?}");
}

#[tokio::test]
async fn qualify_args_reject_unknown_options_before_runtime_access() {
    let error = crate::command::live_corpus::run_live_corpus_command(&[
        "qualify".to_owned(),
        "--unknown".to_owned(),
    ])
    .await
    .expect_err("unknown qualification options must be rejected");
    assert_eq!(error, "unknown live-corpus qualify option: --unknown");
}

#[test]
fn repository_corpus_lock_is_the_materializer_contract() {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/large-library-runtime-corpora.json");
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
