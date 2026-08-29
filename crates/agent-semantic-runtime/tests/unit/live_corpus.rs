use std::{fs, path::Path};

use super::{
    LIVE_CORPUS_ARTIFACT_SCHEMA_ID, LiveCorpusArtifactIdentity,
    language_extension_evidence_from_paths, live_corpus_artifact_manifest,
    live_corpus_artifact_paths, live_corpus_git_repository_paths, live_corpus_lock_digest,
    normalize_extension_set, qualify_reusable_checkout, sync_live_corpus_git_checkout,
};

fn artifact_identity<'a>(revision: &'a str) -> LiveCorpusArtifactIdentity<'a> {
    LiveCorpusArtifactIdentity {
        lock_digest: "0000000000000000000000000000000000000000000000000000000000000000",
        resource_id: "rust.tokio",
        provider_id: "asp-rust",
        language_id: "rust",
        builder_id: "asp-live-corpus",
        revision,
        git_tree: "1111111111111111111111111111111111111111",
        source_merkle_root: "2222222222222222222222222222222222222222222222222222222222222222",
    }
}

#[test]
fn remote_identity_is_gix_normalized_and_blake3_addressed() {
    let state_home = Path::new("/state");
    let https =
        live_corpus_git_repository_paths(state_home, "https://github.com/tokio-rs/tokio.git")
            .expect("HTTPS repository identity");
    let ssh = live_corpus_git_repository_paths(state_home, "git@github.com:tokio-rs/tokio.git")
        .expect("SSH repository identity");

    assert_eq!(https.canonical_remote_identity, "github.com/tokio-rs/tokio");
    assert_eq!(https.remote_digest, ssh.remote_digest);
    assert_eq!(https.repository_dir, ssh.repository_dir);
    assert_eq!(
        https.repository_dir,
        state_home
            .join("git/repo/blake3-256")
            .join(&https.remote_digest)
    );
    assert_eq!(
        https.ghq_alias_path,
        state_home.join("git/by-remote/github.com/tokio-rs/tokio")
    );
}

#[test]
fn gix_sync_feature_contract_can_create_sha1_checkout_repository() {
    let checkout = std::env::temp_dir().join(format!(
        "asp-live-corpus-gix-prepare-{}",
        std::process::id()
    ));
    let preparation = gix::prepare_clone("https://example.invalid/live-corpus.git", &checkout)
        .expect("SHA-1 clone preparation must not panic");

    drop(preparation);
    assert!(!checkout.exists(), "abandoned preparation must clean up");
}

#[test]
#[ignore = "networked developer live-corpus gate"]
fn developer_live_worg_sync_is_pinned_and_reusable() {
    let lock_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/large-library-runtime-corpora.json");
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(&lock_path).expect("read shared live-corpus lock"))
            .expect("decode shared live-corpus lock");
    let corpus = lock["corpora"]
        .as_array()
        .and_then(|corpora| {
            corpora
                .iter()
                .find(|corpus| corpus["resourceId"] == "org.worg")
        })
        .expect("org.worg must be locked");
    let remote = corpus["git"]["remote"].as_str().expect("locked remote");
    let revision = corpus["git"]["revision"].as_str().expect("locked revision");
    let state_home = std::env::temp_dir().join(format!(
        "asp-live-corpus-network-gate-{}",
        std::process::id()
    ));

    let first =
        sync_live_corpus_git_checkout(&state_home, remote, revision).expect("first gix sync");
    assert!(!first.reused);
    let second =
        sync_live_corpus_git_checkout(&state_home, remote, revision).expect("warm gix sync");
    assert!(second.reused);
    qualify_reusable_checkout(&second.checkout_dir, remote, revision)
        .expect("published checkout qualification");
    fs::remove_dir_all(state_home).expect("remove owned network gate state");
}

#[test]
fn lock_digest_changes_when_the_git_lock_changes() {
    assert_ne!(
        live_corpus_lock_digest(br#"{"revision":"111"}"#),
        live_corpus_lock_digest(br#"{"revision":"222"}"#)
    );
}

#[test]
fn same_display_name_on_another_remote_cannot_collide() {
    let state_home = Path::new("/state");
    let upstream =
        live_corpus_git_repository_paths(state_home, "https://github.com/tokio-rs/tokio")
            .expect("upstream identity");
    let fork = live_corpus_git_repository_paths(state_home, "https://gitlab.com/tokio-rs/tokio")
        .expect("fork identity");

    assert_ne!(upstream.remote_digest, fork.remote_digest);
    assert_ne!(upstream.repository_dir, fork.repository_dir);
}

#[test]
fn commit_changes_artifact_but_not_repository_object_store() {
    let state_home = Path::new("/state");
    let repository =
        live_corpus_git_repository_paths(state_home, "https://github.com/tokio-rs/tokio")
            .expect("repository identity");
    let first = live_corpus_artifact_paths(
        state_home,
        &repository,
        &artifact_identity("3333333333333333333333333333333333333333"),
    )
    .expect("first artifact");
    let second = live_corpus_artifact_paths(
        state_home,
        &repository,
        &artifact_identity("4444444444444444444444444444444444444444"),
    )
    .expect("second artifact");

    assert_ne!(first.artifact_digest, second.artifact_digest);
    assert_ne!(first.artifact_dir, second.artifact_dir);
    assert_eq!(first.current_pointer, second.current_pointer);
    assert_eq!(
        first.source_checkout_dir,
        repository
            .repository_dir
            .join("checkouts/3333333333333333333333333333333333333333")
    );

    let manifest = live_corpus_artifact_manifest(
        "https://github.com/tokio-rs/tokio",
        &repository,
        &artifact_identity("3333333333333333333333333333333333333333"),
        &first,
    )
    .expect("typed artifact manifest");
    assert_eq!(manifest.schema_id, LIVE_CORPUS_ARTIFACT_SCHEMA_ID);
    assert_eq!(manifest.artifact_digest, first.artifact_digest);
    assert_eq!(manifest.git.remote_digest, repository.remote_digest);
}

#[test]
fn document_extension_ratio_excludes_non_language_attachments() {
    let evidence = language_extension_evidence_from_paths(
        [
            "index.org",
            "guide/agenda.org",
            "guide/export.org",
            "scripts/check.js",
            "images/logo.png",
            "manual.pdf",
        ]
        .into_iter()
        .map(str::to_string),
        normalize_extension_set("sourceExtensions", &[".org".to_string()])
            .expect("target extensions"),
        normalize_extension_set(
            "providerRegistryExtensions",
            &[".org".to_string(), ".js".to_string(), ".rs".to_string()],
        )
        .expect("registry extensions"),
    )
    .expect("extension evidence");

    assert_eq!(evidence.matching_file_count, 3);
    assert_eq!(evidence.candidate_language_file_count, 4);
    assert_eq!(evidence.source_extensions, [".org"]);
}
