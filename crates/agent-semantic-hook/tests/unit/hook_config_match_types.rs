use super::{CompiledCommandContains, CompiledPathGlobs};

#[test]
fn durable_matchers_preserve_live_command_and_path_semantics() {
    let live_command = crate::hook_config::core::compile::compile_command_contains(vec![
        "Cargo Check".to_owned(),
        "src/Exact.rs".to_owned(),
    ])
    .expect("compile live command matcher");
    let durable_command = CompiledCommandContains::from_durable(live_command.durable_artifact())
        .expect("hydrate command matcher");
    for command in [
        "direnv exec . cargo check -p crate",
        "cat SRC/exact.RS",
        "cargo test",
        "echo unrelated",
    ] {
        assert_eq!(
            durable_command.matches(command),
            live_command.matches(command),
            "command matcher drift for {command:?}"
        );
    }

    let live_paths = crate::hook_config::core::compile::compile_globs(
        "testPathGlobAny",
        vec![
            "**/*.rs".to_owned(),
            "*.rs".to_owned(),
            "crates/**/tests/*.toml".to_owned(),
        ],
    )
    .expect("compile live path matcher");
    let durable_paths = CompiledPathGlobs::from_durable(live_paths.durable_artifact())
        .expect("hydrate path matcher");
    for path in [
        "src/lib.rs",
        "lib.rs",
        "crates/hook/tests/config.toml",
        "crates/hook/src/config.toml",
        "README.md",
    ] {
        assert_eq!(
            durable_paths.matches(path),
            live_paths.matches(path),
            "path matcher drift for {path:?}"
        );
    }
}

#[test]
fn durable_matcher_hydrate_and_match_p99_is_sub_millisecond() {
    let live = crate::hook_config::core::compile::compile_globs(
        "performancePathGlobAny",
        vec![
            "**/*.rs".to_owned(),
            "*.rs".to_owned(),
            "crates/**/tests/*.toml".to_owned(),
        ],
    )
    .expect("compile live path matcher");
    let artifact = live.durable_artifact();
    let mut samples = (0..10_000)
        .map(|_| {
            let started = std::time::Instant::now();
            let matcher = CompiledPathGlobs::from_durable(artifact.clone())
                .expect("hydrate durable path matcher");
            std::hint::black_box(matcher.matches("crates/hook/tests/config.toml"));
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort_unstable();
    let p99 = samples[(samples.len() * 99) / 100];
    eprintln!(
        "[hook-durable-matcher] requests=10000 p99Nanos={} builderInvocations=0",
        p99.as_nanos()
    );
    assert!(
        p99 < std::time::Duration::from_millis(1),
        "durable matcher hydrate+match p99 must remain sub-millisecond, observed {p99:?}"
    );
}
