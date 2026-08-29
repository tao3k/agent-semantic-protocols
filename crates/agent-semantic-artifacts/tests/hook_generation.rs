use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

use agent_semantic_artifacts::hook_generation::{
    HookGenerationCandidate, commit_hook_generation, prepare_hook_generation,
};

#[test]
fn hook_generation_prepare_commit_publishes_bound_candidate() {
    let fixture = Fixture::new();
    let evaluator = fixture.evaluator("g1");
    let prepared = prepare(&fixture, &evaluator, b"g1").expect("prepare HookGeneration");
    assert_eq!(
        prepared.receipt.hook_binary_path.parent(),
        prepared.receipt.generation_path.parent()
    );
    let publication =
        commit_hook_generation(fixture.state_home(), &prepared).expect("commit HookGeneration");
    assert_eq!(publication.schema_version, 1);
    assert!(publication.current_path.is_symlink());
    assert_eq!(
        std::fs::read_link(&publication.current_path)
            .expect("current HookGeneration target")
            .parent(),
        Some(Path::new("generations/blake3-256"))
    );
    assert!(publication.generation.hook_binary_path.exists());
    assert_bound_lookup(fixture.state_home());
}

#[test]
fn candidate_validation_and_commit_failure_preserve_previous() {
    let fixture = Fixture::new();
    let first = fixture.evaluator("g1");
    let first = prepare(&fixture, &first, b"g1").expect("prepare first");
    commit_hook_generation(fixture.state_home(), &first).expect("commit first");
    let previous =
        std::fs::read_link(fixture.state_home().join("hooks/current")).expect("previous current");

    let rejected = fixture.evaluator("rejected");
    let rejected = prepare(&fixture, &rejected, b"rejected").expect("prepare rejected");
    let validation: Result<(), String> = Err("injected validation failure".to_owned());
    assert!(validation.is_err());
    drop(rejected);
    assert_eq!(
        std::fs::read_link(fixture.state_home().join("hooks/current")).expect("current"),
        previous
    );

    let failed = fixture.evaluator("commit-failed");
    let failed = prepare(&fixture, &failed, b"commit-failed").expect("prepare failed commit");
    let hooks = fixture.state_home().join("hooks");
    let mut permissions = std::fs::metadata(&hooks)
        .expect("hooks metadata")
        .permissions();
    permissions.set_mode(0o555);
    std::fs::set_permissions(&hooks, permissions.clone()).expect("freeze hooks root");
    let result = commit_hook_generation(fixture.state_home(), &failed);
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hooks, permissions).expect("restore hooks root");
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_link(fixture.state_home().join("hooks/current")).expect("current"),
        previous
    );
}

#[test]
fn concurrent_publication_and_lookup_never_cross_bind_generation() {
    let fixture = Arc::new(Fixture::new());
    let seed = fixture.evaluator("seed");
    let seed = prepare(&fixture, &seed, b"seed").expect("prepare seed");
    commit_hook_generation(fixture.state_home(), &seed).expect("commit seed");
    let barrier = Arc::new(Barrier::new(33));
    let readers = (0..32)
        .map(|index| {
            let fixture = Arc::clone(&fixture);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let label = format!("g{index}");
                let evaluator = fixture.evaluator(&label);
                let prepared =
                    prepare(&fixture, &evaluator, label.as_bytes()).expect("prepare concurrent");
                barrier.wait();
                let publication = commit_hook_generation(fixture.state_home(), &prepared);
                if let Err(error) = &publication {
                    assert!(
                        error.contains("reasonKind=hook-generation-publication-conflict"),
                        "unexpected publication failure: {error}"
                    );
                }
                assert_bound_lookup(fixture.state_home());
                publication.is_ok()
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let committed = readers
        .into_iter()
        .map(|reader| reader.join().expect("publication thread"))
        .filter(|committed| *committed)
        .count();
    assert!(committed > 0);
    assert_bound_lookup(fixture.state_home());
}

#[test]
fn ten_developer_installs_keep_current_and_one_previous() {
    let fixture = Fixture::new();
    for index in 0..10 {
        let label = format!("dev-{index}");
        let evaluator = fixture.evaluator(&label);
        let prepared = prepare(&fixture, &evaluator, label.as_bytes()).expect("prepare developer");
        let receipt =
            commit_hook_generation(fixture.state_home(), &prepared).expect("commit developer");
        assert!(receipt.retained_generation_count <= 2);
        assert_bound_lookup(fixture.state_home());
    }
    let retained = std::fs::read_dir(fixture.state_home().join("hooks/generations/blake3-256"))
        .expect("Hook generations")
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .count();
    assert!(retained <= 2);
}

#[test]
fn runtime_state_is_not_a_hook_publication_dependency() {
    for runtime_state in ["absent", "stopped", "pending", "failed"] {
        let fixture = Fixture::new();
        if runtime_state != "absent" {
            let runtime = fixture.state_home().join("runtime");
            std::fs::create_dir_all(&runtime).expect("runtime fixture");
            std::fs::write(runtime.join("state"), runtime_state).expect("runtime state");
        }
        let evaluator = fixture.evaluator(runtime_state);
        let prepared = prepare(&fixture, &evaluator, runtime_state.as_bytes())
            .expect("prepare independent HookGeneration");
        commit_hook_generation(fixture.state_home(), &prepared)
            .expect("commit independent HookGeneration");
        assert_bound_lookup(fixture.state_home());
    }
}

fn prepare(
    fixture: &Fixture,
    evaluator: &Path,
    generation: &[u8],
) -> Result<agent_semantic_artifacts::hook_generation::PreparedHookGeneration, String> {
    prepare_hook_generation(
        fixture.state_home(),
        HookGenerationCandidate {
            hook_binary: evaluator,
            config: b"config",
            compiled_matcher: generation,
            registry: b"registry",
        },
    )
}

fn assert_bound_lookup(state_home: &Path) {
    let current = state_home.join("hooks/current");
    let hook_binary =
        std::fs::canonicalize(current.join("asp-hook")).expect("resolve current Hook binary");
    let generation = hook_binary
        .parent()
        .expect("candidate parent")
        .join("compiled-hook-generation.json");
    let hook_binary_label = std::fs::read_to_string(hook_binary).expect("Hook binary bytes");
    let generation_label = std::fs::read_to_string(generation).expect("generation bytes");
    assert!(hook_binary_label.contains(&generation_label));
}

struct Fixture {
    root: tempfile::TempDir,
    source_counter: std::sync::atomic::AtomicU64,
}

impl Fixture {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("fixture"),
            source_counter: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn state_home(&self) -> &Path {
        self.root.path()
    }

    fn evaluator(&self, label: &str) -> PathBuf {
        let nonce = self
            .source_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = self.root.path().join(format!("evaluator-{nonce}"));
        std::fs::write(&path, format!("#!/bin/sh\n# {label}\n{label}\n"))
            .expect("evaluator source");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("executable");
        path
    }
}
